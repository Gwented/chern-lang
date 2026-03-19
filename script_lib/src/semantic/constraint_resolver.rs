use common::{
    builtins::BuiltinType,
    intern::Intern,
    metadata::FileMetadata,
    symbols::{AstId, Cond, InnerArgs, Span, SpannedInnerArgs, TypedId},
};

use crate::{
    parser::ast::{AbstractEnum, AbstractStruct, AbstractTypeDef, AstInfo, Expr, Item, UnaryOp},
    semantic::{
        error::SemanticError,
        representation::{FieldRepre, FuncArgsRepre, Table, VariantRepre},
        semantic_reporter::SemanticReporter,
    },
};

pub struct ConstraintResolver<'a> {
    ast_info: &'a AstInfo,
    interner: &'a Intern,
    table: &'a mut Table,
    reporter: SemanticReporter<'a>,
}

impl ConstraintResolver<'_> {
    pub fn new<'a>(
        ast_info: &'a AstInfo,
        metadata: &'a FileMetadata,
        interner: &'a Intern,
        table: &'a mut Table,
    ) -> ConstraintResolver<'a> {
        ConstraintResolver {
            ast_info,
            interner,
            table,
            reporter: SemanticReporter::new(metadata),
        }
    }

    pub fn resolve(&mut self) {
        //NOTE: TypedIds are being reused here instead of the symbol wrapper which does the same thing
        // But maybe it should be used instead to be less confusing seeming

        // Could be done in a different way but fine for now
        for (id, item) in self.ast_info.items.iter().enumerate() {
            let ast_id = AstId::new(id as u32);

            match item {
                Item::Var(type_def) => {
                    _ = self.resolve_typedef(type_def, ast_id);
                }
                Item::Struct(structure) => {
                    _ = self.resolve_struct(structure, ast_id);
                }
                Item::Enum(enumeration) => {
                    _ = self.resolve_enum(enumeration, ast_id);
                }
            }
        }

        dbg!(&self.table);

        if !self.reporter.err_vec.is_empty() {
            self.reporter.emit_errors();
            std::process::exit(1);
        }
    }

    fn resolve_typedef(
        &mut self,
        abstract_typedef: &AbstractTypeDef,
        ast_id: AstId,
    ) -> Result<(), ()> {
        // Not too favorable of this needing to happen but the ast would need to also have this
        // done otherwise. But maybe this should take in the type def and the id to avoid this.
        let type_def_id = match self.table.typed_ids[&self.table.sym_ids[&ast_id]] {
            TypedId::TypeDef(struct_id) => struct_id,
            _ => unreachable!(),
        };

        let mut args = Vec::new();
        let typed_id = self.table.typedefs[type_def_id.id as usize].typed_id;

        //TODO: Make less terminal and have a better solution for this
        for spanned_arg in abstract_typedef.args.clone() {
            match typed_id {
                TypedId::Struct(_) | TypedId::Enum(_) => {
                    if spanned_arg.arg != InnerArgs::Warn {
                        let span = Span::new(spanned_arg.span.start, spanned_arg.span.end);
                        let sem_err = SemanticError::VagueArg(spanned_arg.arg, span);

                        self.reporter.report_semantic(sem_err);
                        continue;
                    }
                }
                _ => (),
            }

            let resolved_arg = match self.resolve_arg(typed_id, &spanned_arg) {
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

        let type_def = &mut self.table.typedefs[type_def_id.id as usize];
        type_def.conds = conds;
        type_def.args = args;

        Ok(())
    }

    //NOTE: The reason this would need to look at the struct again would be because it is iterating
    // through items despite there already being a known struct id, which could be prevented if the
    // struct id itself was passed, but then the loop would iterate over everything by default
    // which seems bad if they're just builtins etc.
    fn resolve_struct(
        &mut self,
        abstract_struct: &AbstractStruct,
        ast_id: AstId,
    ) -> Result<(), ()> {
        // This looks weird
        let struct_id = match self.table.typed_ids[&self.table.sym_ids[&ast_id]] {
            TypedId::Struct(struct_id) => struct_id,
            _ => unreachable!(),
        };

        let mut conds: Vec<Cond> = Vec::new();

        for expr in &abstract_struct.glob_conds {
            conds.push(self.resolve_cond(expr)?);
        }

        let mut args: Vec<InnerArgs> = Vec::new();
        // This looks odd too
        let fields = &self.table.structs[struct_id.id as usize].fields;
        dbg!(&abstract_struct.glob_args);

        //TODO: Need to point to particular type expression
        for field in fields {
            for spanned_arg in &abstract_struct.glob_args {
                let arg = match self.resolve_arg(field.ty, spanned_arg) {
                    Ok(a) => a,
                    Err(sem_err) => {
                        self.reporter.report_semantic(sem_err);
                        return Err(());
                    }
                };

                args.push(arg);
            }
        }

        dbg!(&args);
        dbg!(&conds);

        let structure = &mut self.table.structs[struct_id.id as usize];

        // I'm scared of this
        structure.args = args;
        structure.conds = conds;

        Ok(())
    }

    fn resolve_enum(&mut self, abstract_enum: &AbstractEnum, ast_id: AstId) -> Result<(), ()> {
        let enum_id = match self.table.typed_ids[&self.table.sym_ids[&ast_id]] {
            TypedId::Enum(enum_id) => enum_id,
            _ => unreachable!(),
        };

        let mut conds: Vec<Cond> = Vec::new();

        for expr in &abstract_enum.glob_conds {
            conds.push(self.resolve_cond(expr)?);
        }

        let variants = &self.table.enums[enum_id.id as usize].variants;
        let mut args: Vec<InnerArgs> = Vec::new();

        for variant in variants {
            for spanned_arg in &abstract_enum.glob_args {
                if let Some(type_id) = variant.typed_id {
                    let arg = match self.resolve_arg(type_id, spanned_arg) {
                        Ok(a) => a,
                        Err(sem_err) => {
                            self.reporter.report_semantic(sem_err);
                            return Err(());
                        }
                    };

                    args.push(arg);
                }
            }
        }

        let enumeration = &mut self.table.enums[enum_id.id as usize];

        enumeration.conds = conds;
        enumeration.args = args;

        Ok(())
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
                // let func_id = self.contains_func(call.name_id);

                let mut args: Vec<FuncArgsRepre> = Vec::new();

                // for expr in &call.exprs {
                //     let arg = self.resolve_func_arg(expr)?;
                //     args.push(arg);
                // }

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
            //NOTE: Types further down the recursive depth do not know they are a field, so they
            // have have their span made accurate within structures and enums
            TypedId::Struct(struct_id) => {
                let structure = &self.table.structs[struct_id.id as usize];

                for (i, field) in structure.fields.iter().enumerate() {
                    let arg_res = self.resolve_arg(field.ty, spanned_arg);

                    // DIRTY
                    if let Err(SemanticError::UnsupportedArg(found_spanned_arg, kind)) = arg_res {
                        if let Item::Struct(abs_struct) =
                            &self.ast_info.items[structure.ast_id.id as usize]
                        {
                            // or field span
                            let ast_span = &abs_struct.fields[field.ast_id.id as usize].ty.span();

                            let start = std::cmp::min(ast_span.start, found_spanned_arg.span.start);
                            let end = std::cmp::max(ast_span.end, found_spanned_arg.span.end);

                            //NOTE:
                            let actual_span = Span::new(start, end);
                            let spanned_arg =
                                SpannedInnerArgs::new(found_spanned_arg.arg, actual_span);

                            return Err(SemanticError::UnsupportedArg(spanned_arg, kind));
                        }
                    }
                }

                Ok(spanned_arg.arg)
            }
            TypedId::Enum(enum_id) => {
                let enumeration = &self.table.enums[enum_id.id as usize];

                for (i, variant) in enumeration.variants.iter().enumerate() {
                    if let Some(ty) = variant.typed_id {
                        let arg_res = self.resolve_arg(typed_id, spanned_arg);

                        if let Err(SemanticError::UnsupportedArg(found_spanned_arg, kind)) = arg_res
                        {
                            if let Item::Struct(abs_struct) =
                                &self.ast_info.items[enumeration.ast_id.id as usize]
                            {
                                // or field span
                                let ast_span =
                                    &abs_struct.fields[variant.ast_id.id as usize].ty.span();

                                let start =
                                    std::cmp::min(ast_span.start, found_spanned_arg.span.start);
                                let end = std::cmp::max(ast_span.end, found_spanned_arg.span.end);

                                //NOTE:
                                let actual_span = Span::new(start, end);
                                let spanned_arg =
                                    SpannedInnerArgs::new(found_spanned_arg.arg, actual_span);

                                return Err(SemanticError::UnsupportedArg(spanned_arg, kind));
                            }
                        }
                    }
                }

                Ok(spanned_arg.arg)
            }

            TypedId::TypeDef(type_def_id) => {
                let type_def = &self.table.typedefs[type_def_id.id as usize];

                self.resolve_arg(type_def.typed_id, spanned_arg)
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
                    BuiltinType::Any(_) => Ok(spanned_arg.arg),
                    builtin_type => {
                        if !spanned_arg.arg.supports_builtin_type(builtin_type) {
                            return Err(SemanticError::UnsupportedArg(
                                spanned_arg.clone(),
                                builtin_type.kind(),
                            ));
                        }

                        Ok(spanned_arg.arg)
                    }
                }
            }
            _ => unreachable!("Functions are not capable of taking arguments in the parser"),
        }
    }
}
