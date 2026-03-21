use common::{
    builtins::BuiltinType,
    intern::Intern,
    keywords::{self, Keyword},
    metadata::FileMetadata,
    symbols::{
        AstId, BuiltinTypeId, Cond, EnumId, FuncId, InnerArgs, NameId, Span, SpannedInnerArgs,
        StructId, SymbolId, TypeDefId, TypeId,
    },
};

use crate::{
    parser::ast::{
        AbstractEnum, AbstractStruct, AbstractTypeDef, AstInfo, Expr, Item, TypeExpr, UnaryOp,
    },
    semantic::{
        error::SemanticError,
        representation::{
            EnumRepre, FieldRepre, FuncArgsRepre, FuncRepre, StructRepre, Symbol, Table, Type,
            TypeDefRepre, VariantRepre,
        },
        semantic_reporter::SemanticReporter,
    },
};
/// Fills a given table with type information regarding the NameId and TypeId
pub struct TypeResolver<'a> {
    ast_info: &'a AstInfo,
    interner: &'a Intern,
    //WARN: Horrors
    table: &'a mut Table,
    // Startup idea:
    reporter: SemanticReporter<'a>,
    //NOTE: May handle this differently but ok for now
    unknown_id: Option<TypeId>,
}

impl TypeResolver<'_> {
    pub fn new<'a>(
        ast_info: &'a AstInfo,
        metadata: &'a FileMetadata,
        interner: &'a Intern,
        table: &'a mut Table,
    ) -> TypeResolver<'a> {
        TypeResolver {
            ast_info,
            interner,
            table,
            reporter: SemanticReporter::new(metadata),
            unknown_id: None,
        }
    }

    //FIXME: USE A SINGULAR VECTOR INDEXED BY NAMEID LATER OVER A HASHMAP NOT NOW PLEASE NOT NOW
    // Ok. But when. I don't know.
    pub fn resolve(&mut self) {
        // Registering namespaces
        for (id, item) in self.ast_info.items.iter().enumerate() {
            let ast_id = AstId::new(id as u32);

            match item {
                Item::Var(type_def) => self.register_typedef(type_def, ast_id),
                Item::Struct(structure) => self.register_struct(structure, ast_id),
                Item::Enum(enumeration) => self.register_enum(enumeration, ast_id),
                Item::Alias(alias) => todo!(),
            }
        }

        if !self.reporter.err_vec.is_empty() {
            self.reporter.emit_errors();
            std::process::exit(1);
        }

        //FIXME: Need to resolve types first so may be better to just resolve args and conds in an
        // entirely different structure, especially due to complexity explosion

        // The is resolving types but not resolving args or conditions.
        // Everything is in order so this cannot fail unless something internally went wrong.
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
                Item::Alias(alias) => todo!(),
            }
        }

        dbg!(&self.table);

        if !self.reporter.err_vec.is_empty() {
            self.reporter.emit_errors();
            std::process::exit(1);
        }
    }

    fn resolve_typedef(&mut self, abs_typedef: &AbstractTypeDef, ast_id: AstId) -> Result<(), ()> {
        // I don't understand
        let type_id = self.resolve_type_expr(&abs_typedef.ty, ast_id)?;

        let sym_id = self.table.sym_ids[&ast_id];

        let type_def = self.table.get_typedef_mut(sym_id);
        type_def.type_id = type_id;

        Ok(())
    }

    fn resolve_struct(&mut self, abs_struct: &AbstractStruct, ast_id: AstId) -> Result<(), ()> {
        let mut fields: Vec<FieldRepre> = Vec::new();

        for (i, type_def) in abs_struct.fields.iter().enumerate() {
            let type_id = self.resolve_type_expr(&type_def.ty, ast_id)?;

            let field_repre = FieldRepre::new(type_def.name_id, type_id, AstId::new(i as u32));

            fields.push(field_repre);
        }

        let sym_id = self.table.sym_ids[&ast_id];

        let struct_repre = self.table.get_struct_mut(sym_id);

        struct_repre.fields.append(&mut fields);

        Ok(())
    }

    fn resolve_enum(&mut self, abs_enum: &AbstractEnum, ast_id: AstId) -> Result<(), ()> {
        let mut variants: Vec<VariantRepre> = Vec::new();

        for (i, variant) in abs_enum.variants.iter().enumerate() {
            if let Some(ty) = &variant.ty {
                let type_id = self.resolve_type_expr(ty, ast_id)?;
                let variant_repre =
                    VariantRepre::new(variant.name_id, Some(type_id), AstId::new(i as u32));

                variants.push(variant_repre);
            }
        }

        let sym_id = self.table.sym_ids[&ast_id];

        let enum_repre = self.table.get_enum_mut(sym_id);

        enum_repre.variants.append(&mut variants);

        Ok(())
    }

    fn resolve_type_expr(&mut self, ty: &TypeExpr, ast_id: AstId) -> Result<TypeId, ()> {
        match ty {
            // Escaped can be put here but it seems weird giving a kind just for this one task
            TypeExpr::Var(name_id, span) => {
                // Returns the name's id since it is a valid non-data structure intrinsic type
                if let Some(_) = BuiltinType::try_from_id(name_id.id) {
                    return Ok(TypeId::new(name_id.id));
                }

                // Loop that checks if the name id was registered, then uses its corresponding ast_id to
                // extract the name id's type and returns that as the type to be referenced
                for (current_ast_id, current_name_id) in &self.table.name_ids {
                    if current_name_id == name_id {
                        let sym_id = self.table.sym_ids[&current_ast_id];
                        let type_id = match &self.table.symbols[&sym_id] {
                            Symbol::Struct(struct_repre) => struct_repre.type_id,
                            Symbol::Func(func_repre) => func_repre.type_id,
                            Symbol::Enum(enum_repre) => enum_repre.type_id,
                            // This is not possible
                            // Symbol::TypeDef(type_def_repre) => type_def_repre.type_id,
                            _ => todo!(),
                        };
                        return Ok(type_id);
                    }
                }

                let err_name = self.interner.search(name_id.id as usize);

                let err_msg = format!("\"{err_name}\" is not defined as a type");

                self.reporter.report_spanned(&err_msg, None, span);

                return Err(());
            }
            // May put this with Var as an OR but separate for now
            TypeExpr::Escaped(name_id, span) => {
                for (current_ast_id, current_name_id) in &self.table.name_ids {
                    if current_name_id == name_id {
                        let sym_id = self.table.sym_ids[&current_ast_id];
                        let type_id = match &self.table.symbols[&sym_id] {
                            Symbol::Struct(struct_repre) => struct_repre.type_id,
                            Symbol::Func(func_repre) => func_repre.type_id,
                            Symbol::Enum(enum_repre) => enum_repre.type_id,
                            _ => unreachable!(),
                        };
                        return Ok(type_id);
                    }
                }

                let err_name = self.interner.search(name_id.id as usize);

                let err_msg = format!("\"{err_name}\" is not defined as a type");

                self.reporter.report_spanned(&err_msg, None, span);

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

                            self.resolve_type_expr(&generic.args[0], ast_id)
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

                            let key = self.resolve_type_expr(&generic.args[0], ast_id)?;
                            let val = self.resolve_type_expr(&generic.args[1], ast_id)?;

                            let map = BuiltinType::Map(key, val);

                            let id = self.table.types.len() as u32;

                            self.table.types.push(Type::BuiltinType(map));

                            Ok(TypeId::new(id))
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

                            self.resolve_type_expr(&generic.args[0], ast_id)
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
            TypeExpr::Any(_) => {
                let id = self.table.types.len() as u32;

                self.table
                    .types
                    .push(Type::BuiltinType(BuiltinType::Any(None)));

                Ok(TypeId::new(id))
            }
        }
    }
    // How do we solve this?
    // I DONT KNOW
    fn resolve_expr(&mut self, expr: &Expr) -> Result<TypeId, ()> {
        match expr {
            Expr::Var(name_id, span) => todo!(),
            Expr::Integer(num, span) => todo!(),
            Expr::Float(num, span) => todo!(),
            Expr::Str(name_id, span) => todo!(),
            Expr::Call(call, span) => todo!(),
            Expr::FieldAccess(abs_field_access, span) => todo!(),
            Expr::Unary(unary, span) => todo!(),
            Expr::BinaryExpr { lhs, op, rhs } => todo!(),
            Expr::Char(_, _) => todo!(),
            Expr::Default(_, expr) => todo!(),
        }
    }

    // TODO: Register functions user made functions first...
    // fn contains_func(&self, name_id: NameId) -> bool {
    //     if let Some(type_id) = self.table.type_ids.get(&name_id) {
    //         if let TypeId::Func(_) = type_id {
    //             return true;
    //         }
    //     }
    //
    //     false
    // }

    fn resolve_func_arg(&mut self, expr: &Expr) -> Result<FuncArgsRepre, ()> {
        todo!();
    }

    // Does this have any reason to return a Result?
    fn register_typedef(&mut self, type_def: &AbstractTypeDef, ast_id: AstId) {
        //NOTE: There is no scoping needed I believe so this is valid

        // This would never realistically cause a bottleneck since, why would you have that many
        // variables? But, still a little bit of code smell.
        if self
            .table
            .name_ids
            .values()
            .any(|id| *id == type_def.name_id)
        {
            let duplicate = self.interner.search(type_def.name_id.id as usize);

            let msg = format!("The symbol \"{duplicate}\" appears more than once");
            self.reporter
                .report_spanned(&msg, None, &type_def.name_span);

            return;
        }

        self.table.name_ids.insert(ast_id, type_def.name_id);

        let sym_id = SymbolId::new(self.table.sym_ids.len() as u32);
        self.table.sym_ids.insert(ast_id, sym_id);

        //WARN: This type id is fake...
        let type_id = if let Some(id) = self.unknown_id {
            id
        } else {
            let id = TypeId::new(self.table.types.len() as u32);
            self.unknown_id = Some(id);
            self.table.types.push(Type::Unknown);

            id
        };

        let type_def_repre = TypeDefRepre::new(type_def.name_id, type_id, sym_id, ast_id);

        self.table
            .symbols
            .insert(sym_id, Symbol::TypeDef(type_def_repre));
    }

    fn register_struct(&mut self, abs_struct: &AbstractStruct, ast_id: AstId) {
        // O(floor)
        if self
            .table
            .name_ids
            .values()
            .any(|id| *id == abs_struct.name_id)
        {
            let duplicate = self.interner.search(abs_struct.name_id.id as usize);

            let msg = format!("The struct \"{duplicate}\" appears more than once");
            self.reporter
                .report_spanned(&msg, None, &abs_struct.name_span);

            return;
        }

        self.table.name_ids.insert(ast_id, abs_struct.name_id);

        let sym_id = SymbolId::new(self.table.sym_ids.len() as u32);
        self.table.sym_ids.insert(ast_id, sym_id);

        let type_id = TypeId::new(self.table.types.len() as u32);

        let struct_repre =
            StructRepre::new(abs_struct.name_id, sym_id, ast_id, type_id, Vec::new());

        self.table
            .symbols
            .insert(sym_id, Symbol::Struct(struct_repre));

        self.table.types.push(Type::Struct(sym_id));
    }

    fn register_enum(&mut self, abs_enum: &AbstractEnum, ast_id: AstId) {
        if self
            .table
            .name_ids
            .values()
            .any(|id| *id == abs_enum.name_id)
        {
            let duplicate = self.interner.search(abs_enum.name_id.id as usize);

            let msg = format!("The enum \"{duplicate}\" appears more than once");
            self.reporter
                .report_spanned(&msg, None, &abs_enum.name_span);

            return;
        }

        self.table.name_ids.insert(ast_id, abs_enum.name_id);

        let type_id = TypeId::new(self.table.types.len() as u32);

        let sym_id = SymbolId::new(self.table.sym_ids.len() as u32);

        self.table.sym_ids.insert(ast_id, sym_id);

        let enum_repre = EnumRepre::new(abs_enum.name_id, sym_id, ast_id, type_id, Vec::new());
        self.table.symbols.insert(sym_id, Symbol::Enum(enum_repre));

        // DO WE NEED THIS?
        self.table.types.push(Type::Enum(sym_id));
    }
}
