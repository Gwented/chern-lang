use common::{
    builtins::BuiltinType,
    intern::Intern,
    keywords::{self, Keyword},
    metadata::FileMetadata,
    symbols::{
        AstId, BuiltinTypeId, Cond, EnumId, FuncId, InnerArgs, NameId, Span, SpannedInnerArgs,
        StructId, SymbolId, TypeDefId, TypedId,
    },
};

use crate::{
    parser::ast::{
        AbstractEnum, AbstractStruct, AbstractTypeDef, AstInfo, Expr, Item, TypeExpr, UnaryOp,
    },
    semantic::{
        error::SemanticError,
        representation::{
            EnumRepre, FieldRepre, FuncArgsRepre, FuncRepre, StructRepre, Table, TypeDefRepre,
            VariantRepre,
        },
        semantic_reporter::SemanticReporter,
    },
};
/// Fills a given table with type information regarding the NameId and TypedId
pub struct TypeResolver<'a> {
    ast_info: &'a AstInfo,
    interner: &'a Intern,
    //WARN: Horrors
    table: &'a mut Table,
    tracker: Tracker,
    // Startup idea:
    reporter: SemanticReporter<'a>,
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
            tracker: Tracker::new(),
            table,
            reporter: SemanticReporter::new(metadata),
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
            }
        }

        if !self.reporter.err_vec.is_empty() {
            self.reporter.emit_errors();
            std::process::exit(1);
        }

        //FIXME: Need to resolve types first so may be better to just resolve args and conds in an
        // entirely different structure, especially due to complexity explosion

        //NOTE: TypedIds are being reused here instead of the symbol wrapper which does the same thing
        // But maybe it should be used instead to be less confusing seeming

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
        let ty = self.resolve_type_expr(&abstract_typedef.ty, ast_id)?;

        let sym_id = self.table.sym_ids[&ast_id];

        let type_def_repre = TypeDefRepre::new(abstract_typedef.name_id, ty, sym_id, ast_id);
        let type_def_id = TypeDefId::new(self.table.typedefs.len() as u32);

        self.table
            .typed_ids
            .insert(sym_id, TypedId::TypeDef(type_def_id));

        self.table.typedefs.push(type_def_repre);

        Ok(())
    }

    fn resolve_struct(
        &mut self,
        abstract_struct: &AbstractStruct,
        ast_id: AstId,
    ) -> Result<(), ()> {
        let mut fields: Vec<FieldRepre> = Vec::new();

        for (i, type_def) in abstract_struct.fields.iter().enumerate() {
            let typed_id = self.resolve_type_expr(&type_def.ty, ast_id)?;

            let field_repre = FieldRepre::new(type_def.name_id, typed_id, AstId::new(i as u32));

            fields.push(field_repre);
        }

        let sym_id = self.table.sym_ids[&ast_id];

        let struct_repre = StructRepre::new(abstract_struct.name_id, sym_id, ast_id, fields);
        let struct_id = StructId::new(self.table.structs.len() as u32);

        self.table
            .typed_ids
            .insert(sym_id, TypedId::Struct(struct_id));

        self.table.structs.push(struct_repre);

        Ok(())
    }

    fn resolve_enum(&mut self, abstract_enum: &AbstractEnum, ast_id: AstId) -> Result<(), ()> {
        let mut variants: Vec<VariantRepre> = Vec::new();

        for (i, variant) in abstract_enum.variants.iter().enumerate() {
            if let Some(ty) = &variant.ty {
                let typed_id = self.resolve_type_expr(ty, ast_id)?;
                let variant_repre =
                    VariantRepre::new(variant.name_id, Some(typed_id), AstId::new(i as u32));

                variants.push(variant_repre);
            }
        }

        let sym_id = self.table.sym_ids[&ast_id];

        let enum_repre = EnumRepre::new(abstract_enum.name_id, sym_id, ast_id, variants);
        let enum_id = EnumId::new(self.table.enums.len() as u32);

        self.table.typed_ids.insert(sym_id, TypedId::Enum(enum_id));

        self.table.enums.push(enum_repre);

        Ok(())
    }

    fn resolve_type_expr(&mut self, ty: &TypeExpr, ast_id: AstId) -> Result<TypedId, ()> {
        match ty {
            TypeExpr::Var(name_id, span) => {
                //WARN: MAKE SURE THIS WORKS
                if let Some(_) = BuiltinType::try_from_id(name_id.id) {
                    let builtin_id = BuiltinTypeId::new(name_id.id);

                    return Ok(TypedId::BuiltinType(builtin_id));
                }

                // Loop that checks if the name id was registered, then uses its corresponding ast_id to
                // extract the name id's type and returns that as the type to be referenced
                for (current_ast_id, current_name_id) in &self.table.name_ids {
                    if current_name_id == name_id {
                        let ty = self.table.typed_ids[&self.table.sym_ids[&current_ast_id]];
                        return Ok(ty);
                    }
                }

                //NOTE: Just curious of this
                // let type_res = self.table.name_ids
                //     .iter()
                //     .find(|(_, current)| current.id == name_id.id)
                //     .map(|(ast, _)| self.table.typed_ids[&self.table.sym_ids[&ast]]);
                //
                // if let Some(typed) = type_res {
                //     return Ok(typed);
                // }

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
            // Maybe this shouldn't have a span
            TypeExpr::Any(_) => {
                let index = self.table.builtin_types.len();

                self.table.builtin_types.push(BuiltinType::Any(None));

                Ok(TypedId::BuiltinType(BuiltinTypeId::new(index as u32)))
            }
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
    // fn contains_func(&self, name_id: NameId) -> bool {
    //     if let Some(typed_id) = self.table.typed_ids.get(&name_id) {
    //         if let TypedId::Func(_) = typed_id {
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
    //TEST: IF ANYTHING BREAKS REVERT
    fn register_typedef(&mut self, type_def: &AbstractTypeDef, ast_id: AstId) {
        let check = self.table.name_ids.insert(ast_id, type_def.name_id);

        //NOTE: There is no scoping needed I believe so this is valid
        if check.is_some() {
            let duplicate = self.interner.search(type_def.name_id.id as usize);

            let msg = format!("The symbol \"{duplicate}\" appears more than once");
            self.reporter
                .report_spanned(&msg, None, &type_def.name_span);

            return;
        }

        let sym_id = SymbolId::new(self.table.sym_ids.len() as u32);
        self.table.sym_ids.insert(ast_id, sym_id);

        let type_def_id = self.tracker.register_typedef();

        self.table
            .typed_ids
            .insert(sym_id, TypedId::TypeDef(type_def_id));
    }

    fn register_struct(&mut self, structure: &AbstractStruct, ast_id: AstId) {
        let check = self.table.name_ids.insert(ast_id, structure.name_id);

        if check.is_some() {
            let duplicate = self.interner.search(structure.name_id.id as usize);

            let msg = format!("The symbol \"{duplicate}\" appears more than once");
            self.reporter
                .report_spanned(&msg, None, &structure.name_span);

            return;
        }

        let sym_id = SymbolId::new(self.table.sym_ids.len() as u32);
        self.table.sym_ids.insert(ast_id, sym_id);

        let struct_id = self.tracker.register_struct();

        self.table
            .typed_ids
            .insert(sym_id, TypedId::Struct(struct_id));
    }

    fn register_enum(&mut self, enumeration: &AbstractEnum, ast_id: AstId) {
        let check = self.table.name_ids.insert(ast_id, enumeration.name_id);

        if check.is_some() {
            let duplicate = self.interner.search(enumeration.name_id.id as usize);

            let msg = format!("The symbol \"{duplicate}\" appears more than once");
            self.reporter
                .report_spanned(&msg, None, &enumeration.name_span);

            return;
        }

        let sym_id = SymbolId::new(self.table.sym_ids.len() as u32);
        self.table.sym_ids.insert(ast_id, sym_id);

        let enum_id = self.tracker.register_enum();

        self.table.typed_ids.insert(sym_id, TypedId::Enum(enum_id));
    }
}

// Likely temp
#[derive(Debug)]
pub(super) struct Tracker {
    next_typedef: u32,
    next_struct: u32,
    next_func: u32,
    next_enum: u32,
}

impl Tracker {
    pub(super) fn new() -> Tracker {
        Tracker {
            next_typedef: 0,
            next_struct: 0,
            next_func: 0,
            next_enum: 0,
        }
    }
    pub(super) fn register_typedef(&mut self) -> TypeDefId {
        let type_def_id = TypeDefId::new(self.next_typedef);
        self.next_typedef += 1;
        type_def_id
    }

    pub(super) fn register_struct(&mut self) -> StructId {
        let struct_id = StructId::new(self.next_struct);
        self.next_struct += 1;
        struct_id
    }

    pub(super) fn register_func(&mut self) -> FuncId {
        let func_id = FuncId::new(self.next_func);
        self.next_func += 1;
        func_id
    }

    pub(super) fn register_enum(&mut self) -> EnumId {
        let enum_id = EnumId::new(self.next_enum);
        self.next_enum += 1;
        enum_id
    }
}
