use std::collections::HashMap;

use chrn_utils::{
    id_types::{AstId, ExprId, InternedId, ModuleId, SymbolId, TypeId, ValueId},
    intern::Intern,
    values::ValueInfo,
};
use common::{chrn_settings::ChernSettings, reporter::diagnostic::Diagnostic};

use crate::{
    parser::ast::{
        AbstractAlias, AbstractEnum, AbstractStruct, AbstractTypeDef, AbstractVar, AstInfo, Item,
    },
    script_compiler::{self, ScriptCompiler},
    semantic::{
        representation::{
            AliasDef, EnumDef, StructDef, Symbol, SymbolKind, Type, TypeDef, TypeInfo,
        },
        semantic_reporter::SemanticReporter,
    },
};

use super::scopes::ScopeType;

pub struct NamespaceResolver<'a> {
    ast_info: &'a AstInfo,
    interner: &'a Intern,
    compiler: &'a mut ScriptCompiler,
    current_mod: ModuleId,
    reporter: SemanticReporter<'a>,
    //NOTE: May handle this differently but ok for now
}

impl NamespaceResolver<'_> {
    pub fn new<'a>(
        settings: &'a ChernSettings,
        ast_info: &'a AstInfo,
        interner: &'a Intern,
        current_mod: ModuleId,
        compiler: &'a mut ScriptCompiler,
    ) -> NamespaceResolver<'a> {
        NamespaceResolver {
            ast_info,
            interner,
            compiler,
            current_mod,
            reporter: SemanticReporter::new(settings, interner),
            //TODO: This will be different
        }
    }

    pub fn resolve(&mut self) -> Result<(), Vec<Diagnostic>> {
        // Registering namespaces
        for (id, item) in self.ast_info.items.iter().enumerate() {
            let ast_id = AstId::new(id as u32);

            match item {
                Item::TypeDef(abs_typedef) => self.register_typedef(abs_typedef, ast_id),
                Item::Struct(abs_struct) => self.register_struct(abs_struct, ast_id),
                Item::Enum(abs_enum) => self.register_enum(abs_enum, ast_id),
                Item::Alias(abs_alias) => self.register_alias(abs_alias, ast_id),
                // Stmt let
                Item::VarDecl(abs_var) => self.register_var(abs_var, ast_id),
            }
        }

        // Type specific needed
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
    fn register_typedef(&mut self, abs_typedef: &AbstractTypeDef, ast_id: AstId) {
        // This will all likely fail eventually
        let module = &mut self.compiler.mods[self.current_mod.id];
        let scope_id = module.push_scope(ScopeType::Var);
        let table = &mut module.get_scope_mut(scope_id).table;

        table.name_ids.insert(ast_id, abs_typedef.name_id);

        let sym_id = SymbolId::new(self.compiler.symbols.len() as u32);
        table.sym_ids.insert(ast_id, sym_id);

        // Promising a type will exist in the given index
        let type_id = TypeId::new(self.compiler.types.len() as u32);

        //WARN: Why was Alan Wake even here???
        let type_def_repre = TypeDef::new(sym_id, type_id);

        let symbol = Symbol::new(
            abs_typedef.name_id,
            sym_id,
            ast_id,
            self.current_mod,
            true,
            SymbolKind::Type(type_id),
        );

        self.compiler.symbols.insert(sym_id, symbol);

        let ty_info = TypeInfo::new(Type::TypeDef(type_def_repre), Some(self.current_mod));
        self.compiler.types.push(ty_info);
    }

    fn register_struct(&mut self, abs_struct: &AbstractStruct, ast_id: AstId) {
        let module = &mut self.compiler.mods[self.current_mod.id];
        let scope_id = module.push_scope(ScopeType::Nest);
        let table = &mut module.get_scope_mut(scope_id).table;

        table.name_ids.insert(ast_id, abs_struct.name_id);

        let sym_id = SymbolId::new(self.compiler.symbols.len() as u32);
        table.sym_ids.insert(ast_id, sym_id);

        let type_id = TypeId::new(self.compiler.types.len() as u32);

        let struct_def = StructDef::new(sym_id, Vec::new());

        let symbol = Symbol::new(
            abs_struct.name_id,
            sym_id,
            ast_id,
            self.current_mod,
            abs_struct.is_priv,
            SymbolKind::Type(type_id),
        );

        self.compiler.symbols.insert(sym_id, symbol);

        let ty_info = TypeInfo::new(Type::Struct(struct_def), Some(self.current_mod));
        self.compiler.types.push(ty_info);
    }

    fn register_enum(&mut self, abs_enum: &AbstractEnum, ast_id: AstId) {
        let module = &mut self.compiler.mods[self.current_mod.id];
        let scope_id = module.push_scope(ScopeType::Nest);
        let table = &mut module.get_scope_mut(scope_id).table;

        table.name_ids.insert(ast_id, abs_enum.name_id);

        let sym_id = SymbolId::new(self.compiler.symbols.len() as u32);
        let type_id = TypeId::new(self.compiler.types.len() as u32);

        table.sym_ids.insert(ast_id, sym_id);

        let enum_def = EnumDef::new(sym_id, Vec::new());

        let symbol = Symbol::new(
            abs_enum.name_id,
            sym_id,
            ast_id,
            self.current_mod,
            abs_enum.is_priv,
            SymbolKind::Type(type_id),
        );

        self.compiler.symbols.insert(sym_id, symbol);

        let ty_info = TypeInfo::new(Type::Enum(enum_def), Some(self.current_mod));
        self.compiler.types.push(ty_info);
    }

    fn register_alias(&mut self, abs_alias: &AbstractAlias, ast_id: AstId) {
        let module = &mut self.compiler.mods[self.current_mod.id];
        let scope_id = module.push_scope(ScopeType::Neutral);
        let table = &mut module.get_scope_mut(scope_id).table;

        table.name_ids.insert(ast_id, abs_alias.name_id);

        let sym_id = SymbolId::new(self.compiler.symbols.len() as u32);
        table.sym_ids.insert(ast_id, sym_id);

        let type_id = TypeId::new(self.compiler.types.len() as u32);

        let alias_def = AliasDef::new(sym_id, Vec::new(), Vec::new(), Vec::new());

        let symbol = Symbol::new(
            abs_alias.name_id,
            sym_id,
            ast_id,
            self.current_mod,
            abs_alias.is_priv,
            SymbolKind::Type(type_id),
        );

        self.compiler.symbols.insert(sym_id, symbol);

        let ty_info = TypeInfo::new(Type::Alias(alias_def), Some(self.current_mod));
        self.compiler.types.push(ty_info);
    }

    //WARN: IS THIS RIGHT?
    fn register_var(&mut self, abs_var: &AbstractVar, ast_id: AstId) {
        let module = &mut self.compiler.mods[self.current_mod.id];
        let scope_id = module.push_scope(ScopeType::Neutral);
        let table = &mut module.get_scope_mut(scope_id).table;

        table.name_ids.insert(ast_id, abs_var.name_id);

        // let type_id = TypeId::new(self.program.types.len() as u32);
        // let ty_info = TypeInfo::new(Type::Unknown, Some(self.current_mod));
        // self.program.types.push(ty_info);

        let sym_id = SymbolId::new(self.compiler.symbols.len() as u32);
        table.sym_ids.insert(ast_id, sym_id);
        // let type_id = TypeId::new(self.compiler.types.len() as u32);
        // This is pushed instead of just set just in case index level mutation as opposed to a new
        // id entirely would like to be used.
        // let ty_info = TypeInfo::new(Type::Unknown, Some(self.current_mod));
        // self.compiler.types.push(ty_info);

        //TODO: PLACEHOLDER USED EXPR ID DOESNT EXIST YET

        // No information that this is a variable other than the fact that AstId -> SymbolId
        let symbol = Symbol::new(
            abs_var.name_id,
            sym_id,
            ast_id,
            self.current_mod,
            true,
            SymbolKind::Unknown,
        );

        self.compiler.symbols.insert(sym_id, symbol);
    }

    // Cannot check for this since the type is not known
    /// Checks registered namespace for duplicates and collects errors if any are found
    fn check_duplicates(&mut self) {
        // Solely a HashMap for spanning
        let mut seen: HashMap<InternedId, AstId> = HashMap::new();

        //NOTE: Suspicious
        let module = &self.compiler.mods[self.current_mod.id];

        // Searching if there are any duplicates with respect to the scope
        for scope in &module.scopes {
            for (ast_id, name_id) in &scope.table.name_ids {
                // Why is it not true if it exists false otherwise...seems backwards
                let ast_opt = seen.insert(*name_id, *ast_id);

                // If the current name id exists in "seen"
                if let Some(orig_ast_id) = ast_opt {
                    let item = &self.ast_info.items[orig_ast_id.id as usize];
                    let orig_span = match item {
                        Item::TypeDef(abs_typedef) => &abs_typedef.name_span,
                        Item::Struct(abs_struct) => &abs_struct.name_span,
                        Item::Enum(abs_enum) => &abs_enum.name_span,
                        Item::Alias(abs_alias) => &abs_alias.name_span,
                        Item::VarDecl(abs_var) => &abs_var.name_span,
                    }
                    .clone();

                    let dup_span = match &self.ast_info.items[ast_id.id as usize] {
                        Item::TypeDef(abs_typedef) => &abs_typedef.name_span,
                        Item::Struct(abs_struct) => &abs_struct.name_span,
                        Item::Enum(abs_enum) => &abs_enum.name_span,
                        Item::Alias(abs_alias) => &abs_alias.name_span,
                        Item::VarDecl(abs_var) => &abs_var.name_span,
                    }
                    .clone();

                    let dup_name = self.interner.search(name_id.id as usize);

                    let msg = format!(
                        "Found more than one symbol with identifier \"{dup_name}\" in the section `{}`",
                        scope.scope_type
                    );

                    self.reporter.report_spanned(
                        &msg,
                        None,
                        &[orig_span, dup_span],
                        &self.compiler.mods[self.current_mod.id],
                    );
                }
            }

            // Clearing after finishing one table
            // More suspicious
            seen.clear();
        }
    }
}
