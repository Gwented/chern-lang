use std::collections::{HashMap, HashSet};

use common::{
    builtins::BuiltinType,
    intern::Intern,
    keywords::{self, Keyword},
    metadata::{ChernSettings, ModuleMetadata},
    reporter::diagnostic::Diagnostic,
    symbols::{
        AstId, BuiltinTypeId, EnumId, FuncId, InnerArgs, NameId, Span, SpannedInnerArgs, StructId,
        SymbolId, TypeDefId, TypeId,
    },
};

use crate::{
    modules::Module,
    parser::ast::{
        AbstractAlias, AbstractConst, AbstractEnum, AbstractStruct, AbstractTypeDef, AstInfo, Expr,
        Import, Item, SpannedTypeExpr, TypeExpr, UnaryOp,
    },
    semantic::{
        representation::{
            AliasRepre, ConstRepre, EnumRepre, FieldRepre, FuncArgsRepre, FuncRepre, StructRepre,
            Symbol, Tuple, Type, TypeDefRepre, VariantRepre,
        },
        semantic_reporter::SemanticReporter,
    },
};
/// Does a namespace and type resolution, not including conditions, arguments or any other form
/// of validation. NamespaceResolver + TypeResolver
pub struct TypeResolver<'a> {
    ast_info: &'a AstInfo,
    interner: &'a Intern,
    //WARN: Horrors
    module: &'a mut Module,
    // Startup idea:
    reporter: SemanticReporter<'a>,
    //NOTE: May handle this differently but ok for now
    unknown_id: Option<TypeId>,
}

impl TypeResolver<'_> {
    pub fn new<'a>(
        settings: &'a ChernSettings,
        ast_info: &'a AstInfo,
        interner: &'a Intern,
        module: &'a mut Module,
    ) -> TypeResolver<'a> {
        TypeResolver {
            ast_info,
            interner,
            module,
            reporter: SemanticReporter::new(settings),
            //TODO: This should be different
            unknown_id: None,
        }
    }

    //FIXME: USE A SINGULAR VECTOR INDEXED BY NAMEID LATER OVER A HASHMAP NOT NOW PLEASE NOT NOW
    // Ok. But when. I don't know.
    //TODO: Check structures of data for same name symbols
    pub fn resolve(&mut self) -> Result<(), Vec<Diagnostic>> {
        // Registering namespaces
        for (id, item) in self.ast_info.items.iter().enumerate() {
            let ast_id = AstId::new(id as u32);

            match item {
                Item::Var(abs_typedef) => self.register_typedef(abs_typedef, ast_id),
                Item::Struct(abs_struct) => self.register_struct(abs_struct, ast_id),
                Item::Enum(abs_enum) => self.register_enum(abs_enum, ast_id),
                Item::Alias(abs_alias) => self.register_alias(abs_alias, ast_id),
                Item::Const(abs_const) => self.register_const(abs_const, ast_id),
                // Maybe imports outside of this should be stored separately
            }
        }

        self.check_duplicates();

        //FIXME: Check symbols here once

        if !self.reporter.err_vec.is_empty() {
            let mut diags = Vec::new();
            diags.append(&mut self.reporter.err_vec);

            return Err(diags);
        }

        // This is resolving types but not resolving args or conditions.
        // Everything is in order so this cannot fail unless something internally went wrong.
        for (id, item) in self.ast_info.items.iter().enumerate() {
            let ast_id = AstId::new(id as u32);

            match item {
                Item::Var(abs_typedef) => _ = self.resolve_typedef(abs_typedef, ast_id),
                Item::Struct(abs_struct) => _ = self.resolve_struct(abs_struct, ast_id),
                Item::Enum(abs_enum) => _ = self.resolve_enum(abs_enum, ast_id),
                Item::Alias(abs_alias) => _ = self.resolve_alias(abs_alias, ast_id),
                Item::Const(abs_const) => _ = self.resolve_const(abs_const, ast_id),
            }
        }

        // Collecting possible same symbol errors

        if !self.reporter.err_vec.is_empty() {
            let mut diags = Vec::new();
            diags.append(&mut self.reporter.err_vec);

            return Err(diags);
        }

        Ok(())
    }

    fn resolve_typedef(&mut self, abs_typedef: &AbstractTypeDef, ast_id: AstId) -> Result<(), ()> {
        let type_id = self.resolve_type_expr(&abs_typedef.spanned_ty_expr, ast_id)?;

        let sym_id = self.module.table.sym_ids[&ast_id];

        // Assinging from `Unknown` to it's actual associated type
        let type_def = self.module.table.get_typedef_mut(sym_id);
        type_def.type_id = type_id;

        Ok(())
    }

    fn resolve_struct(&mut self, abs_struct: &AbstractStruct, ast_id: AstId) -> Result<(), ()> {
        let mut fields: Vec<FieldRepre> = Vec::new();

        let mut seen: Vec<(usize, NameId)> = Vec::new();

        // Checking if there are duplicate name ids within the same struct along with resolution
        for (i, type_def) in abs_struct.fields.iter().enumerate() {
            let type_id = self.resolve_type_expr(&type_def.spanned_ty_expr, ast_id)?;

            if let Some(original) = seen.iter().find(|other| type_def.name_id == other.1) {
                let struct_name = self.interner.search(abs_struct.name_id.id as usize);
                let dup_name = self.interner.search(type_def.name_id.id as usize);

                let orig_span = abs_struct.fields[original.0].name_span;
                let field_span = abs_struct.fields[i].name_span;

                let msg = format!(
                    "More than one field has the identifier \"{dup_name}\" within struct \"{struct_name}\""
                );

                self.reporter.report_spanned(
                    &msg,
                    None,
                    &[orig_span, field_span],
                    &self.module.metadata,
                );
            }

            seen.push((i, type_def.name_id));

            let field_repre = FieldRepre::new(type_def.name_id, type_id, AstId::new(i as u32));

            fields.push(field_repre);
        }

        let sym_id = self.module.table.sym_ids[&ast_id];

        let struct_repre = self.module.table.get_struct_mut(sym_id);

        struct_repre.fields.append(&mut fields);

        Ok(())
    }

    fn resolve_enum(&mut self, abs_enum: &AbstractEnum, ast_id: AstId) -> Result<(), ()> {
        let mut variants: Vec<VariantRepre> = Vec::new();

        // (ast_id, name_id)
        let mut seen: Vec<(usize, NameId)> = Vec::new();
        //Maybe just compute this once after along with struct fields

        // Checking if there are duplicate name ids within the same enum
        for (i, variant) in abs_enum.variants.iter().enumerate() {
            if let Some(original) = seen.iter().find(|other| variant.name_id == other.1) {
                let enum_name = self.interner.search(abs_enum.name_id.id as usize);
                let dup_name = self.interner.search(variant.name_id.id as usize);

                let orig_span = abs_enum.variants[original.0].name_span;
                let variant_span = abs_enum.variants[i].name_span;

                let msg = format!(
                    "More than one variant has the identifier \"{dup_name}\" within enum \"{enum_name}\""
                );

                self.reporter.report_spanned(
                    &msg,
                    None,
                    &[orig_span, variant_span],
                    &self.module.metadata,
                );
            }

            seen.push((i, variant.name_id));

            if let Some(spanned_ty_expr) = &variant.ty_expr {
                let type_id = self.resolve_type_expr(&spanned_ty_expr, ast_id)?;
                let variant_repre =
                    VariantRepre::new(variant.name_id, Some(type_id), AstId::new(i as u32));

                variants.push(variant_repre);
            }
        }

        let sym_id = self.module.table.sym_ids[&ast_id];

        let enum_repre = self.module.table.get_enum_mut(sym_id);

        enum_repre.variants.append(&mut variants);

        Ok(())
    }

    fn resolve_alias(&mut self, abs_alias: &AbstractAlias, ast_id: AstId) -> Result<(), ()> {
        // Should the variable check happen here?
        let mut params: Vec<TypeId> = Vec::new();
        for (i, spanned_ty_expr) in abs_alias.params.iter().enumerate() {
            let type_id = self.resolve_type_expr(&spanned_ty_expr, ast_id)?;
            params.push(type_id);
        }
        todo!();
    }

    fn resolve_const(&mut self, abs_const: &AbstractConst, ast_id: AstId) -> Result<(), ()> {
        todo!();
    }

    fn resolve_type_expr(
        &mut self,
        spanned_ty_expr: &SpannedTypeExpr,
        ast_id: AstId,
    ) -> Result<TypeId, ()> {
        match &spanned_ty_expr.ty_expr {
            TypeExpr::Var(name_id) => {
                // Returns the name's id since it is a valid non-data structure intrinsic type
                if let Some(_) = BuiltinType::try_from_id(name_id.id) {
                    return Ok(TypeId::new(name_id.id));
                }

                // Loop that checks if the name id was registered, then uses its corresponding ast_id to
                // extract the name id's type and returns that as the type to be referenced
                for (current_ast_id, current_name_id) in &self.module.table.name_ids {
                    if current_name_id == name_id {
                        let sym_id = self.module.table.sym_ids[&current_ast_id];
                        let type_id = match &self.module.table.symbols[&sym_id] {
                            Symbol::Struct(struct_repre) => struct_repre.type_id,
                            Symbol::Func(func_repre) => func_repre.type_id,
                            Symbol::Enum(enum_repre) => enum_repre.type_id,
                            // This is not possible
                            // Symbol::TypeDef(type_def_repre) => type_def_repre.type_id,
                            _ => unreachable!("Typedefs are unknown by default"),
                        };

                        return Ok(type_id);
                    }
                }

                let err_name = self.interner.search(name_id.id as usize);

                let err_msg = format!("\"{err_name}\" is not defined as a type");

                self.reporter.report_spanned(
                    &err_msg,
                    Some(err_name),
                    &[spanned_ty_expr.span],
                    &self.module.metadata,
                );

                return Err(());
            }
            // Needs to be merged in a sensible way
            TypeExpr::Escaped(name_id) => {
                for (current_ast_id, current_name_id) in &self.module.table.name_ids {
                    if current_name_id == name_id {
                        let sym_id = self.module.table.sym_ids[&current_ast_id];
                        let type_id = match &self.module.table.symbols[&sym_id] {
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

                self.reporter.report_spanned(
                    &err_msg,
                    None,
                    &[spanned_ty_expr.span],
                    &self.module.metadata,
                );

                return Err(());
            }
            TypeExpr::Generic(generic) => {
                match Keyword::try_as_kw(generic.base.id) {
                    // Self referential type ids used here
                    Some(kw) => match kw {
                        //TODO: Should maybe put List | Set
                        Keyword::List => {
                            if generic.args.len() != 1 {
                                let msg = format!(
                                    "Expected 1 type within `List`, found {}",
                                    generic.args.len()
                                );

                                self.reporter.report_spanned(
                                    &msg,
                                    None,
                                    &[spanned_ty_expr.span],
                                    &self.module.metadata,
                                );

                                return Err(());
                            }

                            let inner = self.resolve_type_expr(&generic.args[0], ast_id)?;

                            let list = BuiltinType::List(inner);
                            let list_id = TypeId::new(self.module.table.types.len() as u32);

                            self.module.table.types.push(Type::BuiltinType(list));

                            return Ok(list_id);
                        }
                        Keyword::Tuple => {
                            let mut elements: Vec<TypeId> = Vec::new();

                            for arg in &generic.args {
                                elements.push(self.resolve_type_expr(arg, ast_id)?);
                            }

                            let type_id = TypeId::new(self.module.table.types.len() as u32);
                            let tuple = Tuple::new(elements, type_id);

                            self.module.table.types.push(Type::Tuple(tuple));

                            Ok(type_id)
                        }
                        Keyword::Map => {
                            if generic.args.len() != 2 {
                                let msg = format!(
                                    "Expected 2 types within `Map`, found {}",
                                    generic.args.len()
                                );

                                self.reporter.report_spanned(
                                    &msg,
                                    None,
                                    &[spanned_ty_expr.span],
                                    &self.module.metadata,
                                );

                                return Err(());
                            }

                            let key = self.resolve_type_expr(&generic.args[0], ast_id)?;
                            let val = self.resolve_type_expr(&generic.args[1], ast_id)?;

                            let map = BuiltinType::Map(key, val);

                            let map_id = self.module.table.types.len() as u32;

                            self.module.table.types.push(Type::BuiltinType(map));

                            Ok(TypeId::new(map_id))
                        }
                        // Should probably just put this with list
                        Keyword::Set => {
                            if generic.args.len() != 1 {
                                let msg = format!(
                                    "Expected 1 type within `Set`, found {}",
                                    generic.args.len()
                                );

                                self.reporter.report_spanned(
                                    &msg,
                                    None,
                                    &[spanned_ty_expr.span],
                                    &self.module.metadata,
                                );

                                return Err(());
                            }

                            let inner = self.resolve_type_expr(&generic.args[0], ast_id)?;

                            let set = BuiltinType::Set(inner);
                            let set_id = TypeId::new(self.module.table.types.len() as u32);

                            self.module.table.types.push(Type::BuiltinType(set));

                            return Ok(set_id);
                        }
                        // I'm sure this can be done better...
                        _ => {
                            let err_name = self.interner.search(generic.base.id as usize);
                            //WARN: Questionablly phrased error message
                            //This COULD change so this will not be upheld at the parsing stage
                            let err_msg = format!(
                                "Found identifier \"{err_name}\" before generic parameters, but only `List`, `Set`, and `Map` are valid data structures"
                            );

                            self.reporter.report_spanned(
                                &err_msg,
                                Some(err_name),
                                &[spanned_ty_expr.span],
                                &self.module.metadata,
                            );

                            Err(())
                        }
                    },
                    None => {
                        // 2004 dog 2004 television
                        let err_name = self.interner.search(generic.base.id as usize);

                        let err_msg = format!(
                            "Found identifier \"{err_name}\" before generic parameters, but only `List`, `Set`, and `Map` are valid data structures"
                        );

                        self.reporter.report_spanned(
                            &err_msg,
                            Some(err_name),
                            &[spanned_ty_expr.span],
                            &self.module.metadata,
                        );

                        Err(())
                    }
                }
            }
            TypeExpr::Any => {
                let id = self.module.table.types.len() as u32;

                self.module
                    .table
                    .types
                    .push(Type::BuiltinType(BuiltinType::Any(None)));

                Ok(TypeId::new(id))
            }
            // If a semantic error was returned I could control when things are reported by
            // intercepting
            TypeExpr::Tuple(unres_tuple) => {
                let mut elements: Vec<TypeId> = Vec::new();

                for element in unres_tuple {
                    let type_id = self.resolve_type_expr(element, ast_id)?;
                    elements.push(type_id);
                }

                let tuple_id = TypeId::new(self.module.table.types.len() as u32);
                let tuple = Tuple::new(elements, tuple_id);

                self.module.table.types.push(Type::Tuple(tuple));

                Ok(tuple_id)
            }
        }
    }

    /// Checks registered namespace for duplicates and collects errors if any are found
    fn check_duplicates(&mut self) {
        // Solely a HashMap for spanning
        let mut seen: HashMap<NameId, AstId> = HashMap::new();

        for (ast_id, name_id) in &self.module.table.name_ids {
            // Why is it not true if it exists false otherwise...seems backwards
            let ast_opt = seen.insert(*name_id, *ast_id);

            if let Some(orig_ast_id) = ast_opt {
                let item = &self.ast_info.items[orig_ast_id.id as usize];
                let orig_span = match item {
                    Item::Var(abs_typedef) => &abs_typedef.name_span,
                    Item::Struct(abs_struct) => &abs_struct.name_span,
                    Item::Enum(abs_enum) => &abs_enum.name_span,
                    Item::Alias(abs_alias) => &abs_alias.name_span,
                    Item::Const(abs_const) => &abs_const.name_span,
                }
                .clone();

                let dup_span = match &self.ast_info.items[ast_id.id as usize] {
                    Item::Var(abs_typedef) => &abs_typedef.name_span,
                    Item::Struct(abs_struct) => &abs_struct.name_span,
                    Item::Enum(abs_enum) => &abs_enum.name_span,
                    Item::Alias(abs_alias) => &abs_alias.name_span,
                    Item::Const(abs_const) => &abs_const.name_span,
                }
                .clone();

                let dup_name = self.interner.search(name_id.id as usize);

                let msg = format!(
                    "Found more than one symbol with identifier \"{dup_name}\" in the same scope"
                );

                self.reporter.report_spanned(
                    &msg,
                    None,
                    &[orig_span, dup_span],
                    &self.module.metadata,
                );
            }
        }
    }

    // How do we solve this?
    // I DONT KNOW
    fn resolve_expr(&mut self, expr: &Expr) -> Result<TypeId, ()> {
        match expr {
            Expr::Var(name_id) => todo!(),
            Expr::Integer(num) => todo!(),
            Expr::Float(num) => todo!(),
            Expr::Str(name_id) => todo!(),
            Expr::Call(_, _) => todo!(),
            Expr::FieldAccess(abs_field_access) => todo!(),
            Expr::Unary(unary) => todo!(),
            Expr::BinaryExpr { lhs, op, rhs } => todo!(),
            Expr::Char(_) => todo!(),
            Expr::Default(_, expr) => todo!(),
        }
    }

    /// Attaches ast_id to the name_id of it's ast structure.
    /// Gives it a unique symbol id and attaches the ast id to it.
    /// Gives the typedef an id attached to `Unknown` which is to be resolved later
    /// Registers the unfinished representation with it's symbol id so that it can still be
    /// referenced

    fn register_typedef(&mut self, type_def: &AbstractTypeDef, ast_id: AstId) {
        self.module.table.name_ids.insert(ast_id, type_def.name_id);

        let sym_id = SymbolId::new(self.module.table.sym_ids.len() as u32);
        self.module.table.sym_ids.insert(ast_id, sym_id);

        //WARN: This type id is fake...
        let type_id = if let Some(id) = self.unknown_id {
            id
        } else {
            let id = TypeId::new(self.module.table.types.len() as u32);
            self.unknown_id = Some(id);
            self.module.table.types.push(Type::Unknown);

            id
        };

        let type_def_repre = TypeDefRepre::new(type_def.name_id, type_id, sym_id, ast_id);

        self.module
            .table
            .symbols
            .insert(sym_id, Symbol::TypeDef(type_def_repre));
    }

    fn register_struct(&mut self, abs_struct: &AbstractStruct, ast_id: AstId) {
        self.module
            .table
            .name_ids
            .insert(ast_id, abs_struct.name_id);

        let sym_id = SymbolId::new(self.module.table.sym_ids.len() as u32);
        self.module.table.sym_ids.insert(ast_id, sym_id);

        let type_id = TypeId::new(self.module.table.types.len() as u32);

        let struct_repre =
            StructRepre::new(abs_struct.name_id, sym_id, ast_id, type_id, Vec::new());

        self.module
            .table
            .symbols
            .insert(sym_id, Symbol::Struct(struct_repre));

        self.module.table.types.push(Type::Struct(sym_id));
    }

    fn register_enum(&mut self, abs_enum: &AbstractEnum, ast_id: AstId) {
        self.module.table.name_ids.insert(ast_id, abs_enum.name_id);

        let type_id = TypeId::new(self.module.table.types.len() as u32);

        let sym_id = SymbolId::new(self.module.table.sym_ids.len() as u32);

        self.module.table.sym_ids.insert(ast_id, sym_id);

        let enum_repre = EnumRepre::new(abs_enum.name_id, sym_id, ast_id, type_id, Vec::new());
        self.module
            .table
            .symbols
            .insert(sym_id, Symbol::Enum(enum_repre));

        self.module.table.types.push(Type::Enum(sym_id));
    }

    fn register_alias(&mut self, abs_alias: &AbstractAlias, ast_id: AstId) {
        self.module.table.name_ids.insert(ast_id, abs_alias.name_id);

        let sym_id = SymbolId::new(self.module.table.sym_ids.len() as u32);
        self.module.table.sym_ids.insert(ast_id, sym_id);

        //Unkown type id for unregistered types
        let type_id = if let Some(id) = self.unknown_id {
            id
        } else {
            let id = TypeId::new(self.module.table.types.len() as u32);
            self.unknown_id = Some(id);
            self.module.table.types.push(Type::Unknown);

            id
        };

        let alias_repre = AliasRepre::new(
            abs_alias.name_id,
            sym_id,
            ast_id,
            type_id,
            Vec::new(),
            Vec::new(),
            Vec::new(),
        );

        self.module
            .table
            .symbols
            .insert(sym_id, Symbol::Alias(alias_repre));

        self.module.table.types.push(Type::Alias(sym_id));
    }

    //WARN: IS THIS RIGHT?
    fn register_const(&mut self, abs_const: &AbstractConst, ast_id: AstId) {
        self.module.table.name_ids.insert(ast_id, abs_const.name_id);

        let type_id = if let Some(id) = self.unknown_id {
            id
        } else {
            let id = TypeId::new(self.module.table.types.len() as u32);
            self.unknown_id = Some(id);
            self.module.table.types.push(Type::Unknown);

            id
        };

        let sym_id = SymbolId::new(self.module.table.sym_ids.len() as u32);

        self.module.table.sym_ids.insert(ast_id, sym_id);

        let const_repre = ConstRepre::new(abs_const.name_id, sym_id, ast_id, type_id);

        self.module
            .table
            .symbols
            .insert(sym_id, Symbol::Const(const_repre));

        self.module.table.types.push(Type::Const(sym_id));
    }

    fn register_import(&mut self, abs_import: &Import, ast_id: AstId) {
        // self.table.name_ids.insert(ast_id, abs_const.name_id);
        //
        // let type_id = if let Some(id) = self.unknown_id {
        //     id
        // } else {
        //     let id = TypeId::new(self.table.types.len() as u32);
        //     self.unknown_id = Some(id);
        //     self.table.types.push(Type::Unknown);
        //
        //     id
        // };
        //
        // let sym_id = SymbolId::new(self.table.sym_ids.len() as u32);
        //
        // self.table.sym_ids.insert(ast_id, sym_id);
        //
        // let const_repre = ConstRepre::new(abs_const.name_id, sym_id, ast_id, type_id);
        //
        // self.table
        //     .symbols
        //     .insert(sym_id, Symbol::Const(const_repre));
        //
        // self.table.types.push(Type::Const(sym_id));
        todo!("Import not done");
    }
}
