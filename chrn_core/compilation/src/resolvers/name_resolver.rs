use chrn_utils::{
    chrn_settings::ChrnSettings,
    id_types::{AstId, ConfigId, ScopeId, SymbolId, TypeId, VariableId},
    intern::Intern,
    source_map::source_diagnostic::{AnnotationKind, DiagnosticLevel, SourceDiagnostic},
};

use crate::{
    lookup::scopes::{LookupPattern, Scope, ScopeInfo, ScopeType},
    parser::ast::ast_concepts::{
        AbstractAlias, AbstractConfig, AbstractEnum, AbstractStruct, AbstractTypeDef, AbstractVar,
        Item,
    },
    resolvers::{resolver_env::ResolverEnv, resolver_state::ResolverState},
    script_compiler::ScriptCompiler,
    semantic::hir::hir_concepts::{
        AliasDef, ConfigDef, EnumDef, StructDef, Symbol, SymbolKind, SymbolOrigin, Type, TypeDef,
        TypeInfo, VarDef, VariableState,
    },
};

pub struct NamespaceResolver<'a> {
    settings: &'a ChrnSettings,
    interner: &'a Intern,
    compiler: &'a mut ScriptCompiler,
    err_vec: Vec<SourceDiagnostic>,
    //NOTE: May handle this differently but ok for now
}

impl NamespaceResolver<'_> {
    /// Instantiation requires that the compiler's state is valid and will panic otherwise
    pub fn new<'a>(
        settings: &'a ChrnSettings,
        interner: &'a Intern,
        compiler: &'a mut ScriptCompiler,
    ) -> NamespaceResolver<'a> {
        // TODO: But this also kind of means that a user CAN'T instantiate a resolver without doing
        // everything at once, or storing the resolver, but then that means the compiler stays
        // borrowed mutably
        //
        // Remove the compiler internal?
        debug_assert_eq!(ResolverState::NAMESPACE, compiler.resolver_state);
        compiler.resolver_state.advance();

        NamespaceResolver {
            settings,
            interner,
            compiler,
            err_vec: Vec::new(),
            //TODO: This will be different
        }
    }

    pub fn resolve(&mut self, env: &ResolverEnv) -> Result<(), Vec<SourceDiagnostic>> {
        // Registering namespaces
        for (id, item) in env.ast_info.items.iter().enumerate() {
            let ast_id = AstId::new(id as u32);

            // Maybe opt into section specific processing
            match item {
                Item::TypeDef(abs_typedef) => self.register_typedef(abs_typedef, ast_id, env),
                Item::Struct(abs_struct) => self.register_struct(abs_struct, ast_id, env),
                Item::Enum(abs_enum) => self.register_enum(abs_enum, ast_id, env),
                Item::Alias(abs_alias) => self.register_alias(abs_alias, ast_id, env),
                Item::Var(abs_var) => self.register_var(abs_var, ast_id, env),
                Item::Config(abs_cfg) => self.register_config(abs_cfg, ast_id, env),
            }
        }

        if !self.err_vec.is_empty() {
            let mut diags = Vec::new();
            diags.append(&mut self.err_vec);

            return Err(diags);
        }

        Ok(())
    }

    fn register_config(&mut self, abs_cfg: &AbstractConfig, ast_id: AstId, env: &ResolverEnv) {
        debug_assert!(
            matches!(
                abs_cfg.lookup_pattern,
                LookupPattern::NamespaceOnly | LookupPattern::OnlyVar
            ),
            "Either configuration of `abs_cfg` was done wrong or a core language change did not update this assertion.\nExpected `LookupPattern::NoRestrictions/OnlyVar`, found {:?}",
            abs_cfg.lookup_pattern
        );

        let scope_id = self
            .compiler
            .push_scope(ScopeType::Complex, env.current_mod);

        let sym_id = SymbolId::new(self.compiler.symbols.len() as u32);
        let cfg_id = ConfigId::new(self.compiler.configs.len() as u32);

        let table = &mut self.compiler.get_scope_mut(scope_id).scope.table;
        let orig_sym_opt = table.interned_to_sym.insert(abs_cfg.name_id, sym_id);
        table.ast_to_sym.insert(ast_id, sym_id);

        let cfg_def = ConfigDef::new(
            abs_cfg.name_id,
            abs_cfg.name_span,
            cfg_id,
            None,
            abs_cfg.lookup_pattern,
            Vec::new(),
            Vec::new(),
        );

        let sym = Symbol::new(
            abs_cfg.name_id,
            sym_id,
            Some(ast_id),
            SymbolOrigin::Module(env.current_mod),
            true,
            None,
            ScopeType::Complex,
            SymbolKind::Config(cfg_id),
        );

        self.compiler.configs.push(cfg_def);
        self.compiler.symbols.push(sym);

        if let Some(orig_sym_id) = orig_sym_opt {
            self.report_duplicate(orig_sym_id, sym_id, env);
        }
    }

    /// Attaches ast_id to the name_id of it's ast structure.
    /// Gives it a unique symbol id and attaches the ast id to it.
    /// Gives the typedef an id attached to `Unknown` which is to be resolved later
    /// Registers the unfinished representation with it's symbol id so that it can still be
    /// referenced
    fn register_typedef(
        &mut self,
        abs_typedef: &AbstractTypeDef,
        ast_id: AstId,
        env: &ResolverEnv,
    ) {
        // This will all likely fail eventually
        let scope_id = self.compiler.push_scope(ScopeType::Var, env.current_mod);
        let sym_id = SymbolId::new(self.compiler.symbols.len() as u32);

        let table = &mut self.compiler.get_scope_mut(scope_id).scope.table;

        table.ast_to_sym.insert(ast_id, sym_id);
        let orig_sym_opt = table.interned_to_sym.insert(abs_typedef.name_id, sym_id);

        // The actual typedefs position to store inside it's symbol
        let type_def_type_id = TypeId::new(self.compiler.types.len() as u32);

        // The id of the spot where the unknown type is placed, for the typedef
        // May or may not be able to use the reserved Unknown spot
        let inner_type_id = TypeId::new((self.compiler.types.len() + 1) as u32);

        let type_def_repre = TypeDef::new(
            sym_id,
            abs_typedef.name_id,
            abs_typedef.name_span,
            inner_type_id,
        );

        let symbol = Symbol::new(
            abs_typedef.name_id,
            sym_id,
            Some(ast_id),
            SymbolOrigin::Module(env.current_mod),
            true,
            None,
            ScopeType::Var,
            SymbolKind::Type(type_def_type_id),
        );

        self.compiler.symbols.push(symbol);

        let type_def_info = TypeInfo::new(Type::TypeDef(type_def_repre), env.current_mod);
        self.compiler.types.push(type_def_info);

        // Yes, ty and type should probably be consistent in some form name-wise.
        let inner_ty_info = TypeInfo::new(Type::Unknown, env.current_mod);
        self.compiler.types.push(inner_ty_info);

        if let Some(orig_sym_id) = orig_sym_opt {
            self.report_duplicate(orig_sym_id, sym_id, env);
        }
    }

    fn register_struct(&mut self, abs_struct: &AbstractStruct, ast_id: AstId, env: &ResolverEnv) {
        let sym_id = SymbolId::new(self.compiler.symbols.len() as u32);
        let scope_id = self.compiler.push_scope(ScopeType::Nest, env.current_mod);
        let table = &mut self.compiler.get_scope_mut(scope_id).scope.table;

        table.ast_to_sym.insert(ast_id, sym_id);
        let orig_sym_opt = table.interned_to_sym.insert(abs_struct.name_id, sym_id);

        if !abs_struct.is_priv {
            let module = &mut self.compiler.mods[env.current_mod.id];
            module.exports.push(sym_id);
        }

        let type_id = TypeId::new(self.compiler.types.len() as u32);
        let struct_def = StructDef::new(sym_id, abs_struct.name_span, Vec::new());

        let symbol = Symbol::new(
            abs_struct.name_id,
            sym_id,
            Some(ast_id),
            SymbolOrigin::Module(env.current_mod),
            abs_struct.is_priv,
            None,
            ScopeType::Nest,
            SymbolKind::Type(type_id),
        );

        self.compiler.symbols.push(symbol);

        let ty_info = TypeInfo::new(Type::Struct(struct_def), env.current_mod);
        self.compiler.types.push(ty_info);

        if let Some(orig_sym_id) = orig_sym_opt {
            self.report_duplicate(orig_sym_id, sym_id, env);
        }
    }

    fn register_enum(&mut self, abs_enum: &AbstractEnum, ast_id: AstId, env: &ResolverEnv) {
        let scope_id = self.compiler.push_scope(ScopeType::Nest, env.current_mod);
        let sym_id = SymbolId::new(self.compiler.symbols.len() as u32);
        let type_id = TypeId::new(self.compiler.types.len() as u32);

        let table = &mut self.compiler.get_scope_mut(scope_id).scope.table;

        table.ast_to_sym.insert(ast_id, sym_id);
        let orig_sym_opt = table.interned_to_sym.insert(abs_enum.name_id, sym_id);

        if !abs_enum.is_priv {
            let module = &mut self.compiler.mods[env.current_mod.id];
            module.exports.push(sym_id);
        }

        let enum_def = EnumDef::new(sym_id, abs_enum.name_span, Vec::new());

        let symbol = Symbol::new(
            abs_enum.name_id,
            sym_id,
            Some(ast_id),
            SymbolOrigin::Module(env.current_mod),
            abs_enum.is_priv,
            None,
            ScopeType::Nest,
            SymbolKind::Type(type_id),
        );

        self.compiler.symbols.push(symbol);

        let ty_info = TypeInfo::new(Type::Enum(enum_def), env.current_mod);
        self.compiler.types.push(ty_info);

        if let Some(orig_sym_id) = orig_sym_opt {
            self.report_duplicate(orig_sym_id, sym_id, env);
        }
    }

    fn register_alias(&mut self, abs_alias: &AbstractAlias, ast_id: AstId, env: &ResolverEnv) {
        let scope_id = self
            .compiler
            .push_scope(ScopeType::Neutral, env.current_mod);
        let sym_id = SymbolId::new(self.compiler.symbols.len() as u32);
        let type_id = TypeId::new(self.compiler.types.len() as u32);

        let table = &mut self.compiler.get_scope_mut(scope_id).scope.table;

        table.ast_to_sym.insert(ast_id, sym_id);
        let orig_sym_opt = table.interned_to_sym.insert(abs_alias.name_id, sym_id);

        if !abs_alias.is_priv {
            let module = &mut self.compiler.mods[env.current_mod.id];
            module.exports.push(sym_id);
        }

        // Making local scopes in this way because sections do not emergently allow for
        // parent hierarchies.
        let local_scope_id = ScopeId::new(self.compiler.scopes.len());
        let local_scope = Scope::new(local_scope_id, ScopeType::Local, false, None);

        self.compiler
            .scopes
            .push(ScopeInfo::new(local_scope, Some(sym_id), env.current_mod));

        let current_mod = &mut self.compiler.mods[env.current_mod.id];
        current_mod.scopes.push(local_scope_id);

        // Ok ok
        let alias_def = AliasDef::new(
            sym_id,
            abs_alias.name_span,
            Vec::new(),
            Vec::new(),
            local_scope_id,
        );

        let symbol = Symbol::new(
            abs_alias.name_id,
            sym_id,
            Some(ast_id),
            SymbolOrigin::Module(env.current_mod),
            abs_alias.is_priv,
            None,
            ScopeType::Neutral,
            SymbolKind::Type(type_id),
        );

        self.compiler.symbols.push(symbol);

        let ty_info = TypeInfo::new(Type::Alias(alias_def), env.current_mod);
        self.compiler.types.push(ty_info);

        if let Some(orig_sym_id) = orig_sym_opt {
            self.report_duplicate(orig_sym_id, sym_id, env);
        }
    }

    fn register_var(&mut self, abs_var: &AbstractVar, ast_id: AstId, env: &ResolverEnv) {
        let sym_id = SymbolId::new(self.compiler.symbols.len() as u32);
        let scope_id = self
            .compiler
            .push_scope(ScopeType::Neutral, env.current_mod);
        let table = &mut self.compiler.get_scope_mut(scope_id).scope.table;

        table.ast_to_sym.insert(ast_id, sym_id);
        let orig_sym_opt = table.interned_to_sym.insert(abs_var.name_id, sym_id);

        if !abs_var.is_priv {
            let module = &mut self.compiler.mods[env.current_mod.id];
            module.exports.push(sym_id);
        }

        let type_id = TypeId::new(self.compiler.types.len() as u32);
        let ty_info = TypeInfo::new(Type::Unknown, env.current_mod);

        let var_id = VariableId::new(self.compiler.variables.len() as u32);

        // TypeId is stored here so that the slot is reserved for anything that may need to refer
        // to it's type before it's actually declared
        let var = VarDef::new(
            sym_id,
            abs_var.name_id,
            abs_var.name_span,
            VariableState::ReservedTypeSlot(type_id),
        );

        // No information that this is a variable other than the fact that AstId -> SymbolId
        let symbol = Symbol::new(
            abs_var.name_id,
            sym_id,
            Some(ast_id),
            SymbolOrigin::Module(env.current_mod),
            abs_var.is_priv,
            None,
            ScopeType::Neutral,
            // Will be SymbolKind::Defer
            SymbolKind::Variable(var_id),
        );

        self.compiler.symbols.push(symbol);
        self.compiler.types.push(ty_info);
        self.compiler.variables.push(var);

        if let Some(orig_sym_id) = orig_sym_opt {
            self.report_duplicate(orig_sym_id, sym_id, env);
        }
    }

    // Cannot check for this since the type is not known
    /// Forms and stores diagnostic, given an original symbol which has the same identifier as an
    /// existing one
    //FIX: CHANGE TO NAME ID
    fn report_duplicate(&mut self, orig_sym_id: SymbolId, dup_sym_id: SymbolId, env: &ResolverEnv) {
        //NOTE: Suspicious
        let orig_sym = &self.compiler.symbols[orig_sym_id.id as usize];
        let orig_ast_id = orig_sym.ast_id.expect("Core should not be resolved");

        let dup_ast_id = &self.compiler.symbols[dup_sym_id.id as usize]
            .ast_id
            .expect("Core should not be resolved");

        let dup_name = self.interner.search(orig_sym.name_id);
        let scope_type = orig_sym.scope_origin;

        let orig_span = env.ast_info.items[orig_ast_id.id as usize].span();
        let dup_span = env.ast_info.items[dup_ast_id.id as usize].span();

        let core_msg = format!(
            "Found more than one symbol with identifier \"{dup_name}\" in the section `{}`",
            &scope_type
        );

        let src_diag =
            SourceDiagnostic::builder(DiagnosticLevel::Error, core_msg, env.region.path_id)
                .add_annotation(
                    orig_span,
                    AnnotationKind::Secondary,
                    format!("`{dup_name}` first seen here").into(),
                )
                .add_annotation(dup_span, AnnotationKind::Primary, None)
                .build();

        self.err_vec.push(src_diag);
    }
}
