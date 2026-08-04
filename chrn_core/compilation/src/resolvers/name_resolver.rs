use chrn_utils::{
    chrn_config::ChrnConfig,
    err_codes::ErrorCode,
    id_types::{AstId, ConfigRootId, ImplId, ScopeId, SymbolId, TypeId, VariableId},
    intern::Intern,
    source_map::source_diagnostic::{
        DiagnosticLevel, SourceDiagnostic, SourceDiagnosticSummary, annotations::AnnotationKind,
    },
};

use crate::{
    lookup::scopes::{Scope, ScopeInfo, ScopeLookupPattern, ScopeType},
    parser::ast::ast_concepts::{
        AbstractAlias, AbstractConfig, AbstractConfigKind, AbstractDecl, AbstractEnum,
        AbstractImpl, AbstractStruct, AbstractTypeDef, AbstractVar, ConfigRootKind, Item,
    },
    resolvers::{resolver_env::RegistrationEnv, resolver_state::ResolverState},
    script_compiler::ScriptCompiler,
    semantic::{
        compilation_unit::CompilationUnit,
        hir::{
            hir_concepts::{Type, TypeInfo},
            hir_impls::{ConfigDefRoot, ImplHir, ImplHirKind},
            hir_symbols::{
                AliasDef, EnumDef, StructDef, Symbol, SymbolKind, SymbolOrigin, TypeDef, VarDef,
                VariableState,
            },
        },
    },
};

/// Registers symbols for every front-facing ast item. Members are not accounted for and should
/// be handled by `MemberResolver`.
///
/// This resolver at most reports symbols with the same identifiers in the same scope, but still
/// registers them.
pub struct NamespaceResolver<'a> {
    cfg: &'a ChrnConfig,
    interner: &'a Intern,
    compiler: &'a mut ScriptCompiler,
    summary: SourceDiagnosticSummary,
    //NOTE: May handle this differently but ok for now
}

impl NamespaceResolver<'_> {
    /// Instantiation requires that the compiler's state is valid and will panic otherwise
    pub fn new<'a>(
        cfg: &'a ChrnConfig,
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
            cfg,
            interner,
            compiler,
            summary: SourceDiagnosticSummary::default(),
            //TODO: This will be different
        }
    }

    // Needs the reporter though
    /// Returns the symbols created from the ast nodes within the given module `env` to allow for
    /// module by module compilation at the symbol level.
    pub fn resolve(
        &mut self,
        env: &RegistrationEnv,
    ) -> (Vec<CompilationUnit>, SourceDiagnosticSummary) {
        // Storing all symbols created associated with the current module so that compilation
        // doens't have to depend on the ast to keep a coherent understanding of
        let mut mod_symbols: Vec<CompilationUnit> = Vec::new();

        // Iterates through sections so that it stores the correct scope type associated with the
        // current ast node so it's compilation unit can use said information.
        for abs_sect_opt in &env.ast_info.sections {
            let Some(abs_sect) = abs_sect_opt else {
                continue;
            };

            let scope_type = abs_sect.kind.to_scope_type();

            for ast_id in abs_sect.nodes.iter().cloned() {
                // Maybe opt into section specific processing
                let comp_unit = match &env.ast_info.items[ast_id] {
                    Item::Decl(abs_decl) => {
                        let sym_id = match abs_decl {
                            AbstractDecl::TypeDef(abs_typedef) => {
                                self.register_typedef(abs_typedef, ast_id, scope_type, env)
                            }
                            AbstractDecl::Struct(abs_struct) => {
                                self.register_struct(abs_struct, ast_id, scope_type, env)
                            }
                            AbstractDecl::Enum(abs_enum) => {
                                self.register_enum(abs_enum, ast_id, scope_type, env)
                            }
                            AbstractDecl::Alias(abs_alias) => {
                                self.register_alias(abs_alias, ast_id, scope_type, env)
                            }
                            AbstractDecl::Var(abs_var) => {
                                self.register_var(abs_var, ast_id, scope_type, env)
                            }
                        };
                        CompilationUnit::Symbol(sym_id)
                    }
                    Item::Impl(abs_impl) => {
                        let impl_id = match abs_impl {
                            AbstractImpl::Config(abs_cfg) => {
                                self.register_config_root(abs_cfg, ast_id, scope_type, env)
                            }
                        };
                        CompilationUnit::Impl(impl_id)
                    }
                };

                mod_symbols.push(comp_unit);
            }
        }

        let mut summary = SourceDiagnosticSummary::default();
        summary.append_summary(&mut self.summary);

        (mod_symbols, summary)
    }
    // These registrations:
    // - Create a new symbol
    // - Create a new `var`, `nest`, `complex`, or `override` scope if the scope was not pushed yet.
    // - If a symbol with the same identifier as another is in the same scope, it overwrites the last symbol
    // and pushes the diagnostic

    // FIX: Should assert that the name id is some, semantically. Should cover this in tests.
    fn register_config_root(
        &mut self,
        abs_cfg: &AbstractConfig,
        ast_id: AstId,
        scope_type: ScopeType,
        env: &RegistrationEnv,
    ) -> ImplId {
        debug_assert!(
            matches!(
                abs_cfg.lookup_pat,
                ScopeLookupPattern::NamespaceOnly
                    | ScopeLookupPattern::OnlyVar
                    | ScopeLookupPattern::OnlyNest
            ),
            "Either config of `abs_cfg` was done wrong or a core language change did not update this assertion.\nExpected `ScopeLookupPattern::NoRestrictions/OnlyVar`, found {:?}",
            abs_cfg.lookup_pat
        );
        debug_assert!(matches!(abs_cfg.kind, AbstractConfigKind::Root(_)));
        debug_assert!(matches!(
            scope_type,
            ScopeType::Complex | ScopeType::Override
        ));

        // Pushing the scope loads all symbols needed by override
        _ = self.compiler.push_scope(scope_type, env.current_mod);

        let impl_id = ImplId::new(self.compiler.impls.len() as u32);
        let cfg_id = ConfigRootId::new(self.compiler.cfgs.len() as u32);

        // If an original exists, get the key so that it can be reported, otherwise insert it. This
        // is to avoid inserting first and overwriting the last symbol since ergonomically, it
        // probably makes more sense to keep the original for scope searching to fall-back to.

        let kind = if scope_type == ScopeType::Complex {
            ConfigRootKind::Complex
        } else {
            ConfigRootKind::Override
        };

        // let orig_sym_opt = table.interned_to_sym.insert(abs_cfg.name_id, sym_id);
        let cfg_def = ConfigDefRoot::new(
            impl_id,
            cfg_id,
            None,
            abs_cfg.lookup_pat,
            kind,
            Vec::new(),
            Vec::new(),
        );

        let impl_hir = ImplHir::new(
            impl_id,
            ImplHirKind::Config(cfg_id),
            scope_type,
            Some(ast_id),
        );

        self.compiler.cfgs.push(cfg_def);
        self.compiler.impls.push(impl_hir);
        impl_id
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
        scope_type: ScopeType,
        env: &RegistrationEnv,
    ) -> SymbolId {
        // Why was this message put here???
        // This will all likely fail eventually
        let scope_id = self.compiler.push_scope(scope_type, env.current_mod);
        let sym_id = SymbolId::new(self.compiler.symbols.len() as u32);

        let table = &mut self.compiler.get_scope_mut(scope_id).scope.table;

        table.ast_to_sym.insert(ast_id, sym_id);

        let orig_sym_opt = if let Some(original) = table.interned_to_sym.get(&abs_typedef.name_id) {
            Some(*original)
        } else {
            table.interned_to_sym.insert(abs_typedef.name_id, sym_id);
            None
        };

        // The actual typedefs position to store inside it's symbol
        let type_def_type_id = TypeId::new(self.compiler.types.len() as u32);

        // The id of the spot where the unknown type is placed, for the typedef
        // May or may not be able to use the reserved Unknown spot
        let inner_type_id = TypeId::new((self.compiler.types.len() + 1) as u32);

        let type_def = TypeDef::new(
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
            abs_typedef.is_priv,
            None,
            scope_type,
            SymbolKind::Type(type_def_type_id),
        );

        self.compiler.symbols.push(symbol);

        let type_def_info = TypeInfo::new(Type::TypeDef(type_def), env.current_mod);
        self.compiler.types.push(type_def_info);

        // Yes, ty and type should probably be consistent in some form name-wise.
        let inner_ty_info = TypeInfo::new(Type::Unknown, env.current_mod);
        self.compiler.types.push(inner_ty_info);

        if let Some(orig_sym_id) = orig_sym_opt {
            self.report_duplicate(orig_sym_id, sym_id, env);
        }
        sym_id
    }

    fn register_struct(
        &mut self,
        abs_struct: &AbstractStruct,
        ast_id: AstId,
        scope_type: ScopeType,
        env: &RegistrationEnv,
    ) -> SymbolId {
        let sym_id = SymbolId::new(self.compiler.symbols.len() as u32);
        let scope_id = self.compiler.push_scope(scope_type, env.current_mod);
        let table = &mut self.compiler.get_scope_mut(scope_id).scope.table;

        table.ast_to_sym.insert(ast_id, sym_id);

        let orig_sym_opt = if let Some(original) = table.interned_to_sym.get(&abs_struct.name_id) {
            Some(*original)
        } else {
            table.interned_to_sym.insert(abs_struct.name_id, sym_id);
            None
        };

        if !abs_struct.is_priv {
            let module = &mut self.compiler.mods[env.current_mod];
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
            scope_type,
            SymbolKind::Type(type_id),
        );

        self.compiler.symbols.push(symbol);

        let ty_info = TypeInfo::new(Type::Struct(struct_def), env.current_mod);
        self.compiler.types.push(ty_info);

        if let Some(orig_sym_id) = orig_sym_opt {
            self.report_duplicate(orig_sym_id, sym_id, env);
        }
        sym_id
    }

    fn register_enum(
        &mut self,
        abs_enum: &AbstractEnum,
        ast_id: AstId,
        scope_type: ScopeType,
        env: &RegistrationEnv,
    ) -> SymbolId {
        let scope_id = self.compiler.push_scope(scope_type, env.current_mod);
        let sym_id = SymbolId::new(self.compiler.symbols.len() as u32);
        let type_id = TypeId::new(self.compiler.types.len() as u32);

        let table = &mut self.compiler.get_scope_mut(scope_id).scope.table;

        table.ast_to_sym.insert(ast_id, sym_id);

        let orig_sym_opt = if let Some(original) = table.interned_to_sym.get(&abs_enum.name_id) {
            Some(*original)
        } else {
            table.interned_to_sym.insert(abs_enum.name_id, sym_id);
            None
        };

        if !abs_enum.is_priv {
            let module = &mut self.compiler.mods[env.current_mod];
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
            scope_type,
            SymbolKind::Type(type_id),
        );

        self.compiler.symbols.push(symbol);

        let ty_info = TypeInfo::new(Type::Enum(enum_def), env.current_mod);
        self.compiler.types.push(ty_info);

        if let Some(orig_sym_id) = orig_sym_opt {
            self.report_duplicate(orig_sym_id, sym_id, env);
        }
        sym_id
    }

    fn register_alias(
        &mut self,
        abs_alias: &AbstractAlias,
        ast_id: AstId,
        scope_type: ScopeType,
        env: &RegistrationEnv,
    ) -> SymbolId {
        let scope_id = self.compiler.push_scope(scope_type, env.current_mod);
        let sym_id = SymbolId::new(self.compiler.symbols.len() as u32);
        let type_id = TypeId::new(self.compiler.types.len() as u32);

        let table = &mut self.compiler.get_scope_mut(scope_id).scope.table;

        table.ast_to_sym.insert(ast_id, sym_id);
        let orig_sym_opt = if let Some(original) = table.interned_to_sym.get(&abs_alias.name_id) {
            Some(*original)
        } else {
            table.interned_to_sym.insert(abs_alias.name_id, sym_id);
            None
        };

        if !abs_alias.is_priv {
            let module = &mut self.compiler.mods[env.current_mod];
            module.exports.push(sym_id);
        }

        // Making local scopes in this way because sections do not emergently allow for
        // parent hierarchies.
        let local_scope_id = ScopeId::new(self.compiler.scopes.len() as u16);
        let local_scope = Scope::new(local_scope_id, ScopeType::Local, false, None);

        self.compiler
            .scopes
            .push(ScopeInfo::new(local_scope, Some(sym_id), env.current_mod));

        let current_mod = &mut self.compiler.mods[env.current_mod];
        current_mod.scopes.push(local_scope_id);

        //NOTE: Thinking about adding this but it would also mean the name resolver someone
        //participates in expression setup, which could entail things.
        //
        // let mut params: Vec<Param> = Vec::new();
        // let mut seen_params: Vec<&AbstractParam> = Vec::new();
        //
        // // Just a bit crowded in here..
        // // WARN: Ok this just looks like an inlined function now
        // for (i, abs_param) in abs_alias.params.iter().enumerate() {
        //     seen_params.push(abs_param);
        //
        //     //TODO: SHOULD THIS BE A VARIABLE?
        //     let expr_id = ExprId::new(self.compiler.exprs.len() as u32);
        //     let val_id = ValueId::new(self.compiler.values.len() as u32);
        //
        //     let type_id = match resolve::resolve_type_expr(
        //         self.compiler,
        //         AssociatedScopeKind::Module(env.current_mod),
        //         &abs_param.sp_ty_expr,
        //         ScopeType::Neutral,
        //         ScopeLookupPattern::NoRestrictions,
        //         env,
        //     ) {
        //         TypeExprResult::Type(type_id) => type_id,
        //         res => {
        //             let preset_err = preset_reporter::type_expr_result_to_preset_err(
        //                 &self.compiler,
        //                 self.interner,
        //                 &res,
        //                 env,
        //             )
        //             .expect("Result enforced by `match`");
        //
        //             preset_reporter::report_preset(&self.compiler,
        //                 &mut self.err_vec,
        //                 preset_err,
        //                 env.region,
        //                 self.settings,
        //                 self.interner,
        //             );
        //
        //             TypeId::new(script_compiler::CORE_UNKNOWN)
        //         }
        //     };
        //
        //     let param_sym_id = SymbolId::new(self.compiler.symbols.len() as u32);
        //     let var_id = VariableId::new(self.compiler.variables.len() as u32);
        //
        //     let var = VarDef::new(
        //         param_sym_id,
        //         abs_param.name_id,
        //         abs_param.name_span,
        //         VariableState::Known(val_id),
        //     );
        //
        //     let param_sym = Symbol::new(
        //         abs_param.name_id,
        //         param_sym_id,
        //         Some(AstId::new(i as u32)),
        //         SymbolOrigin::Module(env.current_mod),
        //         true,
        //         None,
        //         ScopeType::Local,
        //         SymbolKind::Variable(var_id),
        //     );
        //
        //     let expr_hir = ExprHir::Var(param_sym_id);
        //     let resolved_expr =
        //         ResolvedExpr::new(type_id, expr_hir, val_id, abs_param.name_span, Vec::new());
        //
        //     // Can this be possibly const evaluated if if possible if?
        //     //
        //     // Not sure about this
        //
        //     let val_info = ValueInfo::new(type_id, expr_id, None);
        //
        //     self.compiler.symbols.push(param_sym);
        //     self.compiler.variables.push(var);
        //     self.compiler.exprs.push(resolved_expr);
        //     self.compiler.values.push(val_info);
        //
        //     let local_scope = &mut self.compiler.get_scope_mut(local_scope_id).scope;
        //     local_scope
        //         .table
        //         .interned_to_sym
        //         .insert(abs_param.name_id, param_sym_id);
        //
        //     let param = Param::new(param_sym_id, type_id, AstId::new(i as u32));
        //
        //     params.push(param);
        // }
        //
        // //TODO: Will do something about this duplication.
        // for (i, current_param) in seen_params.iter().enumerate() {
        //     if let Some((_, original_param)) = seen_params
        //         .iter()
        //         .enumerate()
        //         // If the other index was declared after the current index and they have the same identifier
        //         //
        //         // Since this iteration specifically checks if the current was declared after the
        //         // last and the iteration terminates upon the first match, this correctly points at
        //         // the original field for all duplicates.
        //         .find(|(other_i, f)| *other_i < i && current_param.name_id == f.name_id)
        //     {
        //         let dup_name = self.interner.search(current_param.name_id);
        //
        //         let orig_span = original_param.name_span;
        //         let current_param_span = current_param.name_span;
        //
        //         // Preset error?
        //         let core_msg = format!("More than one variant has the identifier \"{dup_name}\"");
        //
        //         let src_diag =
        //             SourceDiagnostic::builder(DiagnosticLevel::Error, core_msg, env.region.path_id)
        //                 .add_annotation(
        //                     abs_alias.name_span,
        //                     AnnotationKind::Secondary,
        //                     "Found inside this alias".to_string().into(),
        //                 )
        //                 .add_annotation(
        //                     orig_span,
        //                     AnnotationKind::Secondary,
        //                     format!("Original usage of `{dup_name}` here").into(),
        //                 )
        //                 .add_annotation(current_param_span, AnnotationKind::Primary, None)
        //                 .build();
        //
        //         self.err_vec.push(src_diag);
        //     }
        // }

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
            scope_type,
            SymbolKind::Type(type_id),
        );

        self.compiler.symbols.push(symbol);

        let ty_info = TypeInfo::new(Type::Alias(alias_def), env.current_mod);
        self.compiler.types.push(ty_info);

        if let Some(orig_sym_id) = orig_sym_opt {
            self.report_duplicate(orig_sym_id, sym_id, env);
        }
        sym_id
    }

    /// Pushes neutral scope if needed, exports variable if public, then stores it with the state
    /// `ReservedTypeSlot` so that it can reserve a type slot without making an expression this
    /// early on, which would complicate the process.
    fn register_var(
        &mut self,
        abs_var: &AbstractVar,
        ast_id: AstId,
        scope_type: ScopeType,
        env: &RegistrationEnv,
    ) -> SymbolId {
        let sym_id = SymbolId::new(self.compiler.symbols.len() as u32);
        let scope_id = self.compiler.push_scope(scope_type, env.current_mod);
        let table = &mut self.compiler.get_scope_mut(scope_id).scope.table;

        table.ast_to_sym.insert(ast_id, sym_id);
        let orig_sym_opt = if let Some(original) = table.interned_to_sym.get(&abs_var.name_id) {
            Some(*original)
        } else {
            table.interned_to_sym.insert(abs_var.name_id, sym_id);
            None
        };

        if !abs_var.is_priv {
            let module = &mut self.compiler.mods[env.current_mod];
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
            scope_type,
            // Will be SymbolKind::Defer
            SymbolKind::Variable(var_id),
        );

        self.compiler.symbols.push(symbol);
        self.compiler.types.push(ty_info);
        self.compiler.variables.push(var);

        if let Some(orig_sym_id) = orig_sym_opt {
            self.report_duplicate(orig_sym_id, sym_id, env);
        }
        sym_id
    }

    // Cannot check for this since the type is not known
    /// Forms and stores diagnostic, given an original symbol which has the same identifier as an
    /// existing one
    //FIX: CHANGE TO NAME ID
    fn report_duplicate(
        &mut self,
        orig_sym_id: SymbolId,
        dup_sym_id: SymbolId,
        env: &RegistrationEnv,
    ) {
        //NOTE: Suspicious
        let orig_sym = &self.compiler.symbols[orig_sym_id];
        let orig_ast_id = orig_sym.ast_id.expect("Core should not be resolved");

        let dup_ast_id = self.compiler.symbols[dup_sym_id]
            .ast_id
            .expect("Core should not be resolved");

        let dup_name = self.interner.search(orig_sym.name_id);
        let scope_type = orig_sym.scope_origin;

        let orig_span = env.ast_info.get_decl(orig_ast_id).span();
        let dup_span = env.ast_info.get_decl(dup_ast_id).span();

        let core_msg = format!(
            "Duplicate identifier `{dup_name}` in section `{}`",
            &scope_type
        );

        let src_diag = SourceDiagnostic::builder(
            ErrorCode::ScopeErr.into(),
            DiagnosticLevel::Error,
            core_msg,
            env.region.path_id,
        )
        .add_annotation(
            orig_span,
            AnnotationKind::Secondary,
            format!("`{dup_name}` first seen here").into(),
        )
        .add_annotation(dup_span, AnnotationKind::Primary, None)
        .build();

        self.summary.push_diag(src_diag);
    }
}
