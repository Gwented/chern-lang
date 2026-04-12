use std::collections::{HashMap, HashSet};

use common::{
    builtins::BuiltinType,
    intern::Intern,
    keywords::{self, Keyword},
    metadata::{ChernSettings, ModuleMetadata},
    reporter::diagnostic::Diagnostic,
    symbols::{AstId, InnerArgs, NameId, SymbolId, TypeId, ValueId},
};

use crate::{
    modules::{Module, Program},
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
    program: &'a mut Program,
    current_idx: usize,
    // Startup idea:
    reporter: SemanticReporter<'a>,
    //NOTE: May handle this differently but ok for now
}

impl TypeResolver<'_> {
    pub fn new<'a>(
        settings: &'a ChernSettings,
        ast_info: &'a AstInfo,
        current_idx: usize,
        interner: &'a Intern,
        program: &'a mut Program,
    ) -> TypeResolver<'a> {
        TypeResolver {
            ast_info,
            current_idx,
            reporter: SemanticReporter::new(settings, interner),
            interner,
            program,
            //TODO: This should be different
        }
    }

    //FIXME: USE A SINGULAR VECTOR INDEXED BY NAMEID LATER OVER A HASHMAP NOT NOW PLEASE NOT NOW
    // Ok. But when. I don't know.
    //TODO: Check structures of data for same name symbols
    pub fn resolve(&mut self) -> Result<(), Vec<Diagnostic>> {
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

        if !self.reporter.err_vec.is_empty() {
            let mut diags = Vec::new();
            diags.append(&mut self.reporter.err_vec);

            return Err(diags);
        }

        Ok(())
    }

    fn resolve_typedef(&mut self, abs_typedef: &AbstractTypeDef, ast_id: AstId) -> Result<(), ()> {
        let type_id = self.resolve_type_expr(&abs_typedef.spanned_ty_expr, ast_id)?;

        let module = &mut self.program.mods[self.current_idx];
        let sym_id = module.table.sym_ids[&ast_id];

        // Assinging from `Unknown` to it's actual associated type
        let type_def = module.table.get_typedef_mut(sym_id);
        type_def.type_id = type_id;

        Ok(())
    }

    fn resolve_const(&mut self, abs_const: &AbstractConst, ast_id: AstId) -> Result<(), ()> {
        todo!();
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
                    &self.program.mods[self.current_idx],
                );
            }

            seen.push((i, type_def.name_id));

            let field_repre = FieldRepre::new(type_def.name_id, type_id, AstId::new(i as u32));

            fields.push(field_repre);
        }

        let module = &mut self.program.mods[self.current_idx];
        let sym_id = module.table.sym_ids[&ast_id];

        let struct_repre = module.table.get_struct_mut(sym_id);

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
                    &self.program.mods[self.current_idx],
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

        let module = &mut self.program.mods[self.current_idx];
        let sym_id = module.table.sym_ids[&ast_id];
        let enum_repre = module.table.get_enum_mut(sym_id);

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
        dbg!(&params);
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
                let module = &self.program.mods[self.current_idx];

                for (current_ast_id, current_name_id) in &module.table.name_ids {
                    if current_name_id == name_id {
                        let sym_id = module.table.sym_ids[&current_ast_id];

                        let type_id = match &module.table.symbols[&sym_id] {
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

                if self.program.mod_map.contains_key(name_id) {
                    panic!("Containment");
                }

                let err_name = self.interner.search(name_id.id as usize);

                let err_msg = format!("\"{err_name}\" is not defined as a type");

                self.reporter.report_spanned(
                    &err_msg,
                    Some(err_name),
                    &[spanned_ty_expr.span],
                    module,
                );

                return Err(());
            }
            TypeExpr::Escaped(name_id) => {
                let module = &self.program.mods[self.current_idx];

                for (current_ast_id, current_name_id) in &module.table.name_ids {
                    if current_name_id == name_id {
                        let sym_id = module.table.sym_ids[&current_ast_id];
                        let type_id = match &module.table.symbols[&sym_id] {
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

                self.reporter
                    .report_spanned(&err_msg, None, &[spanned_ty_expr.span], &module);

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
                                    &self.program.mods[self.current_idx],
                                );

                                return Err(());
                            }

                            let inner = self.resolve_type_expr(&generic.args[0], ast_id)?;

                            let module = &mut self.program.mods[self.current_idx];

                            let list = BuiltinType::List(inner);
                            let list_id = TypeId::new(module.table.types.len() as u32);

                            module.table.types.push(Type::BuiltinType(list));

                            return Ok(list_id);
                        }
                        Keyword::Tuple => {
                            let mut elements: Vec<TypeId> = Vec::new();

                            for arg in &generic.args {
                                elements.push(self.resolve_type_expr(arg, ast_id)?);
                            }

                            let module = &mut self.program.mods[self.current_idx];

                            let type_id = TypeId::new(module.table.types.len() as u32);
                            let tuple = Tuple::new(elements, type_id);

                            module.table.types.push(Type::Tuple(tuple));

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
                                    &self.program.mods[self.current_idx],
                                );

                                return Err(());
                            }

                            let key = self.resolve_type_expr(&generic.args[0], ast_id)?;
                            let val = self.resolve_type_expr(&generic.args[1], ast_id)?;

                            let map = BuiltinType::Map(key, val);

                            let module = &mut self.program.mods[self.current_idx];

                            let map_id = module.table.types.len() as u32;

                            module.table.types.push(Type::BuiltinType(map));

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
                                    &self.program.mods[self.current_idx],
                                );

                                return Err(());
                            }

                            let inner = self.resolve_type_expr(&generic.args[0], ast_id)?;

                            let module = &mut self.program.mods[self.current_idx];

                            let set = BuiltinType::Set(inner);
                            let set_id = TypeId::new(module.table.types.len() as u32);

                            module.table.types.push(Type::BuiltinType(set));

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
                                &self.program.mods[self.current_idx],
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
                            &self.program.mods[self.current_idx],
                        );

                        Err(())
                    }
                }
            }
            TypeExpr::Any => {
                let module = &mut self.program.mods[self.current_idx];

                let id = module.table.types.len() as u32;

                module
                    .table
                    .types
                    .push(Type::BuiltinType(BuiltinType::Any(None)));

                Ok(TypeId::new(id))
            }
            TypeExpr::Tuple(unres_tuple) => {
                let mut elements: Vec<TypeId> = Vec::new();

                for element in unres_tuple {
                    let type_id = self.resolve_type_expr(element, ast_id)?;
                    elements.push(type_id);
                }

                let module = &mut self.program.mods[self.current_idx];

                let tuple_id = TypeId::new(module.table.types.len() as u32);
                let tuple = Tuple::new(elements, tuple_id);

                module.table.types.push(Type::Tuple(tuple));

                Ok(tuple_id)
            }
            TypeExpr::Path(spanned_ty_exprs) => {
                for spanned_ty_expr in spanned_ty_exprs {
                    let ty_expr = self.resolve_type_expr(spanned_ty_expr, ast_id)?;
                    panic!("resoved");
                }

                todo!();
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
}
