//FIXME: CHECK IF CONDITIONS AND ARGUMENTS ARE VALID
//
//FIXME: Either separate arg and cond resolving elsewhere or .
mod error;
pub mod representation;
mod semantic;

use common::{
    builtins::BuiltinType,
    intern::Intern,
    keywords::Keyword,
    metadata::FileMetadata,
    symbols::{
        AstId, BuiltinTypeId, Cond, EnumId, FuncId, InnerArgs, NameId, Span, SpannedInnerArgs,
        StructId, TypeDefId, TypedId,
    },
};

use crate::{
    analyzer::{
        error::SemanticError,
        representation::{
            EnumRepre, FieldRepre, FuncArgsRepre, FuncRepre, StructRepre, Table, TypeDefRepre,
            VariantRepre,
        },
        semantic::SemanticReporter,
    },
    parser::ast::{
        AbstractEnum, AbstractStruct, AbstractTypeDef, Expr, Item, Program, TypeExpr, UnaryOp,
    },
};
//WARN: 232 bytes 232 bytes 232 bytes 232 bytes 232 bytes 232 bytes
pub struct TypeResolver<'a> {
    program: &'a Program,
    interner: &'a Intern,
    //WARN: Horrors
    table: Table,
    // Startup idea:
    reporter: SemanticReporter<'a>,
}

impl TypeResolver<'_> {
    pub fn new<'a>(
        program: &'a Program,
        metadata: &'a FileMetadata,
        interner: &'a Intern,
    ) -> TypeResolver<'a> {
        TypeResolver {
            program,
            interner,
            table: Table::new(),
            reporter: SemanticReporter::new(metadata),
        }
    }

    //FIXME: USE A SINGULAR VECTOR INDEXED BY NAMEID LATER OVER A HASHMAP NOT NOW PLEASE NOT NOW
    // Ok. But when. I don't know.
    pub fn resolve(&mut self) -> Result<(), ()> {
        // Registering namespaces
        for (id, item) in self.program.items.iter().enumerate() {
            let ast_id = AstId::new(id as u32);

            match item {
                Item::Var(type_def) => self.register_typedef(type_def, ast_id),
                Item::Struct(structure) => self.register_struct(structure, ast_id),
                Item::Enum(enumeration) => self.register_enum(enumeration, ast_id),
            }
        }

        if !self.reporter.err_vec.is_empty() {
            self.reporter.emit_errors();
            std::process::exit(1);
        }

        //WARN: I don't know about this
        //Probably better off being functional
        let ids: Vec<TypedId> = self.table.sym_table.values().copied().collect();

        //NOTE: TypedIds are being reused here instead of the symbol wrapper which does the same thing
        // But maybe it should be used instead to be less confusing seeming
        for typed_id in ids {
            match typed_id {
                TypedId::TypeDef(type_def_id) => {
                    _ = self.resolve_typedef(type_def_id);
                }
                TypedId::Struct(struct_id) => {
                    _ = self.resolve_struct(struct_id);
                }
                TypedId::Enum(enum_id) => {
                    _ = self.resolve_enum(enum_id);
                }
                TypedId::Func(func_id) => {
                    _ = self.resolve_func(func_id);
                }
                // Um...
                TypedId::BuiltinType(builtintype_id) => todo!(),
            }
        }

        if !self.reporter.err_vec.is_empty() {
            self.reporter.emit_errors();
            std::process::exit(1);
        }

        Ok(())
    }

    fn resolve_typedef(&mut self, type_def_id: TypeDefId) -> Result<(), ()> {
        let ast_def = {
            let type_def = &self.table.typedefs[type_def_id.id as usize];
            &self.program.items[type_def.ast_id.id as usize]
        };

        // DIRTY
        if let Item::Var(abstract_typedef) = ast_def {
            let ty = self.resolve_type_expr(&abstract_typedef.ty)?;

            let mut args = Vec::new();

            //TODO: Make less terminal
            for spanned_arg in abstract_typedef.args.clone() {
                match ty {
                    TypedId::Struct(_) | TypedId::Enum(_) => {
                        if spanned_arg.inner_arg != InnerArgs::Warn {
                            let span = Span::new(spanned_arg.span.start, spanned_arg.span.end);

                            let sem_err = SemanticError::VagueArg(spanned_arg.inner_arg, span);

                            self.reporter.report_semantic(sem_err);
                            continue;
                        }
                    }
                    _ => (),
                }

                let resolved_arg = match self.resolve_arg(ty, &spanned_arg) {
                    Ok(a) => a,
                    Err(sem_err) => {
                        self.reporter.report_semantic(sem_err);
                        return Err(());
                    }
                };

                args.push(resolved_arg);
            }

            let mut conds = Vec::new();

            for expr in &abstract_typedef.conds {
                conds.push(self.resolve_cond(expr)?);
            }

            // DIRTY
            dbg!(args.clone(), conds.clone());
            let type_def = &mut self.table.typedefs[type_def_id.id as usize];
            type_def.type_id = Some(ty);
            type_def.args = args;
            type_def.conds = conds;
        }

        Ok(())
    }

    fn resolve_struct(&mut self, struct_id: StructId) -> Result<(), ()> {
        let ast_struct = {
            let structure = &self.table.structs[struct_id.id as usize];
            &self.program.items[structure.ast_id.id as usize]
        };

        // DIRTY
        if let Item::Struct(abstract_struct) = ast_struct {
            let mut fields: Vec<FieldRepre> = Vec::new();

            for (i, type_def) in abstract_struct.fields.iter().enumerate() {
                let typed_id = self.resolve_type_expr(&type_def.ty)?;

                let field_repre = FieldRepre::new(type_def.name_id, typed_id, AstId::new(i as u32));

                fields.push(field_repre);
            }

            let mut conds: Vec<Cond> = Vec::new();

            for expr in &abstract_struct.glob_conds {
                conds.push(self.resolve_cond(expr)?);
            }

            let mut args: Vec<InnerArgs> = Vec::new();

            dbg!(&abstract_struct.glob_args);

            for field in &fields {
                for spanned_arg in &abstract_struct.glob_args {
                    let arg = match self.resolve_arg(field.ty, spanned_arg) {
                        Ok(a) => a,
                        Err(sem_err) => {
                            self.reporter.report_semantic(sem_err);
                            // match sem_err {
                            //     // WARN: This is a weird way to handle this
                            //     SemanticError::VagueArg(_, _) => {
                            //         let type_span =
                            //             abstract_struct.fields[field.ast_id.id as usize].ty.span();
                            //
                            //         let complete_span =
                            //             Span::new(type_span.start, spanned_arg.span.end);
                            //
                            //         let actual_sem_err = SemanticError::VagueArg(
                            //             spanned_arg.inner_arg,
                            //             complete_span,
                            //         );
                            //
                            //         self.reporter.report_semantic(actual_sem_err);
                            //     }
                            //     err => self.reporter.report_semantic(err),
                            // }

                            // Need to point to particular type expression
                            // Since this is here now can this be handled inside?
                            return Err(());
                        }
                    };

                    args.push(arg);
                }
            }

            let structure = &mut self.table.structs[struct_id.id as usize];
            structure.fields.append(&mut fields);
        }

        Ok(())
    }

    fn resolve_enum(&mut self, enum_id: EnumId) -> Result<(), ()> {
        let ast_enum = {
            let enumeration = &self.table.enums[enum_id.id as usize];
            &self.program.items[enumeration.ast_id.id as usize]
        };

        // DIRTY
        if let Item::Enum(abstract_enum) = ast_enum {
            let mut variants: Vec<VariantRepre> = Vec::new();

            for (i, variant) in abstract_enum.variants.iter().enumerate() {
                if let Some(ty) = &variant.ty {
                    let typed_id = self.resolve_type_expr(ty)?;
                    let field_repre =
                        VariantRepre::new(variant.name_id, Some(typed_id), AstId::new(i as u32));

                    variants.push(field_repre);
                }
            }

            let mut conds: Vec<Cond> = Vec::new();

            for expr in &abstract_enum.glob_conds {
                conds.push(self.resolve_cond(expr)?);
            }

            let mut args: Vec<InnerArgs> = Vec::new();

            for variant in &variants {
                for spanned_arg in &abstract_enum.glob_args {
                    if let Some(type_id) = variant.typed_id {
                        let arg = match self.resolve_arg(type_id, spanned_arg) {
                            Ok(a) => a,
                            Err(sem_err) => {
                                self.reporter.report_semantic(sem_err);
                                //TODO: Fixing
                                // match sem_err {
                                //     SemanticError::VagueArg(inner_args, span) => {
                                //         let type_span = &abstract_enum.variants
                                //             [variant.ast_id.id as usize]
                                //             .ty
                                //             .as_ref()
                                //             .expect("We checked already")
                                //             .span();
                                //
                                //         let complete_span =
                                //             Span::new(type_span.start, spanned_arg.span.end);
                                //
                                //         let actual_sem_err = SemanticError::VagueArg(
                                //             spanned_arg.inner_arg,
                                //             complete_span,
                                //         );
                                //
                                //         self.reporter.report_semantic(actual_sem_err);
                                //     }
                                //     err => self.reporter.report_semantic(err),
                                // }
                                // Since this is here now can this be handled inside?

                                return Err(());
                            }
                        };

                        args.push(arg);
                    }
                }
            }

            let enumeration = &mut self.table.enums[enum_id.id as usize];
            enumeration.variants.append(&mut variants);
        }

        Ok(())
    }

    // Umm
    fn resolve_func(&mut self, func_id: FuncId) -> Result<(), ()> {
        // let ast_func = {
        //     let function = &mut self.table.funcs[func_id.id as usize];
        //     &self.program.items[function.ast_id.id as usize]
        // };

        todo!();
    }

    fn resolve_type_expr(&mut self, ty: &TypeExpr) -> Result<TypedId, ()> {
        match ty {
            TypeExpr::Var(name_id, span) => {
                if let Some(builtin_type) = BuiltinType::try_from_id(name_id.id) {
                    let builtin_id = BuiltinTypeId::new(self.table.builtin_types.len() as u32);

                    self.table.builtin_types.push(builtin_type);

                    return Ok(TypedId::BuiltinType(builtin_id));
                }

                if let Some(typed_id) = self.table.sym_table.get(name_id) {
                    return Ok(typed_id.clone());
                }

                let err_name = self.interner.search(name_id.id as usize);

                let err_msg = format!("\"{err_name}\" is not defined as a type");

                self.reporter.report_spanned(&err_msg, Some(err_name), span);

                return Err(());
            }
            TypeExpr::Generic(generic, span) => {
                match Keyword::try_as_kw(generic.base.id) {
                    Some(kw) => match kw {
                        //TODO: Should maybe put List | Set
                        Keyword::List => {
                            if generic.args.len() != 1 {
                                let msg = format!(
                                    "Expected 1 type within `List`, found {}",
                                    generic.args.len()
                                );

                                self.reporter.report_spanned(&msg, None, span);

                                return Err(());
                            }

                            self.resolve_type_expr(&generic.args[0])
                        }
                        Keyword::Map => {
                            if generic.args.len() != 2 {
                                let msg = format!(
                                    "Expected 2 types within `Map`, found {}",
                                    generic.args.len()
                                );

                                self.reporter.report_spanned(&msg, None, span);

                                return Err(());
                            }

                            let key = self.resolve_type_expr(&generic.args[0])?;
                            let val = self.resolve_type_expr(&generic.args[1])?;

                            let map = BuiltinType::Map(key, val);

                            let builtin_id =
                                BuiltinTypeId::new(self.table.builtin_types.len() as u32);

                            self.table.builtin_types.push(map);

                            Ok(TypedId::BuiltinType(builtin_id))
                        }
                        Keyword::Set => {
                            if generic.args.len() != 1 {
                                let msg = format!(
                                    "Expected one type within `Set`, found {}",
                                    generic.args.len()
                                );

                                self.reporter.report_spanned(&msg, None, span);

                                return Err(());
                            }

                            self.resolve_type_expr(&generic.args[0])
                        }
                        // I'm sure this can be done better...
                        _ => {
                            let err_name = self.interner.search(generic.base.id as usize);
                            //WARN: Questionablly phrased error message
                            //This COULD change so this will not be upheld at the parsing stage
                            let err_msg = format!(
                                "Found identifier \"{err_name}\" before generic parameters, but only `List`, `Set`, and `Map` are valid data structures"
                            );

                            self.reporter.report_spanned(&err_msg, Some(err_name), span);

                            Err(())
                        }
                    },
                    None => {
                        // 2004 dog 2004 television
                        let err_name = self.interner.search(generic.base.id as usize);

                        let err_msg = format!(
                            "Found identifier \"{err_name}\" before generic parameters, but only `List`, `Set`, and `Map` are valid data structures"
                        );

                        self.reporter.report_spanned(&err_msg, Some(err_name), span);

                        Err(())
                    }
                }
            }
            // Maybe this shouldn't have a span
            TypeExpr::Any(_) => {
                let index = self.table.builtin_types.len();

                self.table.builtin_types.push(BuiltinType::Any(None));

                Ok(TypedId::BuiltinType(BuiltinTypeId::new(index as u32)))
            }
        }
    }

    fn resolve_cond(&mut self, expr: &Expr) -> Result<Cond, ()> {
        match expr {
            Expr::Var(name_id, span) => {
                if let Some(cond) = Cond::try_from_id(name_id.id) {
                    return Ok(cond);
                }

                let err_name = self.interner.search(name_id.id as usize);
                let err_msg = format!("\"{err_name}\" is not a valid condition");

                self.reporter.report_spanned(&err_msg, Some(err_name), span);

                Err(())
            }
            Expr::Unary(unary, _) => match unary.op {
                UnaryOp::Not => {
                    let cond = self.resolve_cond(&unary.expr)?;
                    Ok(Cond::Not(Box::new(cond)))
                }
            },
            Expr::Call(call, _) => {
                // This will return a cond with a function id to a defined function with args

                // Can't really do it like this.
                let func_id = self.contains_func(call.name_id);

                let mut args: Vec<FuncArgsRepre> = Vec::new();

                for expr in &call.exprs {
                    let arg = self.resolve_func_arg(expr)?;
                    args.push(arg);
                }

                // let function = FuncRepre::new(call.name_id, func_id, args);

                todo!();
            }
            Expr::Str(name_id, span) => {
                let err_name = self.interner.search(name_id.id as usize);
                let err_msg = format!("\"{err_name}\" is not a valid condition");

                self.reporter.report_spanned(&err_msg, Some(err_name), span);

                Err(())
            }
            Expr::Integer(_, span) | Expr::Float(_, span) => {
                let err_msg = format!("Numerics cannot be used as conditions alone");

                self.reporter.report_spanned(&err_msg, None, span);

                Err(())
            }
            Expr::FieldAccess(field_access, span) => {
                //TODO: Is this worth evaluating as an expression just to get the name?
                // Sure

                let err_msg = format!("Conditions cannot be accessed as fields");

                self.reporter.report_spanned(&err_msg, None, span);

                Err(())
            }
            Expr::BinaryExpr { lhs, op, rhs } => todo!(),
        }
    }

    //FIXME: SPANNING IS LOST IN MANY PLACES
    fn resolve_arg(
        &self,
        typed_id: TypedId,
        spanned_arg: &SpannedInnerArgs,
        // Returns SemanticError due to borrowing issues
    ) -> Result<InnerArgs, SemanticError> {
        match typed_id {
            TypedId::Struct(struct_id) => {
                let structure = &self.table.structs[struct_id.id as usize];

                for (i, field) in structure.fields.iter().enumerate() {
                    self.resolve_arg(field.ty, spanned_arg)?;
                }

                Ok(spanned_arg.inner_arg)
            }
            TypedId::Enum(enum_id) => {
                let enumeration = &self.table.enums[enum_id.id as usize];

                for (i, variant) in enumeration.variants.iter().enumerate() {
                    if let Some(typed_id) = variant.typed_id {
                        self.resolve_arg(typed_id, spanned_arg)?;
                    }
                }

                Ok(spanned_arg.inner_arg)
            }

            TypedId::TypeDef(type_def_id) => {
                let type_def = &self.table.typedefs[type_def_id.id as usize];

                //WARN: REMOVE THE EXPECT (Eventually)
                self.resolve_arg(type_def.type_id.expect("Resolved already"), spanned_arg)
            }
            TypedId::BuiltinType(builtin_type_id) => {
                let ty = &self.table.builtin_types[builtin_type_id.id as usize];

                match ty {
                    BuiltinType::Set(typed_id) | BuiltinType::List(typed_id) => {
                        self.resolve_arg(*typed_id, spanned_arg)
                    }
                    BuiltinType::Map(key_id, val_id) => {
                        // This looks weird...
                        self.resolve_arg(*key_id, spanned_arg)?;
                        self.resolve_arg(*val_id, spanned_arg)
                    }
                    BuiltinType::Any(_) => Ok(spanned_arg.inner_arg),
                    builtin_type => {
                        if !spanned_arg.inner_arg.supports_builtin_type(builtin_type) {
                            return Err(SemanticError::UnsupportedArg(
                                spanned_arg.clone(),
                                builtin_type.kind(),
                            ));
                        }

                        Ok(spanned_arg.inner_arg)
                    }
                }
            }
            _ => unreachable!("Functions are not capable of taking arguments in the parser"),
        }
    }

    // How do we solve this?
    // I DONT KNOW
    fn resolve_expr(&mut self, expr: &Expr) -> Result<TypedId, ()> {
        match expr {
            Expr::Var(name_id, span) => todo!(),
            Expr::Integer(num, span) => todo!(),
            Expr::Float(num, span) => todo!(),
            Expr::Str(name_id, span) => todo!(),
            Expr::Call(call, span) => todo!(),
            Expr::FieldAccess(abstract_field_access, span) => todo!(),
            Expr::Unary(unary, span) => todo!(),
            Expr::BinaryExpr { lhs, op, rhs } => todo!(),
        }
    }

    // TODO: Register functions user made functions first...
    fn contains_func(&self, name_id: NameId) -> bool {
        if let Some(typed_id) = self.table.sym_table.get(&name_id) {
            if let TypedId::Func(_) = typed_id {
                return true;
            }
        }

        false
    }

    fn resolve_func_arg(&mut self, expr: &Expr) -> Result<FuncArgsRepre, ()> {
        todo!();
    }

    fn register_typedef(&mut self, type_def: &AbstractTypeDef, ast_id: AstId) {
        let def_id = TypeDefId::new(self.table.typedefs.len() as u32);

        let check = self
            .table
            .sym_table
            .insert(type_def.name_id, TypedId::TypeDef(def_id));

        if check.is_some() {
            let duplicate = self.interner.search(type_def.name_id.id as usize);

            let msg = format!("The symbol \"{}\" appears more than once", duplicate);
            self.reporter
                .report_spanned(&msg, None, &type_def.name_span);

            return;
        }

        let ty = TypeDefRepre::new(type_def.name_id, ast_id);

        self.table.typedefs.push(ty);
    }

    fn register_struct(&mut self, structure: &AbstractStruct, ast_id: AstId) {
        let struct_id = StructId::new(self.table.structs.len() as u32);

        let check = self
            .table
            .sym_table
            .insert(structure.name_id, TypedId::Struct(struct_id));

        if check.is_some() {
            let duplicate = self.interner.search(structure.name_id.id as usize);

            let msg = format!("The symbol \"{}\" appears more than once", duplicate);
            self.reporter
                .report_spanned(&msg, None, &structure.name_span);

            return;
        }

        let ty = StructRepre::new(structure.name_id, ast_id);

        self.table.structs.push(ty);
    }

    fn register_enum(&mut self, enumeration: &AbstractEnum, ast_id: AstId) {
        let enum_id = EnumId::new(self.table.enums.len() as u32);

        let check = self
            .table
            .sym_table
            .insert(enumeration.name_id, TypedId::Enum(enum_id));

        if check.is_some() {
            let duplicate = self.interner.search(enumeration.name_id.id as usize);

            let msg = format!("The symbol \"{}\" appears more than once", duplicate);
            self.reporter
                .report_spanned(&msg, None, &enumeration.name_span);

            return;
        }

        let ty = EnumRepre::new(enumeration.name_id, ast_id);

        self.table.enums.push(ty);
    }
}
