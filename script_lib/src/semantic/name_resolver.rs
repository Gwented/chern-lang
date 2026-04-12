use std::collections::HashMap;

use common::{
    builtins::BuiltinType,
    intern::Intern,
    keywords::Keyword,
    metadata::ChernSettings,
    reporter::diagnostic::Diagnostic,
    symbols::{AstId, ModuleId, NameId, SymbolId, TypeId, ValueId},
};

use crate::{
    modules::{Module, Program},
    parser::ast::{
        AbstractAlias, AbstractConst, AbstractEnum, AbstractStruct, AbstractTypeDef, AstInfo,
        Import, Item, SpannedTypeExpr, TypeExpr,
    },
    semantic::{
        representation::{
            AliasRepre, ConstRepre, EnumRepre, FieldRepre, StructRepre, Symbol, SymbolInfo, Tuple,
            Type, TypeDefRepre, TypeInfo, VariantRepre,
        },
        semantic_reporter::SemanticReporter,
    },
};

pub struct NamespaceResolver<'a> {
    ast_info: &'a AstInfo,
    interner: &'a Intern,
    //WARN: Horrors
    program: &'a mut Program,
    // HELP
    current_mod: ModuleId,
    reporter: SemanticReporter<'a>,
    //NOTE: May handle this differently but ok for now
    unknown_id: Option<TypeId>,
}

impl NamespaceResolver<'_> {
    pub fn new<'a>(
        settings: &'a ChernSettings,
        ast_info: &'a AstInfo,
        interner: &'a Intern,
        current_mod: ModuleId,
        program: &'a mut Program,
    ) -> NamespaceResolver<'a> {
        NamespaceResolver {
            ast_info,
            interner,
            program,
            current_mod,
            reporter: SemanticReporter::new(settings, interner),
            //TODO: This will be different
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

        // Collecting possible same symbol errors
        self.check_duplicates();

        if !self.reporter.err_vec.is_empty() {
            let mut diags = Vec::new();
            diags.append(&mut self.reporter.err_vec);

            return Err(diags);
        }

        Ok(())
    }

    /// Attaches ast_id to the name_id of it's ast structure.
    /// Gives it a unique symbol id and attaches the ast id to it.
    /// Gives the typedef an id attached to `Unknown` which is to be resolved later
    /// Registers the unfinished representation with it's symbol id so that it can still be
    /// referenced
    fn register_typedef(&mut self, type_def: &AbstractTypeDef, ast_id: AstId) {
        let module = &mut self.program.mods[self.current_mod.id];
        module.table.name_ids.insert(ast_id, type_def.name_id);

        let sym_id = SymbolId::new(self.program.symbols.len() as u32);
        module.table.sym_ids.insert(ast_id, sym_id);

        //WARN: This type id is fake...
        let type_id = if let Some(id) = self.unknown_id {
            id
        } else {
            let id = TypeId::new(self.program.types.len() as u32);
            self.unknown_id = Some(id);
            let ty_info = TypeInfo::new(Type::Unknown, Some(self.current_mod));
            self.program.types.push(ty_info);

            id
        };

        let type_def_repre = TypeDefRepre::new(type_def.name_id, type_id, sym_id, ast_id);

        let sym_info = SymbolInfo::new(Symbol::TypeDef(type_def_repre), self.current_mod);
        self.program.symbols.insert(sym_id, sym_info);
    }

    fn register_struct(&mut self, abs_struct: &AbstractStruct, ast_id: AstId) {
        let module = &mut self.program.mods[self.current_mod.id];
        module.table.name_ids.insert(ast_id, abs_struct.name_id);

        let sym_id = SymbolId::new(self.program.symbols.len() as u32);
        module.table.sym_ids.insert(ast_id, sym_id);

        let type_id = TypeId::new(self.program.types.len() as u32);

        let struct_repre =
            StructRepre::new(abs_struct.name_id, sym_id, ast_id, type_id, Vec::new());

        let sym_info = SymbolInfo::new(Symbol::Struct(struct_repre), self.current_mod);
        self.program.symbols.insert(sym_id, sym_info);

        let ty_info = TypeInfo::new(Type::Struct(sym_id), Some(self.current_mod));
        self.program.types.push(ty_info);
    }

    fn register_enum(&mut self, abs_enum: &AbstractEnum, ast_id: AstId) {
        let module = &mut self.program.mods[self.current_mod.id];
        module.table.name_ids.insert(ast_id, abs_enum.name_id);

        let sym_id = SymbolId::new(self.program.symbols.len() as u32);
        let type_id = TypeId::new(self.program.types.len() as u32);

        module.table.sym_ids.insert(ast_id, sym_id);

        let enum_repre = EnumRepre::new(abs_enum.name_id, sym_id, ast_id, type_id, Vec::new());

        let sym_info = SymbolInfo::new(Symbol::Enum(enum_repre), self.current_mod);
        self.program.symbols.insert(sym_id, sym_info);

        let ty_info = TypeInfo::new(Type::Enum(sym_id), Some(self.current_mod));
        self.program.types.push(ty_info);
    }

    fn register_alias(&mut self, abs_alias: &AbstractAlias, ast_id: AstId) {
        let module = &mut self.program.mods[self.current_mod.id];
        module.table.name_ids.insert(ast_id, abs_alias.name_id);

        let sym_id = SymbolId::new(self.program.symbols.len() as u32);
        module.table.sym_ids.insert(ast_id, sym_id);

        // Unkown type id for unregistered types
        // FIX:
        let type_id = if let Some(id) = self.unknown_id {
            id
        } else {
            let id = TypeId::new(self.program.types.len() as u32);
            self.unknown_id = Some(id);

            let ty_info = TypeInfo::new(Type::Unknown, Some(self.current_mod));
            self.program.types.push(ty_info);

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

        let sym_info = SymbolInfo::new(Symbol::Alias(alias_repre), self.current_mod);
        self.program.symbols.insert(sym_id, sym_info);

        let ty_info = TypeInfo::new(Type::Alias(sym_id), Some(self.current_mod));
        self.program.types.push(ty_info);
    }

    //WARN: IS THIS RIGHT?
    fn register_const(&mut self, abs_const: &AbstractConst, ast_id: AstId) {
        let module = &mut self.program.mods[self.current_mod.id];
        module.table.name_ids.insert(ast_id, abs_const.name_id);

        let type_id = if let Some(id) = self.unknown_id {
            id
        } else {
            let id = TypeId::new(self.program.types.len() as u32);
            self.unknown_id = Some(id);

            let ty_info = TypeInfo::new(Type::Unknown, Some(self.current_mod));
            self.program.types.push(ty_info);

            id
        };

        let sym_id = SymbolId::new(self.program.symbols.len() as u32);
        module.table.sym_ids.insert(ast_id, sym_id);

        let const_repre =
            ConstRepre::new(abs_const.name_id, sym_id, ast_id, type_id, ValueId::new(0));

        let sym_info = SymbolInfo::new(Symbol::Const(const_repre), self.current_mod);
        self.program.symbols.insert(sym_id, sym_info);

        let ty_info = TypeInfo::new(Type::Const(sym_id), Some(self.current_mod));
        self.program.types.push(ty_info);
    }

    /// Checks registered namespace for duplicates and collects errors if any are found
    fn check_duplicates(&mut self) {
        // Solely a HashMap for spanning
        let mut seen: HashMap<NameId, AstId> = HashMap::new();

        let module = &self.program.mods[self.current_mod.id];
        for (ast_id, name_id) in &module.table.name_ids {
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
                    &self.program.mods[self.current_mod.id],
                );
            }
        }
    }
}

