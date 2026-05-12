use std::collections::HashMap;

use chrn_utils::{
    id_types::{AstId, InternedId, ModuleId, SymbolId, TypeId},
    intern::Intern,
};
use common::{chrn_settings::ChrnSettings, reporter::diagnostic::Diagnostic};

use crate::{
    parser::ast::{
        AbstractAlias, AbstractEnum, AbstractStruct, AbstractTypeDef, AbstractVar, AstInfo, Item,
    },
    script_compiler::ScriptCompiler,
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
        settings: &'a ChrnSettings,
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
                Item::Var(abs_var) => self.register_var(abs_var, ast_id),
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
        let scope_id = self.compiler.push_scope(ScopeType::Var, self.current_mod);
        let sym_id = SymbolId::new(self.compiler.symbols.len() as u32);

        let table = &mut self.compiler.get_scope_mut(scope_id).scope.table;

        table.ast_to_interned.insert(ast_id, abs_typedef.name_id);
        table.ast_to_sym.insert(ast_id, sym_id);
        table.interned_to_sym.insert(abs_typedef.name_id, sym_id);

        // Promising a type will exist in the given index
        let type_id = TypeId::new(self.compiler.types.len() as u32);

        //WARN: Why was Alan Wake even here???
        let type_def_repre = TypeDef::new(sym_id, type_id);

        let symbol = Symbol::new(
            abs_typedef.name_id,
            sym_id,
            Some(ast_id),
            self.current_mod,
            true,
            ScopeType::Var,
            SymbolKind::Type(type_id),
        );

        self.compiler.symbols.push(symbol);

        let ty_info = TypeInfo::new(Type::TypeDef(type_def_repre), self.current_mod);
        self.compiler.types.push(ty_info);
    }

    fn register_struct(&mut self, abs_struct: &AbstractStruct, ast_id: AstId) {
        let sym_id = SymbolId::new(self.compiler.symbols.len() as u32);
        let scope_id = self.compiler.push_scope(ScopeType::Nest, self.current_mod);
        let table = &mut self.compiler.get_scope_mut(scope_id).scope.table;

        table.ast_to_interned.insert(ast_id, abs_struct.name_id);
        table.ast_to_sym.insert(ast_id, sym_id);
        table.interned_to_sym.insert(abs_struct.name_id, sym_id);

        if !abs_struct.is_priv {
            let module = &mut self.compiler.mods[self.current_mod.id];
            module.exports.push(sym_id);
        }

        let type_id = TypeId::new(self.compiler.types.len() as u32);

        let struct_def = StructDef::new(sym_id, Vec::new());

        let symbol = Symbol::new(
            abs_struct.name_id,
            sym_id,
            Some(ast_id),
            self.current_mod,
            abs_struct.is_priv,
            ScopeType::Nest,
            SymbolKind::Type(type_id),
        );

        self.compiler.symbols.push(symbol);

        let ty_info = TypeInfo::new(Type::Struct(struct_def), self.current_mod);
        self.compiler.types.push(ty_info);
    }

    fn register_enum(&mut self, abs_enum: &AbstractEnum, ast_id: AstId) {
        let scope_id = self.compiler.push_scope(ScopeType::Nest, self.current_mod);
        let sym_id = SymbolId::new(self.compiler.symbols.len() as u32);
        let type_id = TypeId::new(self.compiler.types.len() as u32);

        let table = &mut self.compiler.get_scope_mut(scope_id).scope.table;

        table.ast_to_interned.insert(ast_id, abs_enum.name_id);
        table.ast_to_sym.insert(ast_id, sym_id);
        table.interned_to_sym.insert(abs_enum.name_id, sym_id);

        if !abs_enum.is_priv {
            let module = &mut self.compiler.mods[self.current_mod.id];
            module.exports.push(sym_id);
        }

        let enum_def = EnumDef::new(sym_id, Vec::new());

        let symbol = Symbol::new(
            abs_enum.name_id,
            sym_id,
            Some(ast_id),
            self.current_mod,
            abs_enum.is_priv,
            ScopeType::Nest,
            SymbolKind::Type(type_id),
        );

        self.compiler.symbols.push(symbol);

        let ty_info = TypeInfo::new(Type::Enum(enum_def), self.current_mod);
        self.compiler.types.push(ty_info);
    }

    fn register_alias(&mut self, abs_alias: &AbstractAlias, ast_id: AstId) {
        let scope_id = self
            .compiler
            .push_scope(ScopeType::Neutral, self.current_mod);
        let sym_id = SymbolId::new(self.compiler.symbols.len() as u32);
        let type_id = TypeId::new(self.compiler.types.len() as u32);

        let table = &mut self.compiler.get_scope_mut(scope_id).scope.table;

        table.ast_to_interned.insert(ast_id, abs_alias.name_id);
        table.ast_to_sym.insert(ast_id, sym_id);
        table.interned_to_sym.insert(abs_alias.name_id, sym_id);

        if !abs_alias.is_priv {
            let module = &mut self.compiler.mods[self.current_mod.id];
            module.exports.push(sym_id);
        }

        let alias_def = AliasDef::new(sym_id, Vec::new(), Vec::new(), Vec::new());

        let symbol = Symbol::new(
            abs_alias.name_id,
            sym_id,
            Some(ast_id),
            self.current_mod,
            abs_alias.is_priv,
            ScopeType::Neutral,
            SymbolKind::Type(type_id),
        );

        self.compiler.symbols.push(symbol);

        let ty_info = TypeInfo::new(Type::Alias(alias_def), self.current_mod);
        self.compiler.types.push(ty_info);
    }

    fn register_var(&mut self, abs_var: &AbstractVar, ast_id: AstId) {
        let sym_id = SymbolId::new(self.compiler.symbols.len() as u32);
        let scope_id = self
            .compiler
            .push_scope(ScopeType::Neutral, self.current_mod);
        let table = &mut self.compiler.get_scope_mut(scope_id).scope.table;

        table.ast_to_interned.insert(ast_id, abs_var.name_id);
        table.ast_to_sym.insert(ast_id, sym_id);
        table.interned_to_sym.insert(abs_var.name_id, sym_id);

        if !abs_var.is_priv {
            let module = &mut self.compiler.mods[self.current_mod.id];
            module.exports.push(sym_id);
        }

        //TODO: PLACEHOLDER USED EXPR ID DOESNT EXIST YET

        // No information that this is a variable other than the fact that AstId -> SymbolId
        let symbol = Symbol::new(
            abs_var.name_id,
            sym_id,
            Some(ast_id),
            self.current_mod,
            abs_var.is_priv,
            ScopeType::Neutral,
            SymbolKind::Unknown,
        );

        self.compiler.symbols.push(symbol);
    }

    // Cannot check for this since the type is not known
    /// Checks registered namespace for duplicates and collects errors if any are found
    //FIX: CHANGE TO NAME ID
    fn check_duplicates(&mut self) {
        // Solely a HashMap for spanning
        let mut seen: HashMap<InternedId, AstId> = HashMap::new();

        //NOTE: Suspicious
        let module = &self.compiler.mods[self.current_mod.id];

        // Searching if there are any duplicates with respect to the scope
        for scope_id in &module.scopes {
            let scope_info = &self.compiler.scopes[scope_id.id];
            for (ast_id, name_id) in &self.compiler.scopes[scope_id.id]
                .scope
                .table
                .ast_to_interned
            {
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
                        Item::Var(abs_var) => &abs_var.name_span,
                    }
                    .clone();

                    let dup_span = match &self.ast_info.items[ast_id.id as usize] {
                        Item::TypeDef(abs_typedef) => &abs_typedef.name_span,
                        Item::Struct(abs_struct) => &abs_struct.name_span,
                        Item::Enum(abs_enum) => &abs_enum.name_span,
                        Item::Alias(abs_alias) => &abs_alias.name_span,
                        Item::Var(abs_var) => &abs_var.name_span,
                    }
                    .clone();

                    let dup_name = self.interner.search(name_id.id as usize);

                    let msg = format!(
                        "Found more than one symbol with identifier \"{dup_name}\" in the section `{}`",
                        &scope_info.scope.scope_type
                    );

                    let module = &self.compiler.mods[self.current_mod.id];
                    self.reporter.report_spanned(
                        &msg,
                        None,
                        &[orig_span, dup_span],
                        &module
                            .src_metadata
                            .as_ref()
                            .expect("core should not be resolved"),
                    );
                }
            }

            // Clearing after finishing one table
            // More suspicious
            seen.clear();
        }
    }
}
