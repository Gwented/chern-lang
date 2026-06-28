//TODO:
// Please split this...
pub mod type_context;

use chrn_utils::chrn_settings::ChrnSettings;
use chrn_utils::id_types::{
    AstId, ConfigId, DirectiveId, ExprId, InternedId, MemberId, ScopeId, SpannedContainer,
    SymbolId, TypeId, ValueId, VariableId,
};
use chrn_utils::intern::Intern;
use chrn_utils::source_map::source_diagnostic::{
    AnnotationKind, DiagnosticLevel, SourceDiagnostic,
};
use lang::directives::Directive;
use lang::fmter::{Formattable, Formatted};
use lang::types::builtins::{BuiltinType, BuiltinTypeKind};
use lang::values::{Value, ValueInfo};

use crate::constraints::ArgConstraint;
use crate::lookup::member_lookup::{self, MemberLookupResult};
use crate::lookup::scopes::{self, AssociatedScopeKind, LookupPattern, ScopeType};
use crate::parser::ast::ast_concepts::{
    AbstractAlias, AbstractConfig, AbstractDirective, AbstractEnum, AbstractStruct,
    AbstractTypeDef, AbstractVar, Item,
};
use crate::parser::ast::ast_exprs::{
    Expr, PathSegment, SpannedExpr, SpannedPathSegment, SpannedTypeExpr, TypeExpr,
};
use crate::resolvers::resolver_env::ResolverEnv;
use crate::script_compiler::{self, ScriptCompiler};
use crate::semantic::hir::hir_concepts::{
    ConfigOptionAssignment, FieldRepre, MemberSymbolKind, VariableState, VariantRepre,
};
use crate::semantic::hir::hir_concepts::{Symbol, SymbolKind, SymbolOrigin, VarDef};
use crate::semantic::hir::hir_concepts::{Type, TypeInfo};
use crate::semantic::hir::hir_exprs::{ExprHir, Param, PossibleMember, ResolvedExpr};
use crate::semantic::preset_err::{LookupError, MathError, PresetErr};
use crate::semantic::{evaluator, inference, preset_reporter};

use crate::resolvers::type_resolver::type_context::{
    ParentInfo, ParentState, PendingExpr, PendingSymbol, TypeContext,
};

//TODO: Less complicated injection
/// Resolves types and builds the rest of any structs, enums, or expressions that can be const
/// evaluated. Does so by mutating the compiler given, and maintaining context to retain it's last
/// state.
pub struct TypeResolver<'a> {
    settings: &'a ChrnSettings,
    interner: &'a Intern,
    compiler: &'a mut ScriptCompiler,
    ty_ctx: TypeContext,
    err_vec: Vec<SourceDiagnostic>,
}

impl<'a> TypeResolver<'a> {
    pub fn new(
        settings: &'a ChrnSettings,
        interner: &'a Intern,
        compiler: &'a mut ScriptCompiler,
    ) -> TypeResolver<'a> {
        TypeResolver {
            settings,
            ty_ctx: TypeContext::new(),
            err_vec: Vec::new(),
            interner,
            compiler,
        }
    }

    /// Mutates inner `ScriptCompiler` and `TypeContext` given the `env`.
    ///
    /// * `env`: The current environment the resolver is operating in. This being passed in
    /// explicitly allows for `TypeResolver` to maintain it's state throughout resolution while
    /// mutating off of given envs.
    pub fn resolve(&mut self, env: &ResolverEnv) -> Result<(), Vec<SourceDiagnostic>> {
        // This is resolving types but not resolving args or conditions.
        // Everything is in order so this cannot fail unless something internally went wrong.
        for item in &env.ast_info.items {
            match item {
                Item::TypeDef(abs_typedef) => _ = self.resolve_typedef(abs_typedef, env),
                Item::Struct(abs_struct) => _ = self.resolve_struct(abs_struct, env),
                Item::Enum(abs_enum) => _ = self.resolve_enum(abs_enum, env),
                Item::Alias(abs_alias) => _ = self.resolve_alias(abs_alias, env),
                Item::Var(abs_var) => _ = self.resolve_var(abs_var, env),
                // Why does this sound like grug </3
                //TODO: Cfgs need to be able to check if any member they have been processed or
                //not upfront so that they can be skipped and have their ast index saved to be gone
                //over. Maybe some light polling would be usable here to avoid either REALLY late
                //checks just because a type have a config, or over-checking.
                Item::Config(abs_cfg) => {
                    _ = self.resolve_cfg(abs_cfg, env);
                }
            }
        }

        // This is a system of tracking to where it dynamically through knowing the result
        // of the expression incremental resolution, and accounting for stale caching in regards
        // to not setting it's parent to resolved multiple times.
        // (which may be a little bit of over-complication but it works)
        //
        // The architecture is, say we have, let a = b, let b = c, let c = d + 5, let d = 4.
        // a and d have no resolved type or const value. c has a type because of inference regarding
        // literal 5, d has a const value AND type. In resolution, a, b and d remain unchanged, but c
        // being set to d is noticed, and d has a const value and const type, which results in the
        // incremental update marking c as both const booleans. Because expressions that were pending
        // inside of pending symbols know how far they went, they are checked. So, c would have b checked,
        // b would have it's parent's info that it's fully const and resolved, we then check if b is
        // dependended on, a depends on b, so now b has it's expressions attempted to be resolved, which
        // leads to a realizing it has 2 const values, which makes a resolved.

        // These variables are the sole determining factors as to how long the expression context
        // is looped, given any new information.

        let mut last_resolved_count: u32 = 0;
        let mut current_resolved_count: u32 = 0;
        while self.ty_ctx.needs_check {
            // let sym = &self.compiler.symbols[44];
            // let name = self.interner.search(sym.name_id.id as usize);
            // dbg!(name);
            //actually resolved already.
            self.ty_ctx.needs_check = false;
            // Giving ownership to a variable since the traversal chosen needs mutation while
            // traversing
            let mut pending_syms: Vec<(SymbolId, PendingSymbol)> = Vec::new();
            pending_syms.extend(self.ty_ctx.sym_queue.drain());

            let mut removable_syms: Vec<SymbolId> = Vec::new();

            for (sym_id, pending_sym) in &mut pending_syms {
                // If there is no resolved type then there cannot exist a const value
                if !pending_sym.has_resolved_ty {
                    continue;
                }

                match self.try_resolve_pending(*sym_id, pending_sym, env) {
                    //TODO: Can something be done with these?
                    //Succeeding just means no errors ocurred, not that new information was found,
                    //so maybe we can check here for removable symbols, say, if queue is empty?
                    //Is removing even worth it?
                    Ok(can_remove) => {
                        // Not sure about this yet
                        if can_remove {
                            removable_syms.push(*sym_id);
                        }
                    }
                    // Not sure if anything more can be done here since the diagnostic is already
                    // made
                    Err(_) => (),
                };
            }

            // Giving self back the data
            self.ty_ctx.sym_queue.extend(pending_syms);

            // Not changing this right now.
            //
            // The pending symbol the expression was found in
            // The index of the expression to set as stale.
            // The actual parent's info to fill in.
            let mut resolved_parents: Vec<(SymbolId, usize, ParentInfo)> = Vec::new();

            // Also needs to check if there exists a pending symbol which has ONLY stale
            // expressions inside, meaning it should be removed.

            // Finding all parents that recieved new information by checking if a pending expr has
            // the `Resolved` variant.
            for (pending_sym_id, pending_sym) in &self.ty_ctx.sym_queue {
                for (i, pending_expr) in pending_sym.pending_exprs.iter().enumerate() {
                    if let ParentState::Resolved(has_resolved_ty, has_const_val) =
                        pending_expr.parent_state
                    {
                        let possible_pending = pending_expr.parent_sym;
                        if self.ty_ctx.sym_queue.contains_key(&possible_pending) {
                            // Maybe current resolved can be removed now?
                            current_resolved_count += 1;
                            let parent_info =
                                ParentInfo::new(possible_pending, has_resolved_ty, has_const_val);

                            resolved_parents.push((*pending_sym_id, i, parent_info));
                        }
                    }
                }
            }

            // Loop that sets whatever resolution information regarding the parent to
            // true, so that it can actually be accounted for as a resolved pending symbol. Pending
            // symbol's expressions are never attempted for resolution unless they are marked to at
            // least have a resolved type. So, resolution trigerring is lazy and fully dependent on
            // signals.
            for (pending_sym_id, pending_expr_idx, parent_info) in resolved_parents {
                // Setting expr to stale
                let pending_sym = self
                    .ty_ctx
                    .sym_queue
                    .get_mut(&pending_sym_id)
                    .expect("Previous loop failed");

                pending_sym.pending_exprs[pending_expr_idx].parent_state =
                    ParentState::Notified(parent_info.has_resolved_ty, parent_info.has_const_val);

                // Allowing for parent to be searched in resolution
                let parent = self
                    .ty_ctx
                    .sym_queue
                    .get_mut(&parent_info.pending_sym_id)
                    .expect("Previous loop failed");

                parent.has_resolved_ty = parent_info.has_resolved_ty;
                parent.has_const_val = parent_info.has_const_val;
            }

            //WARN: By logic this seems fine since if the queue is empty then that means everything
            //found in pending_expr has a fully resolved parent.
            for sym_id in removable_syms {
                self.ty_ctx.sym_queue.remove(&sym_id);
            }

            if current_resolved_count == last_resolved_count {
                break;
            } else {
                last_resolved_count = current_resolved_count;
                self.ty_ctx.needs_check = true;
            }
        }

        // let symbol = &self.compiler.symbols[&SymbolId::new(0)];
        // match symbol.kind {
        //     SymbolKind::Type(type_id) => {
        //         let name = self.interner.search(symbol.name_id.id as usize);
        //         let ty = &self.compiler.types[type_id.id as usize];
        //         dbg!(name, &ty.ty);
        //     }
        //     SymbolKind::Val(value_id) => {
        //         let name = self.interner.search(symbol.name_id.id as usize);
        //         let val_info = &self.compiler.values[value_id.id as usize];
        //         let ty_info = &self.compiler.types[val_info.type_id.id as usize];
        //
        //         dbg!(name, ty_info);
        //     }
        //     _ => todo!(),
        // };

        // if env.current_mod == self.compiler.mods[self.compiler.mods.len() - 2].mod_id {
        //     dbg!(&self.ty_ctx);
        //     for symbol in &self.compiler.symbols {
        //         if self.interner.search(symbol.name_id) == "a" {
        //             let name = self.interner.search(symbol.name_id);
        //             dbg!(name);
        //             match symbol.kind {
        //                 SymbolKind::Val(value_id) => {
        //                     let val = &self.compiler.values[value_id.id as usize];
        //                     let expr = &self.compiler.exprs[val.expr_id.id as usize];
        //                     // dbg!(expr.val_id, expr);
        //                     dbg!(expr, val);
        //                 }
        //                 SymbolKind::Type(type_id) => {
        //                     let ty_info = &self.compiler.types[type_id.id as usize];
        //                     match &ty_info.ty {
        //                         Type::BuiltinType(builtin_type) => {
        //                             dbg!(builtin_type);
        //                         }
        //                         Type::Struct(struct_def) => todo!(),
        //                         Type::Enum(enum_def) => todo!(),
        //                         Type::Func(func_def) => todo!(),
        //                         Type::Alias(alias_def) => todo!(),
        //                         Type::TypeDef(type_def) => {
        //                             let ty = &self.compiler.types[type_def.type_id.id as usize];
        //                             dbg!(ty);
        //                         }
        //                         Type::Unknown => todo!(),
        //                         _ => todo!(),
        //                     }
        //                 }
        //                 _ => todo!(),
        //             }
        //             panic!("Done");
        //         }
        //     }
        // }

        //     for ty in &self.compiler.types {
        //         dbg!(ty);
        //     }
        //
        //     for expr_thing in &self.compiler.exprs {
        //         dbg!(expr_thing);
        //     }
        //
        //     for val in &self.compiler.values {
        //         dbg!(val);
        //     }
        // }

        if !self.err_vec.is_empty() {
            let mut diags = Vec::new();
            diags.append(&mut self.err_vec);

            return Err(diags);
        }

        Ok(())
    }

    fn resolve_cfg(&mut self, abs_cfg: &AbstractConfig, env: &ResolverEnv) -> Result<(), ()> {
        let mut opt_assignments: Vec<MemberId> = Vec::new();
        let mut inner_field_cfgs: Vec<ConfigId> = Vec::new();

        let scope_id = self
            .compiler
            .extract_scope_id(ScopeType::Complex, env.current_mod);
        let table = &self.compiler.get_scope(scope_id).scope.table;

        //TODO: global condition and argument setting.
        //field arg and cond settings.
        //same for enums.

        let parent_sym_id = table.interned_to_sym[&abs_cfg.name_id];
        let associated_scope = AssociatedScopeKind::Module(env.current_mod);

        // Checks if the symbol is valid later
        let found_sym_id = if let Some((found_sym, _)) = scopes::find_sym_id(
            self.compiler,
            associated_scope,
            abs_cfg.name_id,
            ScopeType::Complex,
            // I don't know about this lookup. It should probably be able to search namespace by
            // namespace, but not by #)%*@%)@(
            LookupPattern::NamespaceOnly,
        ) {
            found_sym
        } else {
            let name = self.interner.search(abs_cfg.name_id);
            let core_msg = format!(
                "Could not find a symbol with the identifier `{name}` in all complex searchable scopes"
            );

            let src_diag =
                SourceDiagnostic::builder(DiagnosticLevel::Error, core_msg, env.region.path_id)
                    .add_annotation(abs_cfg.name_span, AnnotationKind::Primary, None)
                    .build();

            self.err_vec.push(src_diag);

            return Err(());
        };

        // Right...

        // Get schema option then lookup against the actual possibilities
        // Maybe do this in constraints
        //
        // WARN: It is enforced that a member access config declaration CANNOT happen, therefore this can
        // never fail unless the current module does not have the type having it's config defined
        // declared anywhere. If this were to ever be lifted as a syntax enforced rule, this would
        // break.
        for abs_opt in &abs_cfg.opt_assignments {
            let expr_id = match self.register_expr(
                parent_sym_id,
                &abs_opt.array_expr,
                None,
                associated_scope,
                // This purposeful setting is done on purpose.
                ScopeType::Complex,
                &mut vec![],
                env,
            ) {
                Ok(expr_id) => expr_id,
                Err(preset_err) => {
                    preset_reporter::report_preset(
                        &mut self.err_vec,
                        preset_err,
                        env.region,
                        self.settings,
                        self.interner,
                    );

                    continue;
                }
            };

            let member_id = MemberId::new(self.compiler.members.len() as u32);
            let opt = ConfigOptionAssignment::new(
                parent_sym_id,
                member_id,
                abs_opt.name_id,
                abs_opt.name_span,
                expr_id,
            );
            self.compiler
                .members
                .push(MemberSymbolKind::OptionAssignment(opt));

            opt_assignments.push(member_id);
        }

        for abs_inner_cfg in &abs_cfg.inner_field_cfg {
            // Safe unwrap (This is not safe)
            let found_type_id = self.compiler.get_sym_type_id(found_sym_id).unwrap();
            let member_id = match member_lookup::lookup_member(
                self.compiler,
                found_type_id,
                abs_inner_cfg.name_id,
            ) {
                // These are split so that the theoretical ok and err paths are able to reduce
                // boilerplate where needed
                MemberLookupResult::Found(mem_id) => mem_id,
                lookup_res => {
                    // In case the lookup error points to an issue with the actual symbol we found
                    // rather than the member not existing or some non-terminal lookup error
                    //
                    // This is done because the validity of the symbol isn't checked before we
                    // actually lookup it's members
                    let mut should_break = false;

                    let src_diag = match lookup_res {
                        MemberLookupResult::InvalidTypeMemberAccess(type_id) => {
                            should_break = true;
                            //FIX: This has odd phrasing and pointers
                            // If we get a variable, this is matched, but the error is more so, you
                            // cannot use a variable in configuration, rather than the member
                            // access itself
                            let decl_span = self.compiler.get_sym_decl_span(found_sym_id).expect(
                                "Should have a span since it has members and was searched for",
                            );

                            let found_sym = &self.compiler.symbols[found_sym_id.id as usize];
                            let found_name = self.interner.search(found_sym.name_id);

                            let preset_err = PresetErr::Lookup(
                                LookupError::InvalidTypeMemberAccess(SpannedContainer::new(
                                    Type::to_fmt(self.compiler, type_id),
                                    decl_span,
                                )),
                            );

                            preset_reporter::create_diag_builder_preset(
                                &mut self.err_vec,
                                preset_err,
                                env.region,
                                self.settings,
                                self.interner,
                            )
                            .add_annotation(
                                abs_cfg.name_span,
                                AnnotationKind::Secondary,
                                format!("`{found_name}` used here").into(),
                            )
                            .add_annotation(
                                abs_inner_cfg.name_span,
                                AnnotationKind::Secondary,
                                "member searched for".to_string().into(),
                            )
                            .build()
                        }
                        MemberLookupResult::MemberNotFoundInType(type_id) => {
                            let decl_span = self
                                .compiler
                                .get_sym_decl_span(found_sym_id)
                                .expect("Should have a span since it has members and was searched");
                            let fmtted_ty = Type::to_fmt(self.compiler, type_id);

                            let preset_err = PresetErr::Lookup(LookupError::MemberNotFound(
                                SpannedContainer::new(abs_cfg.name_id, abs_cfg.name_span),
                                abs_inner_cfg.name_id,
                            ));

                            // List available members?
                            preset_reporter::create_diag_builder_preset(
                                &mut self.err_vec,
                                preset_err,
                                env.region,
                                self.settings,
                                self.interner,
                            )
                            .add_annotation(
                                decl_span,
                                AnnotationKind::Secondary,
                                format!("{} defined here", fmtted_ty).into(),
                            )
                            .add_annotation(
                                abs_inner_cfg.name_span,
                                AnnotationKind::Secondary,
                                "Searched for this member".to_string().into(),
                            )
                            .build()
                        }
                        // MemberLookupResult::InvalidSymbolMemberAccess => {
                        //     should_break = true;
                        //     let preset_err = SemanticError::Lookup(
                        //         LookupError::InvalidSymbolMemberAccess(SpannedContainer::new(
                        //             SymbolKind::to_fmt(self.compiler, found_sym_id),
                        //             abs_cfg.name_span,
                        //         )),
                        //     );
                        //
                        //     self
                        //         .reporter
                        //         .create_diag_builder_preset(preset_err)
                        //         .add_note("Config blocks expect a valid `nest->` or `var->` defined variable".to_string())
                        //         .build()
                        // }
                        // When is this case reached?
                        MemberLookupResult::Unknown(type_id) => {
                            let var = self.compiler.get_var(found_sym_id);
                            let name = self.interner.search(var.name_id);

                            // dbg!(&self.compiler.types[var.type_id.id as usize]);
                            todo!("RUST_BACKTRACE=1");
                        }
                        MemberLookupResult::Found(_) => unreachable!(),
                    };

                    self.err_vec.push(src_diag);

                    if should_break {
                        break;
                    }

                    continue;
                }
            };

            // Result doesn't matter since we continue either way
            _ = self.resolve_cfg_inner(parent_sym_id, member_id, abs_inner_cfg, env);
        }

        // dbg!(&abs_cfg);
        // panic!();

        let cfg_def = self.compiler.get_cfg_def_mut(parent_sym_id);

        cfg_def.sym_id = Some(found_sym_id);
        cfg_def.opt_assignments = opt_assignments;
        cfg_def.inner_field_cfgs = inner_field_cfgs;
        dbg!(&cfg_def);
        // panic!();

        Ok(())
    }

    /// Recursive function for resolving inner configs
    fn resolve_cfg_inner(
        &mut self,
        parent_sym_id: SymbolId,
        parent_member_id: MemberId,
        abs_cfg: &AbstractConfig,
        env: &ResolverEnv,
    ) -> Result<(), ()> {
        let associated_scope = AssociatedScopeKind::Module(env.current_mod);

        let mut opt_assignments: Vec<MemberId> = Vec::new();
        let mut inner_field_cfgs: Vec<ConfigId> = Vec::new();

        // Checks if the symbol is valid later

        // Get schema option then lookup against the actual possibilities
        // Maybe do this in constraints
        for abs_opt in &abs_cfg.opt_assignments {
            let expr_id = match self.register_expr(
                parent_sym_id,
                &abs_opt.array_expr,
                None,
                associated_scope,
                // This purposeful setting is done on purpose.
                ScopeType::Complex,
                &mut vec![],
                env,
            ) {
                Ok(expr_id) => expr_id,
                Err(preset_err) => {
                    preset_reporter::report_preset(
                        &mut self.err_vec,
                        preset_err,
                        env.region,
                        self.settings,
                        self.interner,
                    );

                    continue;
                }
            };

            let member_id = MemberId::new(self.compiler.members.len() as u32);
            let opt = ConfigOptionAssignment::new(
                parent_sym_id,
                member_id,
                abs_opt.name_id,
                abs_opt.name_span,
                expr_id,
            );

            self.compiler
                .members
                .push(MemberSymbolKind::OptionAssignment(opt));

            opt_assignments.push(member_id);
        }

        for abs_inner_cfg in &abs_cfg.inner_field_cfg {
            let parent_type_id = self.compiler.get_member_type_id(parent_member_id).unwrap();
            let name = self.interner.search(abs_inner_cfg.name_id);
            dbg!(name);
            let Type::Struct(structure) = &self.compiler.types[parent_type_id.id as usize].ty
            else {
                todo!();
            };
            let name = self
                .interner
                .search(self.compiler.symbols[structure.sym_id.id as usize].name_id);
            dbg!(name);
            // Person fields are not available since the config is not gauranteed to be touchign a
            // type that is known yet
            let person_fields = member_lookup::collect_all_members(self.compiler, parent_type_id);
            dbg!(person_fields);
            panic!();
            let member_id = match member_lookup::lookup_member(
                self.compiler,
                parent_type_id,
                abs_inner_cfg.name_id,
            ) {
                // These are split so that the theoretical ok and err paths are able to reduce
                // boilerplate where needed
                MemberLookupResult::Found(mem_id) => mem_id,
                lookup_res => {
                    // In case the lookup error points to an issue with the actual symbol we found
                    // rather than the member not existing or some non-terminal lookup error
                    //
                    // This is done because the validity of the symbol isn't checked before we
                    // actually lookup it's members
                    let mut should_break = false;

                    let parent_sym_id =
                        self.compiler.members[parent_member_id.id as usize].parent_sym_id();
                    let parent_decl_span = self
                        .compiler
                        .get_sym_decl_span(parent_sym_id)
                        .expect("Should exist in an ast searching context");
                    let parent_name_id = self.compiler.symbols[parent_sym_id.id as usize].name_id;
                    let parent_name = self.interner.search(parent_name_id);

                    let src_diag = match lookup_res {
                        MemberLookupResult::InvalidTypeMemberAccess(type_id) => {
                            should_break = true;
                            //FIX: This has odd phrasing and pointers
                            // If we get a variable, this is matched, but the error is more so, you
                            // cannot use a variable in configuration, rather than the member
                            // access itself

                            let found_sym = &self.compiler.symbols[parent_member_id.id as usize];
                            let found_name = self.interner.search(found_sym.name_id);

                            let preset_err = PresetErr::Lookup(
                                LookupError::InvalidTypeMemberAccess(SpannedContainer::new(
                                    Type::to_fmt(self.compiler, type_id),
                                    parent_decl_span,
                                )),
                            );

                            preset_reporter::create_diag_builder_preset(
                                &mut self.err_vec,
                                preset_err,
                                env.region,
                                self.settings,
                                self.interner,
                            )
                            .add_annotation(
                                abs_cfg.name_span,
                                AnnotationKind::Secondary,
                                format!("`{found_name}` used here").into(),
                            )
                            .add_annotation(
                                abs_inner_cfg.name_span,
                                AnnotationKind::Secondary,
                                "member searched for".to_string().into(),
                            )
                            .build()
                        }
                        MemberLookupResult::MemberNotFoundInType(type_id) => {
                            let fmtted_ty = Type::to_fmt(self.compiler, type_id);

                            let preset_err = PresetErr::Lookup(LookupError::MemberNotFound(
                                SpannedContainer::new(parent_name_id, abs_cfg.name_span),
                                abs_inner_cfg.name_id,
                            ));

                            // List available members?
                            preset_reporter::create_diag_builder_preset(
                                &mut self.err_vec,
                                preset_err,
                                env.region,
                                self.settings,
                                self.interner,
                            )
                            .add_annotation(
                                parent_decl_span,
                                AnnotationKind::Secondary,
                                format!("{} defined here", fmtted_ty).into(),
                            )
                            .add_annotation(
                                abs_inner_cfg.name_span,
                                AnnotationKind::Secondary,
                                "Searched for this member".to_string().into(),
                            )
                            .build()
                        }
                        // MemberLookupResult::InvalidSymbolMemberAccess => {
                        //     should_break = true;
                        //     let preset_err = SemanticError::Lookup(
                        //         LookupError::InvalidSymbolMemberAccess(SpannedContainer::new(
                        //             SymbolKind::to_fmt(self.compiler, found_sym_id),
                        //             abs_cfg.name_span,
                        //         )),
                        //     );
                        //
                        //     self
                        //         .reporter
                        //         .create_diag_builder_preset(preset_err)
                        //         .add_note("Config blocks expect a valid `nest->` or `var->` defined variable".to_string())
                        //         .build()
                        // }
                        // When is this case reached?
                        MemberLookupResult::Unknown(type_id) => {
                            // dbg!(&self.compiler.types[var.type_id.id as usize]);
                            todo!("RUST_BACKTRACE=1");
                        }
                        MemberLookupResult::Found(_) => unreachable!(),
                    };
                    self.err_vec.push(src_diag);

                    if should_break {
                        break;
                    }

                    continue;
                }
            };

            // Result doesn't matter since we continue either way
            let cfg_id = self.resolve_cfg_inner(parent_sym_id, member_id, abs_inner_cfg, env);
            // inner_field_cfgs.push(cfg_id);
        }

        // dbg!(&abs_cfg);
        // panic!();

        let cfg_def = self.compiler.get_cfg_def_mut(parent_sym_id);

        // Not quite sure what to do with this
        // cfg_def.sym_id = None;
        cfg_def.opt_assignments = opt_assignments;
        cfg_def.inner_field_cfgs = inner_field_cfgs;

        Ok(())
    }

    // fn should_wait(&self, abs_cfg: &AbstractConfig, env: &ResolverEnv) -> bool {
    //     let parent_sym_id = table.interned_to_sym[&abs_cfg.name_id];
    //     let associated_scope = AssociatedScopeKind::Module(env.current_mod);
    //     let all_member_ids = Vec::new();
    //     let member_id = MemberId::new(self.compiler.members.len() as u32);
    //
    //     // Checks if the symbol is valid later
    //     let found_sym_id = if let Some((found_sym, _)) = scopes::find_sym_id(
    //         self.compiler,
    //         associated_scope,
    //         abs_cfg.name_id,
    //         ScopeType::Complex,
    //         // I don't know about this lookup. It should probably be able to search namespace by
    //         // namespace, but not by #)%*@%)@(
    //         LookupPattern::NamespaceOnly,
    //     ) {
    //         found_sym
    //     } else {
    //         let name = self.interner.search(abs_cfg.name_id);
    //         let core_msg = format!(
    //             "Could not find a symbol with the identifier `{name}` in all complex searchable scopes"
    //         );
    //
    //         let src_diag =
    //             SourceDiagnostic::builder(DiagnosticLevel::Error, core_msg, env.region.path_id)
    //                 .add_annotation(abs_cfg.name_span, AnnotationKind::Primary, None)
    //                 .build();
    //
    //         self.err_vec.push(src_diag);
    //
    //         return true;
    //     };
    //
    //     //     let member_id = match member_lookup::lookup_member(
    //     //         self.compiler,
    //     //         found_type_id,
    //     //         abs_inner_cfg.name_id,
    //     //     ) {
    //     //         MemberLookupResult::Found(m_id) => m_id,
    //     //         _ => return false,
    //     //     };
    //     //
    //     // }
    //     for abs_inner_cfg in &abs_cfg.inner_field_cfg {
    //         // Safe unwrap (This is not safe)
    //         self.compiler.get_sym_type_id(found_sym_id).unwrap();
    //
    //         _ = self.should_wait_nested(abs_inner_cfg, parent_member_id);
    //     }
    //
    //     // Result doesn't matter since we continue either way
    //     false
    // }
    //
    // fn should_wait_nested(&self, abs_cfg: &AbstractConfig, parent_member_id: MemberId) {
    //     for abs_inner_cfg in &abs_cfg.inner_field_cfg {
    //         let parent_type_id = self.compiler.get_member_type_id(parent_member_id).unwrap();
    //         let name = self.interner.search(abs_inner_cfg.name_id);
    //         dbg!(name);
    //         let Type::Struct(structure) = &self.compiler.types[parent_type_id.id as usize].ty
    //         else {
    //             todo!();
    //         };
    //         let name = self
    //             .interner
    //             .search(self.compiler.symbols[structure.sym_id.id as usize].name_id);
    //         dbg!(name);
    //         // Person fields are not available since the config is not gauranteed to be touchign a
    //         // type that is known yet
    //         let person_fields = member_lookup::collect_all_members(self.compiler, parent_type_id);
    //         dbg!(person_fields);
    //         panic!();
    //     }
    // }

    fn try_resolve_pending(
        &mut self,
        resolved_sym_id: SymbolId,
        pending_sym: &mut PendingSymbol,
        env: &ResolverEnv,
        // Eyes
        // No actually why did this say eyes?
    ) -> Result<bool, ()> {
        // Tells the caller if the given pending symbol is fully resolved to where it can be
        // removed as a pending symbol
        let mut can_remove = false;
        let mut queue: Vec<ExprId> = Vec::new();

        //Suspicious
        for pending_expr in &pending_sym.pending_exprs {
            if let ParentState::Notified(true, true) = pending_expr.parent_state {
                continue;
            }

            // Error being treated the same as a resolved expression since it can't be mutated
            // further
            if pending_expr.parent_state == ParentState::Error {
                continue;
            }

            queue.push(pending_expr.pending_id);
        }

        // In the example:
        //
        // ```
        // let y = x + 2
        // let x = 2
        // ```
        //
        // root_expr = x
        // So, it needs to go x -> x + 2 -> None
        //

        // Needs to resolve first root
        for (i, root_id) in queue.iter().copied().enumerate() {
            // Still need to repair root expr
            let root_expr = &mut self.compiler.exprs[root_id.id as usize];
            match self.compiler.symbols[resolved_sym_id.id as usize].kind {
                SymbolKind::Variable(var_id) => {
                    let var = &self.compiler.variables[var_id.id as usize];
                    let VariableState::Known(val_id) = var.state else {
                        continue;
                    };

                    if pending_sym.has_resolved_ty {
                        let val_info = &self.compiler.values[val_id.id as usize];
                        let other_type_id = val_info.type_id;

                        self.compiler.types[root_expr.type_id.id as usize].ty =
                            Type::Deferred(other_type_id);

                        let inner_val = &mut self.compiler.values[root_expr.val_id.id as usize];
                        self.compiler.types[inner_val.type_id.id as usize].ty =
                            Type::Deferred(other_type_id);
                    }

                    if pending_sym.has_const_val {
                        let val_info = &self.compiler.values[val_id.id as usize];
                        let const_val_opt = val_info.const_val.clone();

                        let inner_val = &mut self.compiler.values[root_expr.val_id.id as usize];
                        inner_val.const_val = const_val_opt;
                    }
                }
                // I forgot what this means tbh honest
                // NOTE: Since expressions are initialized as `ReservedTypeSlot`, if there is say,
                // a cyclic dependency error, the error will exist and emit later, but this
                // technically still exists and needs to be ignored. Not currently aware of any
                // direct issues with this. Maybe an Error tag on a pending expression could help?
                SymbolKind::Type(_)
                | SymbolKind::Module(_)
                | SymbolKind::Config(_)
                | SymbolKind::Directive(_) => {
                    unreachable!("Not possible")
                }
            }

            if let Some(user) = root_expr.user {
                // TEST: Not sure if this accurately tracks yet
                match self.traverse_expr(user) {
                    Ok((has_resolved_ty, has_const_val)) => {
                        let pending_expr = &mut pending_sym.pending_exprs[i];

                        let has_new_info = match pending_expr.parent_state {
                            ParentState::Unresolved => true,
                            // Only value matters here since being resolved previous means there at
                            // least is a resolved type present.
                            ParentState::Resolved(_, old_val)
                            | ParentState::Notified(_, old_val) => has_const_val && !old_val,
                            ParentState::Error => false,
                        };

                        if has_new_info {
                            if has_resolved_ty {
                                pending_expr.parent_state =
                                    ParentState::Resolved(has_resolved_ty, has_const_val);
                            }
                        }
                    }
                    // WARN: This case is not hit yet
                    // Reports the error and continues
                    Err(preset_err) => {
                        // Extracting module of origin from the pending expression by using the symbol
                        // attached to the expression upon it's creation
                        //WARN: Suspicious

                        preset_reporter::report_preset(
                            &mut self.err_vec,
                            preset_err,
                            env.region,
                            self.settings,
                            self.interner,
                        );
                    }
                };
            } else {
                // If the root has no users, then that means its, let y = x where there is nothing else
                // that needs resolution since the root is always a single variable.

                // Also sending signal that the parent of this is resolved since it's a root.

                let pending_expr = &mut pending_sym.pending_exprs[i];
                let has_resolved_ty = pending_sym.has_resolved_ty;
                let has_const_val = pending_sym.has_const_val;

                let has_new_info = match pending_expr.parent_state {
                    ParentState::Unresolved => true,
                    // Only value matters here since being resolved previous means there at
                    // least is a resolved type present.
                    ParentState::Notified(_, old_val) | ParentState::Resolved(_, old_val) => {
                        has_const_val && !old_val
                    }
                    ParentState::Error => false,
                };

                if has_new_info {
                    pending_expr.parent_state =
                        ParentState::Resolved(has_resolved_ty, has_const_val);
                }

                break;
            }
        }

        // Meaning every pending_expr are impossible to be resolved further
        if queue.is_empty() {
            can_remove = true;
        }

        Ok(can_remove)
    }

    /// Returns an `Ok(true)` upon fully resolving a tree of expressions.
    /// Returns an `Ok(false)` if the resolution failed because a value was unknown.
    /// Returns `Err` upon real user errors.
    /// Method to recursively mutate tree of unresolved expression
    /// This works as root -> user -> user -> ... -> None
    // This needs to go from x -> x + 2 -> y recursively however long needed

    // A bit concerned that these are cloning themselves constantly to an extent
    fn traverse_expr(&mut self, current_expr_id: ExprId) -> Result<(bool, bool), PresetErr> {
        let expr = &self.compiler.exprs[current_expr_id.id as usize];
        let val_info = &self.compiler.values[expr.val_id.id as usize];

        //TEST:
        // Maybe types could always be inferred better? Although that doesn't really make sense
        // since if there is a type already inferred, if the types don't match then that's going to
        // error anyways depending on if the operation is applied
        let mut has_resolved_ty = !self.compiler.check_unknown(expr.type_id);
        let mut has_const_val = val_info.const_val.is_some();

        // But doesn't the queue disallow expressions that are resolved fully anyways? Wouldn't
        // this only need a const value check? Maybe.

        //TODO: Should use the booleans to prevent costly traversal operations
        match &self.compiler.exprs[current_expr_id.id as usize].expr_hir {
            ExprHir::Val(val_id) => {
                // This is unreachable
                let val_info = &self.compiler.values[val_id.id as usize];

                let new_type_id = val_info.type_id;
                let const_val_opt = val_info.const_val.clone();

                has_resolved_ty = self.compiler.check_unknown(new_type_id);
                has_const_val = const_val_opt.is_some();

                let expr = &mut self.compiler.exprs[current_expr_id.id as usize];
                // Mutating the type address so that it is now deferred to it's real type
                self.compiler.types[expr.type_id.id as usize].ty = Type::Deferred(new_type_id);

                let inner_val = &mut self.compiler.values[expr.val_id.id as usize];
                self.compiler.types[inner_val.type_id.id as usize].ty = Type::Deferred(new_type_id);

                inner_val.type_id = new_type_id;
                inner_val.const_val = const_val_opt;

                todo!("Make sure this is ok")
            }
            ExprHir::Unary { op, operand } => {
                // Getting the operand that could be resolved (Might be guarnteed but um..e)
                let operand_expr = &self.compiler.exprs[operand.id as usize];

                let is_unknown = self.compiler.check_unknown(operand_expr.type_id);
                // This means that we reached an expression inside of a resolved expression that is
                // not fully resolved yet, which is fine.
                if is_unknown {
                    return Ok((false, false));
                }

                has_resolved_ty = true;

                let operand_val_info = &self.compiler.values[operand_expr.val_id.id as usize];

                // Basic validation of expression to see if it's const or runtime
                let const_val_opt = if let Some(const_val) = &operand_val_info.const_val {
                    if !evaluator::is_compatible_unary(*op, const_val) {
                        return Err(MathError::UnaryOpMismatch(
                            SpannedContainer::new(const_val.kind().to_fmt(), operand_expr.span),
                            op.to_fmt(),
                        ))?;
                    } else {
                        has_const_val = true;
                        Some(evaluator::apply_unary_op(*op, const_val)?)
                    }
                } else {
                    None
                };

                let new_type_id = operand_expr.type_id;

                // Should this be deferred or new?
                //
                // Mutating expression's type so that the symbol using this expr reflects the new
                // information
                let expr = &mut self.compiler.exprs[current_expr_id.id as usize];
                self.compiler.types[expr.type_id.id as usize].ty = Type::Deferred(new_type_id);

                // Mutating inner value so that the symbol using this value reflects the new
                // information
                let inner_val = &mut self.compiler.values[expr.val_id.id as usize];
                self.compiler.types[inner_val.type_id.id as usize].ty = Type::Deferred(new_type_id);
                inner_val.const_val = const_val_opt;
            }
            ExprHir::BinaryExpr { lhs, op, rhs } => {
                //TODO: Considering a span vector so that they dont need to be duplicated or
                //computed by going inside items anymore.

                let lhs_expr = &self.compiler.exprs[lhs.id as usize];
                let rhs_expr = &self.compiler.exprs[rhs.id as usize];

                let is_unknown = if self.compiler.check_unknown(lhs_expr.type_id)
                    || self.compiler.check_unknown(rhs_expr.type_id)
                {
                    true
                } else {
                    false
                };

                // This means that we reached an expression inside of a resolved expression that is
                // not fully resolved yet
                if is_unknown {
                    return Ok((false, false));
                }

                has_resolved_ty = true;

                // Composing this so it can be matched cleanly for if const eval can be performed
                let lhs_val_opt = self.compiler.values[lhs_expr.val_id.id as usize]
                    .const_val
                    .as_ref();

                let rhs_val_opt = self.compiler.values[rhs_expr.val_id.id as usize]
                    .const_val
                    .as_ref();

                // This just checks if both are const, not if they were comptaible in the first
                // place. So, if it's not a comptaible binary, that could either mean 2 + "hi" or 2
                // + x where we just don't know x yet
                let const_val_opt: Option<Value> = match (lhs_val_opt, rhs_val_opt) {
                    (Some(lhs_const), Some(rhs_const)) => {
                        // If cannot perform operation and neither are unknown then there is actual
                        // corruption, and not one part just being unresolved
                        if !evaluator::is_compatible_binary(lhs_const, *op, rhs_const) {
                            return Err(MathError::BinaryOpMismatch(
                                SpannedContainer::new(lhs_const.kind().to_fmt(), lhs_expr.span),
                                SpannedContainer::new(rhs_const.kind().to_fmt(), rhs_expr.span),
                                op.to_fmt(),
                            ))?;
                        } else {
                            has_const_val = true;
                            Some(evaluator::apply_binary_op(lhs_const, *op, rhs_const)?)
                        }
                    }
                    _ => None,
                };

                //WARN: Suspicious
                // Should this account for I$@)($$*#%)$?

                let new_type_id: TypeId = if let Some(const_val) = &const_val_opt {
                    inference::infer_type_from_val(self.compiler, const_val)
                } else {
                    // The is_unknown params are a bit odd
                    inference::infer_type_from_binary_op(
                        lhs_expr.type_id,
                        rhs_expr.type_id,
                        false,
                        *op,
                        false,
                    )
                }
                .expect("Infallable since unknown is checked before this");

                //NOTE: Only the type of the expression is altered here, the rest is the inner
                //value
                let expr = &mut self.compiler.exprs[current_expr_id.id as usize];
                // Assigning directly since this is a newly created type id..
                expr.type_id = new_type_id;
                // dbg!(expr.type_id, new_type_id);
                // self.compiler.types[expr.type_id.id as usize].ty = panic!();

                let inner_val = &mut self.compiler.values[expr.val_id.id as usize];
                inner_val.type_id = new_type_id;
                // self.compiler.types[inner_val.type_id.id as usize].ty = Type::Deferred(new_type_id);
                inner_val.const_val = const_val_opt;
            }
            ExprHir::Call(expr_id, expr_ids) => todo!(),
            ExprHir::Var(sym_id) => {
                todo!("What is a varrrble")
            }
            ExprHir::Default(sym_id, expr_id) => {
                todo!("Default not finished")
            }
            // Hallucinating severely here.
            ExprHir::Array(expr_ids) => {
                //TODO: Need to require const here
                // So, maybe need to look at the context at some point later, or just typecheck.
                // tybejeg TYPE check
                let array = &self.compiler.exprs[current_expr_id.id as usize];
                let array_len = expr_ids.len();

                let mut type_id_opt: Option<TypeId> = None;
                let mut found_const_vals = 0;

                // If unknown then try to find an element that has a type inferred
                if self.compiler.check_unknown(array.type_id) {
                    for expr_id in expr_ids {
                        let expr = &self.compiler.exprs[expr_id.id as usize];

                        //WARN: Need to typecheck this too later
                        if !self.compiler.check_unknown(expr.type_id) && type_id_opt.is_none() {
                            type_id_opt = Some(expr.type_id);
                        }

                        let val_info = &self.compiler.values[expr.val_id.id as usize];
                        if val_info.const_val.is_some() {
                            found_const_vals += 1;
                        }
                    }
                }

                if !has_const_val && found_const_vals == array_len {
                    has_const_val = true;

                    let mut values: Vec<Value> = Vec::new();
                    for expr_id in expr_ids {
                        let val_id = &self.compiler.exprs[expr_id.id as usize].val_id;
                        let val = self.compiler.values[val_id.id as usize]
                            .const_val
                            .as_ref()
                            .expect("Previous loop failed")
                            .clone();

                        values.push(val);
                    }

                    let array_expr = &mut self.compiler.exprs[current_expr_id.id as usize];
                    let array_val = &mut self.compiler.values[array_expr.val_id.id as usize];
                    array_val.const_val = Some(Value::Array(values));
                }

                // This is setting a type id everytime. May be concerning.
                if !has_resolved_ty {
                    if let Some(new_type_id) = type_id_opt {
                        let array = &mut self.compiler.exprs[current_expr_id.id as usize];
                        array.type_id = new_type_id;
                        has_resolved_ty = true;
                    }
                }
            }
        }

        // Traversing up tree
        let expr = &self.compiler.exprs[current_expr_id.id as usize];
        //WARN: Seems to be working
        if let Some(user) = expr.user {
            return self.traverse_expr(user);
        }

        Ok((has_resolved_ty, has_const_val))
    }

    fn resolve_var(&mut self, abs_var: &AbstractVar, env: &ResolverEnv) -> Result<(), ()> {
        let scope_id = self
            .compiler
            .extract_scope_id(ScopeType::Neutral, env.current_mod);
        let table = &mut self.compiler.get_scope_mut(scope_id).scope.table;

        let sym_id = table.interned_to_sym[&abs_var.name_id];
        let associated_scope = AssociatedScopeKind::Module(env.current_mod);

        //NOTE: Pipeline where expressions are always returned, just that some may have
        //unresolved parts, which are put into the queue, not the variable itself.
        let expr_id = match self.register_expr(
            sym_id,
            &abs_var.spanned_expr,
            None,
            associated_scope,
            ScopeType::Neutral,
            &mut vec![sym_id],
            env,
        ) {
            Ok(expr_id) => expr_id,
            Err(preset_err) => {
                preset_reporter::report_preset(
                    &mut self.err_vec,
                    preset_err,
                    env.region,
                    self.settings,
                    self.interner,
                );

                return Err(());
            }
        };

        let expr = &self.compiler.exprs[expr_id.id as usize];
        let val = &self.compiler.values[expr.val_id.id as usize];

        //                      NOT unknown
        let has_resolved_ty = !self.compiler.check_unknown(expr.type_id);
        let has_const_val = val.const_val.is_some();

        let val_id = expr.val_id;

        // Sets the symbol's value to be the last expression's value so that later, if it's
        // expression is resolved further, since it's already pointing the the same expression it
        // will by proxy be updated

        //WARN: MAKE SURE EXPRESSION RESOLUTION IS NOT BROKEN FROM VAR CHANGES
        let var = self.compiler.get_var_mut(sym_id);
        var.state = VariableState::Known(val_id);

        // If the symbol that was just examined is a pending symbol AND it was actually resolved,
        // then it'll be marked as resolved
        if let Some(pending_sym) = self.ty_ctx.sym_queue.get_mut(&sym_id) {
            // Three flags for resolver use
            pending_sym.has_resolved_ty = has_resolved_ty;
            pending_sym.has_const_val = has_const_val;

            self.ty_ctx.needs_check = true;
        }

        Ok(())
    }

    fn resolve_typedef(
        &mut self,
        abs_typedef: &AbstractTypeDef,
        env: &ResolverEnv,
    ) -> Result<(), ()> {
        let type_id = match self.resolve_type_expr(
            AssociatedScopeKind::Module(env.current_mod),
            &abs_typedef.sp_ty_expr,
            ScopeType::Var,
            LookupPattern::NoRestrictions,
            env,
        ) {
            Ok(tid) => tid,
            Err(preset_err) => {
                preset_reporter::report_preset(
                    &mut self.err_vec,
                    preset_err,
                    env.region,
                    self.settings,
                    self.interner,
                );
                // I believe this is fine since it just points to the new type if found with no
                // mutation
                //
                // It is already initalized as unknown so this is redundant
                TypeId::new(script_compiler::CORE_UNKNOWN)
            }
        };

        let scope_id = self
            .compiler
            .extract_scope_id(ScopeType::Var, env.current_mod);
        let table = &self.compiler.get_scope(scope_id).scope.table;
        let sym_id = table.interned_to_sym[&abs_typedef.name_id];
        let associated_scope = AssociatedScopeKind::Module(env.current_mod);

        let mut conds: Vec<ExprId> = Vec::new();
        for spanned_expr in &abs_typedef.conds {
            //FIX: Scope type is a little wrong here since it's a condition
            match self.register_expr(
                sym_id,
                spanned_expr,
                None,
                associated_scope,
                ScopeType::Neutral,
                &mut vec![sym_id],
                env,
            ) {
                // For allowing for more diagnostics instead of just leaving the rest of the struct
                // unfinished upon singular errors
                Ok(c) => conds.push(c),
                Err(preset_err) => {
                    preset_reporter::report_preset(
                        &mut self.err_vec,
                        preset_err,
                        env.region,
                        self.settings,
                        self.interner,
                    );
                }
            }
        }

        let (directives, preset_errs) = self.handle_directives(&abs_typedef.directives, env);
        preset_reporter::report_preset_vec(
            &mut self.err_vec,
            preset_errs,
            env.region,
            self.settings,
            self.interner,
        );

        let type_def = self.compiler.get_typedef_mut(sym_id);

        // Assinging from `Unknown` to it's actual type if found
        //TODO: Constraints should check if this is unknown
        type_def.type_id = type_id;
        type_def.conds = conds;
        type_def.directives = directives;

        Ok(())
    }

    fn resolve_struct(&mut self, abs_struct: &AbstractStruct, env: &ResolverEnv) -> Result<(), ()> {
        // Not sure of if this should stay a Field type or just be a TypeDef since their intent
        // somewhat conflicts. For now, typedef is just consumed differently depending on if it's a
        // field declared in var-> or not since var-> fields may be made possible to reference, but
        // fields in structures can't. Will possibly just be unified in the future.
        let mut fields: Vec<MemberId> = Vec::new();
        let mut seen: Vec<(usize, InternedId)> = Vec::new();

        let scope_id = self
            .compiler
            .extract_scope_id(ScopeType::Nest, env.current_mod);
        let table = &self.compiler.get_scope(scope_id).scope.table;

        //TODO: global condition and argument setting.
        //field arg and cond settings.
        //same for enums.

        let sym_id = table.interned_to_sym[&abs_struct.name_id];
        let associated_scope = AssociatedScopeKind::Module(env.current_mod);

        // Checking if there are duplicate name ids within the same struct along with resolution
        for (i, field_typedef) in abs_struct.fields.iter().enumerate() {
            let type_id = match self.resolve_type_expr(
                AssociatedScopeKind::Module(env.current_mod),
                &field_typedef.sp_ty_expr,
                ScopeType::Nest,
                LookupPattern::NoRestrictions,
                env,
            ) {
                Ok(tid) => tid,
                Err(preset_err) => {
                    preset_reporter::report_preset(
                        &mut self.err_vec,
                        preset_err,
                        env.region,
                        self.settings,
                        self.interner,
                    );
                    continue;
                }
            };

            if let Some(original) = seen.iter().find(|other| field_typedef.name_id == other.1) {
                let struct_name = self.interner.search(abs_struct.name_id);
                let dup_name = self.interner.search(field_typedef.name_id);

                let orig_span = abs_struct.fields[original.0].name_span;
                let field_span = abs_struct.fields[i].name_span;

                let core_msg = format!(
                    "More than one field has the identifier \"{dup_name}\" within struct `{struct_name}`"
                );

                let src_diag =
                    SourceDiagnostic::builder(DiagnosticLevel::Error, core_msg, env.region.path_id)
                        .add_annotation(
                            abs_struct.name_span,
                            AnnotationKind::Secondary,
                            "Found inside this struct".to_string().into(),
                        )
                        .add_annotation(
                            orig_span,
                            AnnotationKind::Secondary,
                            format!("Original usage of identifier `{dup_name}` here").into(),
                        )
                        .add_annotation(field_span, AnnotationKind::Primary, None)
                        .build();

                self.err_vec.push(src_diag);
            }

            seen.push((i, field_typedef.name_id));

            let member_id = MemberId::new(self.compiler.members.len() as u32);

            let parent_sym_id = if let Some(found) = self.compiler.get_sym_from_type(type_id) {
                found
            } else {
                sym_id
            };

            let field = FieldRepre::new(
                parent_sym_id,
                member_id,
                field_typedef.name_id,
                field_typedef.name_span,
                type_id,
            );

            self.compiler.members.push(MemberSymbolKind::Field(field));
            fields.push(member_id);
        }

        for (i, current_member_id) in fields.iter().enumerate() {
            let abs_field = &abs_struct.fields[i];
            let mut conds: Vec<ExprId> = Vec::new();

            for cond in &abs_field.conds {
                match self.register_expr(
                    sym_id,
                    &cond,
                    None,
                    associated_scope,
                    ScopeType::Nest,
                    &mut vec![sym_id],
                    env,
                ) {
                    Ok(c) => conds.push(c),
                    Err(preset_err) => {
                        preset_reporter::report_preset(
                            &mut self.err_vec,
                            preset_err,
                            env.region,
                            self.settings,
                            self.interner,
                        );
                    }
                };
            }

            let (directives, preset_errs) = self.handle_directives(&abs_field.directives, env);
            preset_reporter::report_preset_vec(
                &mut self.err_vec,
                preset_errs,
                env.region,
                self.settings,
                self.interner,
            );

            let field = self.compiler.get_field_mut(*current_member_id);
            field.conds = conds;
            field.directives = directives;
            // field.directives = abs_field
            //     .directives
            //     .iter()
            //     .map(|sp_directive| sp_directive.inner)
            //     .collect();
        }

        let mut glob_conds: Vec<ExprId> = Vec::new();

        for cond in &abs_struct.glob_conds {
            match self.register_expr(
                sym_id,
                cond,
                None,
                associated_scope,
                ScopeType::Nest,
                &mut vec![sym_id],
                env,
            ) {
                Ok(c) => glob_conds.push(c),
                Err(preset_err) => {
                    preset_reporter::report_preset(
                        &mut self.err_vec,
                        preset_err,
                        env.region,
                        self.settings,
                        self.interner,
                    );
                }
            }
        }

        let (glob_directives, preset_errs) =
            self.handle_directives(&abs_struct.glob_directives, env);

        preset_reporter::report_preset_vec(
            &mut self.err_vec,
            preset_errs,
            env.region,
            self.settings,
            self.interner,
        );

        let struct_def = self.compiler.get_struct_mut(sym_id);

        struct_def.fields.append(&mut fields);
        struct_def.glob_conds = glob_conds;
        struct_def.glob_directives = glob_directives;

        Ok(())
    }

    fn resolve_enum(&mut self, abs_enum: &AbstractEnum, env: &ResolverEnv) -> Result<(), ()> {
        let mut variants: Vec<MemberId> = Vec::new();

        let scope_id = self
            .compiler
            .extract_scope_id(ScopeType::Nest, env.current_mod);
        let table = &self.compiler.get_scope(scope_id).scope.table;

        let sym_id = table.interned_to_sym[&abs_enum.name_id];
        let associated_scope = AssociatedScopeKind::Module(env.current_mod);

        // (ast variant idx, name_id)
        let mut seen: Vec<(usize, InternedId)> = Vec::new();
        //Maybe just compute this once after along with struct fields

        // Checking if there are duplicate name ids within the same enum
        for (i, variant) in abs_enum.variants.iter().enumerate() {
            if let Some(original) = seen.iter().find(|other| variant.name_id == other.1) {
                let enum_name = self.interner.search(abs_enum.name_id);
                let dup_name = self.interner.search(variant.name_id);

                let orig_span = abs_enum.variants[original.0].name_span;
                let variant_span = abs_enum.variants[i].name_span;

                let core_msg = format!(
                    "More than one variant has the identifier \"{dup_name}\" within enum `{enum_name}`"
                );

                let src_diag =
                    SourceDiagnostic::builder(DiagnosticLevel::Error, core_msg, env.region.path_id)
                        .add_annotation(
                            abs_enum.name_span,
                            AnnotationKind::Secondary,
                            "Found inside this enum".to_string().into(),
                        )
                        .add_annotation(
                            orig_span,
                            AnnotationKind::Secondary,
                            format!("Original usage of identifier `{dup_name}` here").into(),
                        )
                        .add_annotation(variant_span, AnnotationKind::Primary, None)
                        .build();

                self.err_vec.push(src_diag);
            }

            seen.push((i, variant.name_id));

            let member_id = MemberId::new(self.compiler.members.len() as u32);
            let variant_repre = if let Some(spanned_ty_expr) = &variant.ty_expr {
                let type_id = match self.resolve_type_expr(
                    AssociatedScopeKind::Module(env.current_mod),
                    &spanned_ty_expr,
                    ScopeType::Nest,
                    LookupPattern::NoRestrictions,
                    env,
                ) {
                    Ok(tid) => tid,
                    Err(preset_err) => {
                        preset_reporter::report_preset(
                            &mut self.err_vec,
                            preset_err,
                            env.region,
                            self.settings,
                            self.interner,
                        );
                        TypeId::new(script_compiler::CORE_UNKNOWN)
                    }
                };

                let parent_sym_id = if let Some(found) = self.compiler.get_sym_from_type(type_id) {
                    found
                } else {
                    sym_id
                };

                VariantRepre::new(
                    parent_sym_id,
                    member_id,
                    variant.name_id,
                    variant.name_span,
                    Some(type_id),
                    AstId::new(i as u32),
                )
            } else {
                VariantRepre::new(
                    sym_id,
                    member_id,
                    variant.name_id,
                    variant.name_span,
                    None,
                    AstId::new(i as u32),
                )
            };

            self.compiler
                .members
                .push(MemberSymbolKind::Variant(variant_repre));

            variants.push(member_id);
        }

        for (i, current_member_id) in variants.iter().enumerate() {
            let abs_variant = &abs_enum.variants[i];
            let mut conds: Vec<ExprId> = Vec::new();

            for cond in &abs_variant.conds {
                let cond_opt = match self.register_expr(
                    sym_id,
                    &cond,
                    None,
                    associated_scope,
                    ScopeType::Nest,
                    &mut vec![sym_id],
                    env,
                ) {
                    Ok(c) => Some(c),
                    Err(preset_err) => {
                        preset_reporter::report_preset(
                            &mut self.err_vec,
                            preset_err,
                            env.region,
                            self.settings,
                            self.interner,
                        );
                        None
                    }
                };

                if let Some(cond) = cond_opt {
                    conds.push(cond);
                }
            }

            let (directives, preset_errs) = self.handle_directives(&abs_variant.directives, env);
            preset_reporter::report_preset_vec(
                &mut self.err_vec,
                preset_errs,
                env.region,
                self.settings,
                self.interner,
            );

            let variant = self.compiler.get_variant_mut(*current_member_id);

            variant.conds = conds;
            variant.directives = directives;
        }

        let mut glob_conds: Vec<ExprId> = Vec::new();
        for cond in &abs_enum.glob_conds {
            let cond_opt = match self.register_expr(
                sym_id,
                cond,
                None,
                associated_scope,
                ScopeType::Nest,
                &mut vec![sym_id],
                env,
            ) {
                Ok(c) => Some(c),
                Err(preset_err) => {
                    preset_reporter::report_preset(
                        &mut self.err_vec,
                        preset_err,
                        env.region,
                        self.settings,
                        self.interner,
                    );
                    None
                }
            };

            if let Some(cond) = cond_opt {
                glob_conds.push(cond);
            }
        }

        let (glob_directives, preset_errs) = self.handle_directives(&abs_enum.glob_directives, env);
        preset_reporter::report_preset_vec(
            &mut self.err_vec,
            preset_errs,
            env.region,
            self.settings,
            self.interner,
        );

        let enum_def = self.compiler.get_enum_mut(sym_id);

        enum_def.variants.append(&mut variants);
        enum_def.glob_conds = glob_conds;
        enum_def.glob_directives = glob_directives;

        Ok(())
    }

    fn resolve_alias(&mut self, abs_alias: &AbstractAlias, env: &ResolverEnv) -> Result<(), ()> {
        let scope_id = self
            .compiler
            .extract_scope_id(ScopeType::Neutral, env.current_mod);
        let table = &self.compiler.get_scope_mut(scope_id).scope.table;

        let alias_sym_id = table.interned_to_sym[&abs_alias.name_id];
        let associated_scope = AssociatedScopeKind::Module(env.current_mod);

        let local_scope_id = self.compiler.get_alias(alias_sym_id).local_scope_id;

        let mut params: Vec<Param> = Vec::new();
        let mut seen: Vec<(usize, InternedId)> = Vec::new();

        // Just a bit crowded in here..
        // WARN: Ok this just looks like an inlined function now
        for (i, abs_param) in abs_alias.params.iter().enumerate() {
            if let Some(original) = seen.iter().find(|other| abs_param.name_id == other.1) {
                let alias_name = self.interner.search(abs_alias.name_id);
                let dup_name = self.interner.search(abs_param.name_id);

                let orig_span = abs_alias.params[original.0].name_span;

                let core_msg = format!(
                    "More than one variable has the identifier \"{dup_name}\" within alias `{alias_name}`"
                );

                let src_diag =
                    SourceDiagnostic::builder(DiagnosticLevel::Error, core_msg, env.region.path_id)
                        .add_annotation(
                            abs_alias.name_span,
                            AnnotationKind::Secondary,
                            "Found inside this alias".to_string().into(),
                        )
                        .add_annotation(
                            orig_span,
                            AnnotationKind::Secondary,
                            format!("Original usage of identifier `{dup_name}` here").into(),
                        )
                        .add_annotation(abs_param.name_span, AnnotationKind::Primary, None)
                        .build();

                self.err_vec.push(src_diag);
            }

            seen.push((i, abs_param.name_id));

            //TODO: SHOULD THIS BE A VARIABLE?
            let expr_id = ExprId::new(self.compiler.exprs.len() as u32);
            let val_id = ValueId::new(self.compiler.values.len() as u32);

            let type_id = match self.resolve_type_expr(
                AssociatedScopeKind::Module(env.current_mod),
                &abs_param.ty_expr,
                ScopeType::Neutral,
                LookupPattern::NoRestrictions,
                env,
            ) {
                Ok(tid) => tid,
                Err(preset_err) => {
                    preset_reporter::report_preset(
                        &mut self.err_vec,
                        preset_err,
                        env.region,
                        self.settings,
                        self.interner,
                    );
                    return Err(());
                }
            };

            let param_sym_id = SymbolId::new(self.compiler.symbols.len() as u32);
            let var_id = VariableId::new(self.compiler.variables.len() as u32);

            let var = VarDef::new(
                param_sym_id,
                abs_param.name_id,
                abs_param.name_span,
                VariableState::Known(val_id),
            );

            let param_sym = Symbol::new(
                abs_param.name_id,
                param_sym_id,
                Some(AstId::new(i as u32)),
                SymbolOrigin::Module(env.current_mod),
                true,
                None,
                ScopeType::Local,
                SymbolKind::Variable(var_id),
            );

            let expr_hir = ExprHir::Var(param_sym_id);
            let resolved_expr =
                ResolvedExpr::new(type_id, expr_hir, val_id, abs_param.name_span, Vec::new());

            // Can this be possibly const evaluated if if possible if?
            //
            // Not sure about this

            let val_info = ValueInfo::new(type_id, expr_id, None);

            self.compiler.symbols.push(param_sym);

            self.compiler.variables.push(var);
            self.compiler.exprs.push(resolved_expr);
            self.compiler.values.push(val_info);

            let local_scope = &mut self.compiler.get_scope_mut(local_scope_id).scope;
            local_scope
                .table
                .interned_to_sym
                .insert(abs_param.name_id, param_sym_id);

            let param = Param::new(param_sym_id, type_id, AstId::new(i as u32));

            params.push(param);
        }

        let mut conds: Vec<ExprId> = Vec::new();
        for spanned_expr in &abs_alias.conds {
            let cond_opt = match self.register_expr(
                alias_sym_id,
                spanned_expr,
                Some(local_scope_id),
                //NOTE: Could this change?
                associated_scope,
                ScopeType::Neutral,
                &mut vec![alias_sym_id],
                env,
            ) {
                Ok(c) => Some(c),
                Err(preset_err) => {
                    preset_reporter::report_preset(
                        &mut self.err_vec,
                        preset_err,
                        env.region,
                        self.settings,
                        self.interner,
                    );
                    None
                }
            };

            if let Some(cond) = cond_opt {
                conds.push(cond);
            }
        }

        let (directives, preset_errs) = self.handle_directives(&abs_alias.directives, env);
        preset_reporter::report_preset_vec(
            &mut self.err_vec,
            preset_errs,
            env.region,
            self.settings,
            self.interner,
        );

        //TODO: Arg constraint and option tpe constraint.
        //Could technically happen in constraint resolver since it. Yes.
        let alias_def = self.compiler.get_alias_mut(alias_sym_id);
        let param_count = params.len() as u32;

        //WARN: Does not yet have constraints of params discovered
        alias_def.params = params;
        // This could just be an explicit field, but in case of future changes keeping it under the
        // same layer of arg constraints so it's compatible with the already present checks in
        // `ConstraintResolver`.
        alias_def
            .arg_constraints
            // Well param count and arg count could mean a lot of different things
            // Ok
            .push(ArgConstraint::ArgCount(param_count));

        alias_def.conds = conds;
        alias_def.directives = directives;

        Ok(())
    }
    // These params are getting a little inflated so maybe a ctx struct for this environment could
    // be @()@$_ something

    /// On `Ok`, Creates a HIR expression type and returns the `ExprId` which is either going to be
    /// fully resolved, or marked as pending to be resolved later if possible.
    // The innate `parent_sym_id` may be weird depending on the context
    // Should it be replaced with VariableId?
    fn register_expr(
        &mut self,
        parent_sym_id: SymbolId,
        spanned_expr: &SpannedExpr,
        // Only usable with something like, alias(x) where x is local, not section local overall
        // like var->
        local_scope_id: Option<ScopeId>,
        associated_scope: AssociatedScopeKind,
        scope_type: ScopeType,
        seen: &mut Vec<SymbolId>,
        env: &ResolverEnv,
    ) -> Result<ExprId, PresetErr> {
        match &spanned_expr.expr {
            Expr::Var(name_id) => {
                if let Some(scope_id) = local_scope_id {
                    //FIXME:
                    if let Some(local_sym_id) =
                        scopes::find_sym_id_local(self.compiler, scope_id, *name_id)
                    {
                        // Stores it like this because local symbols can only be parameters, and
                        // parameters are inferred in type, so they are basically just variables
                        // that have their own process of being resolved

                        // Not sure if this should be a known index or not yet depending on what
                        // the constraint type becomes
                        let expr_id = ExprId::new(self.compiler.exprs.len() as u32);
                        let expr = match self.compiler.symbols[local_sym_id.id as usize].kind {
                            SymbolKind::Variable(var_id) => {
                                let var = &self.compiler.variables[var_id.id as usize];
                                let expr_hir = ExprHir::Var(local_sym_id);

                                let VariableState::Known(val_id) = var.state else {
                                    unreachable!("Not possible right now")
                                };

                                let type_id = self.compiler.values[val_id.id as usize].type_id;

                                ResolvedExpr::new(
                                    type_id,
                                    expr_hir,
                                    val_id,
                                    spanned_expr.span,
                                    Vec::new(),
                                )
                            }
                            // Local scopes can't reach these right now
                            SymbolKind::Type(type_id) => todo!(),
                            SymbolKind::Module(mod_id) => todo!(),
                            SymbolKind::Config(config_id) => todo!(),
                            SymbolKind::Directive(directive_id) => todo!(),
                        };

                        self.compiler.exprs.push(expr);

                        return Ok(expr_id);
                    }
                }

                if let Some((found_sym_id, _)) = scopes::find_sym_id(
                    self.compiler,
                    associated_scope,
                    *name_id,
                    scope_type,
                    // Should this be no restrictions?
                    LookupPattern::NoRestrictions,
                ) {
                    //WARN: Constant iteration upon seeing any symbol instead of a single check
                    //elsewhere
                    seen.push(found_sym_id);

                    // Code duplication reduction
                    self.check_cycle(seen, parent_sym_id, found_sym_id, env)?;

                    //NOTE: Only the PendingSymbol struct carries the PendingExpr struct, meaning
                    //there is no way to check for cycles outside of `TypeContext`, so this has to
                    //pick up the edge case of, "let x = x". Could change.
                    if found_sym_id == parent_sym_id {
                        let name = self
                            .interner
                            .search(self.compiler.symbols[found_sym_id.id as usize].name_id);

                        let core_msg = format!("Cannot declare symbol `{name}` as itself");

                        let dup_span = spanned_expr.span;

                        //FIX: Not failable since the exntire expression has to be placed in one module,
                        // to error to begin with, but should still operate off stored spans
                        let parent_ast_id = self.compiler.symbols[parent_sym_id.id as usize]
                            .ast_id
                            .expect("Parent must be a valid symbol to get to this point");

                        let parent_span = env.ast_info.get_sym_span(parent_ast_id);

                        let src_diag = SourceDiagnostic::builder(
                            DiagnosticLevel::Error,
                            core_msg,
                            env.region.path_id,
                        )
                        .add_annotation(parent_span, AnnotationKind::Primary, None)
                        .add_annotation(
                            dup_span,
                            AnnotationKind::Primary,
                            None,
                        );

                        return Err(PresetErr::General(src_diag));
                    }

                    let symbol = &self.compiler.symbols[found_sym_id.id as usize];
                    let expr_id = ExprId::new(self.compiler.exprs.len() as u32);

                    // I don't think this is needed since types are already known
                    let resolved_expr = match symbol.kind {
                        //WARN: Should this be the same?
                        SymbolKind::Type(type_id) => {
                            // Not sure what to do with this yet
                            // This would make types expressions, which wasn't true before
                            let ty_info = &self.compiler.types[type_id.id as usize];
                            //TODO: Alias is being looked up and seen as a type, not a
                            //function-like entity
                            match &ty_info.ty {
                                Type::Func(_) | Type::Alias(_) => {
                                    let val_id = ValueId::new(self.compiler.values.len() as u32);
                                    let val_info = ValueInfo::new(type_id, expr_id, None);
                                    self.compiler.values.push(val_info);

                                    let expr_hir = ExprHir::Var(found_sym_id);

                                    ResolvedExpr::new(
                                        type_id,
                                        expr_hir,
                                        val_id,
                                        spanned_expr.span,
                                        Vec::new(),
                                    )
                                }
                                Type::BuiltinType(_)
                                | Type::Struct(_)
                                | Type::Enum(_)
                                | Type::TypeDef(_)
                                | Type::Unknown => {
                                    let core_msg =
                                        "Cannot have a type within expressions".to_string();

                                    let src_diag = SourceDiagnostic::builder(
                                        DiagnosticLevel::Error,
                                        core_msg,
                                        env.region.path_id,
                                    )
                                    .add_annotation(
                                        spanned_expr.span,
                                        AnnotationKind::Primary,
                                        None,
                                    );

                                    return Err(PresetErr::General(src_diag));
                                }
                                Type::Constrained(ty_constraint) => todo!(),
                                Type::Deferred(type_id) => todo!("Is this possible?"),
                            }
                        }
                        SymbolKind::Variable(var_id) => {
                            let var = &self.compiler.variables[var_id.id as usize];

                            match var.state {
                                // A value is attached to the variable found
                                VariableState::Known(val_id) => {
                                    let val_info = &self.compiler.values[val_id.id as usize];
                                    let ty = &self.compiler.types[val_info.type_id.id as usize].ty;

                                    // The type of the variable is unknown meaning it still needs
                                    // to await
                                    if let Type::Unknown = ty {
                                        let pending_expr = PendingExpr::new(expr_id, parent_sym_id);
                                        self.ty_ctx.store_pending_expr(found_sym_id, pending_expr);
                                    }

                                    let expr_hir = ExprHir::Var(found_sym_id);

                                    ResolvedExpr::new(
                                        val_info.type_id,
                                        expr_hir,
                                        val_id,
                                        spanned_expr.span,
                                        Vec::new(),
                                    )
                                }
                                VariableState::ReservedTypeSlot(reserved_ty_id) => {
                                    let expr_id = ExprId::new(self.compiler.exprs.len() as u32);
                                    let expr_hir = ExprHir::Var(found_sym_id);
                                    let pending_expr = PendingExpr::new(expr_id, parent_sym_id);

                                    //NOTE: ONLY THIS POINT SHOULD STORE THE SYMBOL. This is how the
                                    //connection is made so that, y = x + 2, goes from x -> x + 2 -> None
                                    //after x is resolved.
                                    self.ty_ctx.store_pending_expr(found_sym_id, pending_expr);
                                    // Will possibly call for others to be resolved here, or do it from the
                                    // var resolution method itself

                                    // Creates value id that has an unknown type, no constant value, and an
                                    // unresolved expression.
                                    let val_id = ValueId::new(self.compiler.values.len() as u32);
                                    let val_info = ValueInfo::new(reserved_ty_id, expr_id, None);

                                    self.compiler.values.push(val_info);

                                    ResolvedExpr::new(
                                        reserved_ty_id,
                                        expr_hir,
                                        val_id,
                                        spanned_expr.span,
                                        Vec::new(),
                                    )
                                }
                            }
                        }
                        // SymbolKind::ReservedTypeSlot(reserved_ty_id) => {
                        //     let expr_id = ExprId::new(self.compiler.exprs.len() as u32);
                        //     let expr_hir = ExprHir::Var(found_sym_id);
                        //     let pending_expr = PendingExpr::new(expr_id, parent_sym_id);
                        //
                        //     //NOTE: ONLY THIS POINT SHOULD STORE THE SYMBOL. This is how the
                        //     //connection is made so that, y = x + 2, goes from x -> x + 2 -> None
                        //     //after x is resolved.
                        //     self.ty_ctx.store_pending_expr(found_sym_id, pending_expr);
                        //     // Will possibly call for others to be resolved here, or do it from the
                        //     // var resolution method itself
                        //
                        //     // Creates value id that has an unknown type, no constant value, and an
                        //     // unresolved expression.
                        //     let val_id = ValueId::new(self.compiler.values.len() as u32);
                        //     let val_info = ValueInfo::new(reserved_ty_id, expr_id, None);
                        //
                        //     self.compiler.values.push(val_info);
                        //
                        //     ResolvedExpr::new(
                        //         reserved_ty_id,
                        //         expr_hir,
                        //         val_id,
                        //         spanned_expr.span,
                        //         Vec::new(),
                        //     )
                        // }
                        SymbolKind::Module(_) => {
                            let err_mod_name = self.interner.search(*name_id);
                            // TODO: Should send help, which should be done after re-doing how
                            // errors are rendered
                            let core_msg = format!(
                                "The symbol `{err_mod_name}` is a module, which cannot be assigned as an expression value"
                            );

                            let src_diag = SourceDiagnostic::builder(
                                DiagnosticLevel::Error,
                                core_msg,
                                env.region.path_id,
                            )
                            .add_annotation(
                                spanned_expr.span,
                                AnnotationKind::Primary,
                                None,
                            );

                            return Err(PresetErr::General(src_diag));
                        }
                        SymbolKind::Directive(_) => unreachable!("We'll see"),
                        // Config not declarable in neutral sections so this should not be possible
                        SymbolKind::Config(_) => {
                            unreachable!("Should be impossible due to sections")
                        }
                    };

                    self.compiler.exprs.push(resolved_expr);

                    Ok(expr_id)
                } else {
                    let ident = self.interner.search(*name_id);
                    // if ident == "_" {
                    //     panic!("hi");
                    // }

                    // SemanticError needs centralization
                    let module = &self.compiler.mods[env.current_mod.id];
                    let mod_name = self.interner.search(module.name_id);

                    let and_local = if local_scope_id.is_some() {
                        " and local"
                    } else {
                        ""
                    };

                    let core_msg = format!(
                        "The symbol `{ident}` was not found in the module `{mod_name}` within `{scope_type}`{and_local} searchable scopes"
                    );

                    let src_diag = SourceDiagnostic::builder(
                        DiagnosticLevel::Error,
                        core_msg,
                        env.region.path_id,
                    )
                    .add_annotation(
                        spanned_expr.span,
                        AnnotationKind::Primary,
                        None,
                    );

                    Err(PresetErr::General(src_diag))
                }
            }
            Expr::Integer(name_id, _) => {
                if let Ok(num) = self.interner.search(*name_id).parse::<i64>() {
                    // Getting what it's spot would be when it's expression and value parts are
                    // pushed
                    let expr_id = ExprId::new(self.compiler.exprs.len() as u32);
                    let val_id = ValueId::new(self.compiler.values.len() as u32);

                    // Creating it's default type to the literal value of integer, as well as it's
                    // expression of just being a singular value type
                    let expr_hir = ExprHir::Val(val_id);
                    let type_id = TypeId::new(script_compiler::CORE_I64);

                    let resolved_expr =
                        ResolvedExpr::new(type_id, expr_hir, val_id, spanned_expr.span, Vec::new());

                    // Creating the actual value portion of the expression
                    let val = Value::I64(num);
                    let val_info = ValueInfo::new(type_id, expr_id, Some(val));

                    self.compiler.values.push(val_info);
                    self.compiler.exprs.push(resolved_expr);

                    Ok(expr_id)
                } else {
                    Err(PresetErr::NumericOverflow(
                        SpannedContainer::new(*name_id, spanned_expr.span),
                        Formatted::Integer,
                    ))
                }
            }
            Expr::Float(name_id, _) => {
                // No BigFloat yet
                if let Ok(num) = self.interner.search(*name_id).parse::<f64>() {
                    let expr_id = ExprId::new(self.compiler.exprs.len() as u32);
                    let val_id = ValueId::new(self.compiler.values.len() as u32);

                    let expr_hir = ExprHir::Val(val_id);
                    let type_id = TypeId::new(script_compiler::CORE_F64);
                    let expr =
                        ResolvedExpr::new(type_id, expr_hir, val_id, spanned_expr.span, Vec::new());

                    let val = Value::F64(num);
                    let val_info = ValueInfo::new(type_id, expr_id, Some(val));

                    self.compiler.values.push(val_info);
                    self.compiler.exprs.push(expr);

                    Ok(expr_id)
                } else {
                    Err(PresetErr::NumericOverflow(
                        SpannedContainer::new(*name_id, spanned_expr.span),
                        Formatted::Float,
                    ))
                }
            }
            Expr::BinaryExpr { lhs, op, rhs } => {
                let lhs_id = self.register_expr(
                    parent_sym_id,
                    &*lhs,
                    local_scope_id,
                    associated_scope,
                    scope_type,
                    seen,
                    env,
                )?;

                let rhs_id = self.register_expr(
                    parent_sym_id,
                    &*rhs,
                    local_scope_id,
                    associated_scope,
                    scope_type,
                    seen,
                    env,
                )?;

                let lhs_expr = &self.compiler.exprs[lhs_id.id as usize];
                let rhs_expr = &self.compiler.exprs[rhs_id.id as usize];

                let lhs_is_unknown = self.compiler.check_unknown(lhs_expr.type_id);
                let rhs_is_unknown = self.compiler.check_unknown(rhs_expr.type_id);

                // Composing this so it can be matched cleanly for if const eval can be performed
                let lhs_val_opt = self.compiler.values[lhs_expr.val_id.id as usize]
                    .const_val
                    .as_ref();

                let rhs_val_opt = self.compiler.values[rhs_expr.val_id.id as usize]
                    .const_val
                    .as_ref();

                // This just checks if both are const, not if they were comptaible in the first
                // place. So, if it's not a comptaible binary, that could either mean 2 + "hi" or 2
                // + x where we just don't know x yet
                let const_val_opt: Option<Value> = match (lhs_val_opt, rhs_val_opt) {
                    (Some(lhs_const), Some(rhs_const)) => {
                        // If cannot perform operation and neither are unknown then there is actual
                        // corruption, and not one part just being unresolved
                        if !evaluator::is_compatible_binary(lhs_const, *op, rhs_const)
                            && !lhs_is_unknown
                            && !rhs_is_unknown
                        {
                            return Err(MathError::BinaryOpMismatch(
                                SpannedContainer::new(lhs_const.kind().to_fmt(), lhs_expr.span),
                                SpannedContainer::new(rhs_const.kind().to_fmt(), rhs_expr.span),
                                op.to_fmt(),
                            ))?;
                        } else {
                            Some(evaluator::apply_binary_op(lhs_const, *op, rhs_const)?)
                        }
                    }
                    _ => None,
                };

                let val_id = ValueId::new(self.compiler.values.len() as u32);
                let expr_id = ExprId::new(self.compiler.exprs.len() as u32);

                let expr_hir = ExprHir::BinaryExpr {
                    lhs: lhs_id,
                    op: *op,
                    rhs: rhs_id,
                };

                let lhs_type_id = self.compiler.exprs[lhs_id.id as usize].type_id;
                let rhs_type_id = self.compiler.exprs[lhs_id.id as usize].type_id;
                // Maybe apply BinaryOp shouuld account for unknowns and return unknowns

                // Tries two levels of inference before allocating an unknown type id
                let type_id_opt: Option<TypeId> = if let Some(const_val) = &const_val_opt {
                    inference::infer_type_from_val(self.compiler, const_val)
                } else {
                    // The is_unknown params are a bit odd
                    inference::infer_type_from_binary_op(
                        lhs_type_id,
                        rhs_type_id,
                        lhs_is_unknown,
                        *op,
                        rhs_is_unknown,
                    )
                };

                // If a type was inferred then we will use that, otherwise unknown is allocated
                let type_id = if let Some(inner_type_id) = type_id_opt {
                    inner_type_id
                } else {
                    let type_id = TypeId::new(self.compiler.types.len() as u32);

                    let ty_info = TypeInfo::new(Type::Unknown, env.current_mod);
                    self.compiler.types.push(ty_info);
                    type_id
                };

                // Assigning the user so that if unresolved, the expression can later go up a tree
                // of all expressions that use it and have them be resolved alongside it where
                // possible.
                self.compiler.exprs[lhs_id.id as usize].user = Some(expr_id);
                self.compiler.exprs[rhs_id.id as usize].user = Some(expr_id);

                // Expression points to the value so the expr_id is returned alone.
                let resolved_expr = ResolvedExpr::new(
                    type_id,
                    expr_hir,
                    val_id,
                    spanned_expr.span,
                    vec![lhs_id, rhs_id],
                );

                let val_info = ValueInfo::new(type_id, expr_id, const_val_opt);

                self.compiler.exprs.push(resolved_expr);
                self.compiler.values.push(val_info);

                Ok(expr_id)
            }
            Expr::Char(c) => {
                let expr_id = ExprId::new(self.compiler.exprs.len() as u32);
                let val_id = ValueId::new(self.compiler.values.len() as u32);
                let type_id = TypeId::new(script_compiler::CORE_CHAR);

                let val = Value::Char(*c);
                let val_info = ValueInfo::new(type_id, expr_id, Some(val));
                self.compiler.values.push(val_info);

                let expr_hir = ExprHir::Val(val_id);
                let resolved_expr =
                    ResolvedExpr::new(type_id, expr_hir, val_id, spanned_expr.span, Vec::new());
                self.compiler.exprs.push(resolved_expr);

                Ok(expr_id)
            }
            Expr::Default(ident_expr, spanned_expr) => {
                let expr_id = ExprId::new(self.compiler.exprs.len() as u32);
                let val_id = ValueId::new(self.compiler.values.len() as u32);

                //WARN: SUSPICIOUS
                let default_ident_expr_id = self.register_expr(
                    parent_sym_id,
                    &ident_expr,
                    local_scope_id,
                    associated_scope,
                    scope_type,
                    seen,
                    env,
                )?;

                let default_val_expr_id = self.register_expr(
                    parent_sym_id,
                    &spanned_expr,
                    local_scope_id,
                    associated_scope,
                    scope_type,
                    seen,
                    env,
                )?;

                // Need the entire alias to use this as it's type through checks
                let type_id = self.compiler.exprs[default_val_expr_id.id as usize].type_id;

                //TODO: Need symbol of name id
                //Need it's inputs to be the symbol and spanned expression

                // DO NOT QUESTION THIS
                //WARN: Needs to be a smimbol
                let expr_hir = ExprHir::Default(default_ident_expr_id, default_val_expr_id);

                // Is the parameter an input if it doesn't have a value?
                // The issue is, it's not a known input of any sort, it's just an identifier.
                // Also, default is just a default so default only defaults when default defaults
                let resolved_expr = ResolvedExpr::new(
                    type_id,
                    expr_hir,
                    val_id,
                    spanned_expr.span,
                    vec![default_val_expr_id],
                );

                self.compiler.exprs[default_ident_expr_id.id as usize].user = Some(expr_id);
                self.compiler.exprs[default_val_expr_id.id as usize].user = Some(expr_id);

                self.compiler.exprs.push(resolved_expr);

                Ok(expr_id)
            }
            Expr::Str(name_id) => {
                let expr_id = ExprId::new(self.compiler.exprs.len() as u32);
                let val_id = ValueId::new(self.compiler.values.len() as u32);

                let type_id = TypeId::new(script_compiler::CORE_STR);

                let val = Value::InternedStr(*name_id);
                let val_info = ValueInfo::new(type_id, expr_id, Some(val));
                self.compiler.values.push(val_info);

                let expr_hir = ExprHir::Val(val_id);
                let resolved_expr =
                    ResolvedExpr::new(type_id, expr_hir, val_id, spanned_expr.span, Vec::new());
                self.compiler.exprs.push(resolved_expr);

                Ok(expr_id)
            }
            Expr::Unary(unary) => {
                let operand_id = self.register_expr(
                    parent_sym_id,
                    &unary.spanned_expr,
                    local_scope_id,
                    associated_scope,
                    scope_type,
                    seen,
                    env,
                )?;

                let operand_expr = &self.compiler.exprs[operand_id.id as usize];

                let is_unknown = self.compiler.check_unknown(operand_expr.type_id);

                let operand_val_opt = &self.compiler.values[operand_expr.val_id.id as usize];

                let const_val_opt = if let Some(const_val) = &operand_val_opt.const_val {
                    if !evaluator::is_compatible_unary(unary.op, const_val) && !is_unknown {
                        return Err(MathError::UnaryOpMismatch(
                            SpannedContainer::new(const_val.kind().to_fmt(), operand_expr.span),
                            unary.op.to_fmt(),
                        ))?;
                    } else {
                        Some(evaluator::apply_unary_op(unary.op, const_val)?)
                    }
                } else {
                    None
                };

                let val_id = ValueId::new(self.compiler.values.len() as u32);
                let unary_expr_id = ExprId::new(self.compiler.exprs.len() as u32);

                let expr_hir = ExprHir::Unary {
                    op: unary.op,
                    operand: operand_id,
                };

                let type_id = if const_val_opt.is_some() {
                    operand_expr.type_id
                } else {
                    let type_id = TypeId::new(self.compiler.types.len() as u32);
                    let ty_info = TypeInfo::new(Type::Unknown, env.current_mod);
                    self.compiler.types.push(ty_info);

                    type_id
                };

                let resolved_expr = ResolvedExpr::new(
                    type_id,
                    expr_hir,
                    val_id,
                    spanned_expr.span,
                    vec![operand_id],
                );

                self.compiler.exprs.push(resolved_expr);
                self.compiler.exprs[operand_id.id as usize].user = Some(unary_expr_id);

                let val_info = ValueInfo::new(type_id, unary_expr_id, const_val_opt);
                self.compiler.values.push(val_info);

                Ok(unary_expr_id)
            }
            Expr::Bool(boolean) => {
                //FIX:
                let type_id = TypeId::new(script_compiler::CORE_BOOL);
                if *boolean == true {
                    let expr_id = ExprId::new(self.compiler.exprs.len() as u32);
                    let val_id = ValueId::new(self.compiler.values.len() as u32);

                    let val = Value::Bool(true);
                    let val_info = ValueInfo::new(type_id, expr_id, Some(val));

                    let expr_hir = ExprHir::Val(val_id);
                    let resolved_expr =
                        ResolvedExpr::new(type_id, expr_hir, val_id, spanned_expr.span, vec![]);

                    self.compiler.exprs.push(resolved_expr);
                    self.compiler.values.push(val_info);

                    Ok(expr_id)
                } else {
                    let expr_id = ExprId::new(self.compiler.exprs.len() as u32);
                    let val_id = ValueId::new(self.compiler.values.len() as u32);

                    // Generics can only be thest types so this can stay for now
                    let val = Value::Bool(false);
                    let val_info = ValueInfo::new(type_id, expr_id, Some(val));

                    let expr_hir = ExprHir::Val(val_id);
                    let resolved_expr =
                        ResolvedExpr::new(type_id, expr_hir, val_id, spanned_expr.span, vec![]);

                    self.compiler.exprs.push(resolved_expr);
                    self.compiler.values.push(val_info);

                    Ok(expr_id)
                }
            }
            Expr::Call(caller, arg_exprs) => {
                // The "Call" in "Call(x, y)"
                let caller_id = self.register_expr(
                    parent_sym_id,
                    caller,
                    local_scope_id,
                    associated_scope,
                    scope_type,
                    seen,
                    env,
                )?;
                //WARN: Does this need something?
                let type_id = self.compiler.exprs[caller_id.id as usize].type_id;
                let mut call_args: Vec<ExprId> = Vec::new();

                for sp_expr in arg_exprs {
                    let arg = self.register_expr(
                        parent_sym_id,
                        sp_expr,
                        local_scope_id,
                        associated_scope,
                        scope_type,
                        seen,
                        env,
                    )?;

                    call_args.push(arg);
                }

                let expr_id = ExprId::new(self.compiler.exprs.len() as u32);
                let val_id = ValueId::new(self.compiler.values.len() as u32);

                let inputs = call_args.clone();

                let expr_hir = ExprHir::Call(caller_id, call_args);
                // Are the arguments inputs if they are the expression itself?
                let resolved_expr =
                    ResolvedExpr::new(type_id, expr_hir, val_id, spanned_expr.span, inputs);
                let val_info = ValueInfo::new(type_id, expr_id, None);

                self.compiler.exprs.push(resolved_expr);
                self.compiler.values.push(val_info);

                Ok(expr_id)
            }
            Expr::MemberAccess(abs_member_access) => {
                match self.resolve_member(
                    parent_sym_id,
                    &abs_member_access.base,
                    local_scope_id,
                    associated_scope,
                    scope_type,
                    seen,
                    env,
                )? {
                    // Maybe this shouldn't be allowed here since parsing types is different from
                    // parinsg expressions within this resolver, meaning this should be an error
                    //
                    // But also, this is literally impossible since only `nest` sections can
                    // actually access types, but expressions use types to check for if a value is
                    // searchable so is it still needed?
                    PossibleMember::Type(type_id) => {
                        todo!("Type id");
                    }
                    PossibleMember::Var(val_id) => {
                        unimplemented!("Nothing matches this case yet");
                    }
                    PossibleMember::Nothing => todo!("Unresolved"),
                }
            }
            Expr::StaticAccess(spanned_segments) => {
                let last_scope = self.resolve_static_access(
                    spanned_segments,
                    associated_scope,
                    scope_type,
                    false,
                    env,
                )?;

                let last_seg = &spanned_segments[spanned_segments.len() - 1];

                // This is a little odd, but it technically isn't different from if it were
                // classified as an expr in the first place. This is done since, making paths take
                // in a generic "Expr" would be an insanely large amount of possibilites for
                // something that is enforced at parse-time, making it more confusing. But,
                // creating inline expressions is also confusing so, not sure.
                let inline_expr = match &last_seg.kind {
                    PathSegment::Ident(interned_id) => {
                        SpannedExpr::new(Expr::Var(*interned_id), last_seg.span)
                    }
                    PathSegment::Generic(_) => {
                        let core_msg = "Generics are only usable in type expressions".to_string();
                        let src_diag = SourceDiagnostic::builder(
                            DiagnosticLevel::Error,
                            core_msg,
                            env.region.path_id,
                        )
                        .add_annotation(
                            last_seg.span,
                            AnnotationKind::Primary,
                            None,
                        );

                        return Err(PresetErr::General(src_diag));
                    }
                };

                self.register_expr(
                    parent_sym_id,
                    &inline_expr,
                    local_scope_id,
                    last_scope,
                    scope_type,
                    seen,
                    env,
                )
            }
            Expr::Array(array_expr) => {
                let mut array: Vec<ExprId> = Vec::new();

                let mut found_const_vals = 0;
                let mut type_id_opt = None;

                for sp_expr in &array_expr.elements {
                    // register as inputs?
                    let expr_id = self.register_expr(
                        parent_sym_id,
                        sp_expr,
                        local_scope_id,
                        associated_scope,
                        scope_type,
                        seen,
                        env,
                    )?;

                    let expr = &self.compiler.exprs[expr_id.id as usize];
                    let val_info = &self.compiler.values[expr.val_id.id as usize];

                    if val_info.const_val.is_some() {
                        found_const_vals += 1;
                    }

                    //WARN: Need to typecheck this too later
                    if type_id_opt.is_none() && !self.compiler.check_unknown(expr.type_id) {
                        type_id_opt = Some(expr.type_id);
                    }

                    array.push(expr_id);
                }

                let inputs = array.clone();

                let array_expr_id = ExprId::new(self.compiler.exprs.len() as u32);

                // Connecting all expressions to the array for resolution propagation purposes.
                //
                // Doing this loop AFTER pushing into the array because the registering of
                // expression ids would make it so the array indexes to the first element of the
                // array, rather than it's own position.
                for expr_id in &array {
                    let expr = &mut self.compiler.exprs[expr_id.id as usize];
                    expr.user = Some(array_expr_id);
                }

                let array_type_id = if let Some(inner_type_id) = type_id_opt {
                    inner_type_id
                } else {
                    let type_id = TypeId::new(self.compiler.types.len() as u32);
                    let ty_info = TypeInfo::new(Type::Unknown, env.current_mod);
                    self.compiler.types.push(ty_info);

                    type_id
                };

                let const_val_opt = if found_const_vals == array.len() {
                    let mut values: Vec<Value> = Vec::new();

                    for expr_id in &array {
                        let expr = &self.compiler.exprs[expr_id.id as usize];
                        let val_info = &self.compiler.values[expr.val_id.id as usize];
                        let val = val_info
                            .const_val
                            .as_ref()
                            .expect("Const value counting failed")
                            .clone();

                        values.push(val);
                    }

                    Some(Value::Array(values))
                } else {
                    None
                };

                // Um?
                let array_val_id = ValueId::new(self.compiler.values.len() as u32);
                let val_info = ValueInfo::new(array_type_id, array_expr_id, const_val_opt);

                let array_expr_hir = ExprHir::Array(array);

                let resolved_expr = ResolvedExpr::new(
                    array_type_id,
                    array_expr_hir,
                    array_val_id,
                    spanned_expr.span,
                    inputs,
                );

                self.compiler.values.push(val_info);
                self.compiler.exprs.push(resolved_expr);

                Ok(array_expr_id)
            }
        }
    }

    // Ok maybe this should be separated a bit more
    /// Method so that code can be re-used for traversing scopes in a static access.
    ///
    /// Takes in the segments to traverse, scope to start in, scope type for scoping rules, and
    /// whether or not type expression restrictions should be applied.
    ///
    /// Returns an `Ok` with the last scope found so that wherever this was called from can use the
    /// last segment for it's correct use-case.
    /// Returns an `Err` upon any errors, given whether or not a type expression was the caller.
    fn resolve_static_access(
        &mut self,
        spanned_path_segs: &[SpannedPathSegment],
        mut current_scope: AssociatedScopeKind,
        scope_type: ScopeType,
        in_ty_expr: bool,
        env: &ResolverEnv,
    ) -> Result<AssociatedScopeKind, PresetErr> {
        for (i, sp_path_seg) in spanned_path_segs.iter().enumerate() {
            match &sp_path_seg.kind {
                PathSegment::Ident(interned_id) => {
                    if let Some((sym_id, _)) = scopes::find_sym_id(
                        self.compiler,
                        current_scope,
                        *interned_id,
                        scope_type,
                        LookupPattern::NamespaceOnly,
                    ) {
                        let sym = &self.compiler.symbols[sym_id.id as usize];
                        match sym.associated_scope {
                            // Modules have their own symbol id for their given namespace so they
                            // can't be symbol checked..
                            Some(new_scope) => {
                                current_scope = new_scope;
                            }
                            // meaning the search is DONE
                            None => {
                                // If not at end AND there is no namespace associated with the
                                // current symbol
                                if i + 1 < spanned_path_segs.len() {
                                    let current_namespace = self.interner.search(*interned_id);

                                    let core_msg =
                                        format!("No namespace found in `{current_namespace}`");

                                    let src_diag = SourceDiagnostic::builder(
                                        DiagnosticLevel::Error,
                                        core_msg,
                                        env.region.path_id,
                                    )
                                    .add_annotation(
                                        sp_path_seg.span,
                                        AnnotationKind::Primary,
                                        None,
                                    );

                                    return Err(PresetErr::General(src_diag));
                                }
                                // Success case where the last symbol has no scope and the end was
                                // reached
                                // --------------------------------
                                // Drops to Ok
                            }
                        }
                        // Symbol not found
                    } else {
                        let current_namespace = self.interner.search(*interned_id);

                        let prev_namespace_opt = if i > 0 {
                            Some(&spanned_path_segs[i - 1])
                        } else {
                            None
                        };

                        // Different error message depending on if at least the first
                        // member was resolved or not
                        let src_diag = if let Some(prev) = prev_namespace_opt {
                            let prev_namespace = match &prev.kind {
                                PathSegment::Ident(prev_name_id) => {
                                    self.interner.search(*prev_name_id)
                                }
                                PathSegment::Generic(_) => {
                                    // Represents "module::Generic<T>::stuff" where the middle
                                    // generic has the ability to access members.
                                    // Which is not possible right now.
                                    unreachable!("Generics may never exist in this form.");
                                }
                            };

                            let core_msg = format!(
                                "Could not find the symbol `{}` in the namespace `{}`",
                                current_namespace, prev_namespace
                            );

                            SourceDiagnostic::builder(
                                DiagnosticLevel::Error,
                                core_msg,
                                env.region.path_id,
                            )
                            .add_annotation(
                                sp_path_seg.span,
                                AnnotationKind::Primary,
                                None,
                            )
                        } else {
                            let core_msg = format!(
                                "The symbol `{current_namespace}` was not found in all `{scope_type}` searchable scopes"
                            );

                            SourceDiagnostic::builder(
                                DiagnosticLevel::Error,
                                core_msg,
                                env.region.path_id,
                            )
                            .add_annotation(
                                sp_path_seg.span,
                                AnnotationKind::Primary,
                                None,
                            )
                        };

                        return Err(PresetErr::General(src_diag));
                    };
                }
                PathSegment::Generic(_) if in_ty_expr => {
                    // Still disallows something like, core.List<i32>.other_thing
                    if i + 1 != spanned_path_segs.len() {
                        let core_msg = "Generics cannot use `::` pathing at any point".to_string();
                        let src_diag = SourceDiagnostic::basic_builder(
                            DiagnosticLevel::Error,
                            core_msg,
                            env.region.path_id,
                            sp_path_seg.span,
                        );

                        return Err(PresetErr::General(src_diag));
                    }

                    break;
                }
                PathSegment::Generic(_) => {
                    let core_msg = "Generics cannot be used inside of expressions".to_string();
                    let src_diag = SourceDiagnostic::basic_builder(
                        DiagnosticLevel::Error,
                        core_msg,
                        env.region.path_id,
                        sp_path_seg.span,
                    );

                    return Err(PresetErr::General(src_diag));
                }
            }
        }

        Ok(current_scope)
    }

    // Umm...
    fn resolve_member(
        &mut self,
        sym_parent: SymbolId,
        member: &SpannedExpr,
        local_scope: Option<ScopeId>,
        associated_scope: AssociatedScopeKind,
        scope_type: ScopeType,
        seen: &mut Vec<SymbolId>,
        env: &ResolverEnv,
    ) -> Result<PossibleMember, PresetErr> {
        let res = self.register_expr(
            sym_parent,
            member,
            local_scope,
            associated_scope,
            scope_type,
            seen,
            env,
        )?;
        dbg!(res);
        panic!();

        if let Ok(expr_id) = self.register_expr(
            sym_parent,
            member,
            local_scope,
            associated_scope,
            scope_type,
            seen,
            env,
        ) {
            let resolved_expr = &self.compiler.exprs[expr_id.id as usize];

            todo!();
        }

        if let Expr::Var(name_id) = member.expr {
            if let Some(sym_id) = scopes::find_sym_id(
                self.compiler,
                todo!(),
                name_id,
                scope_type,
                LookupPattern::NoRestrictions,
            ) {
                todo!();
                // let type_id = self.compiler.symbols[sym_id.id as usize];
                // return Ok(PossibleMember::Type(type_id));
            } else {
                let msg = format!(
                    "Could not find the symbol `{}` as a module or value",
                    self.interner.search(name_id)
                );

                return Err(PresetErr::General(todo!()));
            }
        }

        Err(PresetErr::UndefinedMember(member.span))
    }

    /// Convenience method that takes an array of directives an evaluates as many as possible.
    ///
    /// If any of the directives given are invalid, they will be skipped, and a diagnostic will be
    /// created.
    ///
    /// Returns a tuple of both the the any directives and diagnostics found
    fn handle_directives(
        &self,
        abs_directives: &[AbstractDirective],
        env: &ResolverEnv,
    ) -> (Vec<SpannedContainer<DirectiveId>>, Vec<PresetErr>) {
        let mut directive_ids = Vec::new();
        let mut preset_errs = Vec::new();

        // Collecting all possible directives while also collecting and preset errors
        for abs_directive in abs_directives {
            match Directive::try_from_interned_str(abs_directive.sp_name_id.inner) {
                // Trying REALLY hard not to use the shortened "dir" for this
                Some(dir) => {
                    let directive_id = script_compiler::directive_to_id(&dir);
                    let sp_directive_id =
                        SpannedContainer::new(directive_id, abs_directive.sp_name_id.span);

                    directive_ids.push(sp_directive_id);
                }
                None => {
                    let preset_err = PresetErr::UnknownDirective(abs_directive.sp_name_id.clone());
                    preset_errs.push(preset_err);
                }
            };
        }

        (directive_ids, preset_errs)
    }

    // Helper
    fn check_cycle(
        &self,
        seen: &Vec<SymbolId>,
        parent_sym_id: SymbolId,
        found_sym_id: SymbolId,
        env: &ResolverEnv,
    ) -> Result<(), PresetErr> {
        for seen_sym_id in seen.iter() {
            // In:
            // ```
            // let a = b
            // let b = c
            // let c = b
            // ```
            // Within b, it checks of the symbol a is inside of `TypeContext`, and
            // if that a depends on symbol b
            if let Some(pending_sym) = self.ty_ctx.sym_queue.get(seen_sym_id) {
                let has_cycle = pending_sym
                    .pending_exprs
                    .iter()
                    .any(|pend_expr| pend_expr.parent_sym == found_sym_id);

                // In, "let a = b, let b = a"
                // a would be cycled
                // b would be current
                //TODO: Would be changed if symbols were given Option span directly
                if has_cycle {
                    let current_sym = &self.compiler.symbols[parent_sym_id.id as usize];
                    let current_name = self.interner.search(current_sym.name_id);
                    let current_ast_id = current_sym.ast_id.expect("core should not be resolved");

                    let cycled_sym = &self.compiler.symbols[found_sym_id.id as usize];
                    let cycled_ast_id = cycled_sym.ast_id.expect("core should not be resolved");
                    let cycled_name = self.interner.search(cycled_sym.name_id);

                    let cycled_span = env.ast_info.get_sym_span(cycled_ast_id);
                    let current_span = env.ast_info.get_sym_span(current_ast_id);

                    let core_msg = format!(
                        "`{}` depends on itself through `{}`",
                        current_name, cycled_name
                    );

                    let src_diag = SourceDiagnostic::builder(
                        DiagnosticLevel::Error,
                        core_msg,
                        env.region.path_id,
                    )
                    .add_annotation(
                        cycled_span,
                        AnnotationKind::Secondary,
                        "This has no value yet".to_string().into(),
                    )
                    .add_annotation(
                        current_span,
                        AnnotationKind::Primary,
                        format!("Uses `{cycled_name}` before it has a value").into(),
                    );

                    return Err(PresetErr::General(src_diag));
                }
            }
        }

        Ok(())
    }

    //FIX: This should be removed or shortened
    /// - active_mod_id: The target module to search which is only altered if an external module is
    /// used within a member access
    /// - spanned_ty_expr: The type expression to be resolved
    /// - scope_type: The scope which determines how much of a module can be searched.
    /// - lookup_pattern: The type of lookup which is recursively changed depending on if a direct
    /// member access is being searched, or if a library such as core can be searched externally.
    fn resolve_type_expr(
        &mut self,
        // Module that is actively being searched within, not the source. Source remains
        // current_mod
        associated_scope: AssociatedScopeKind,
        sp_ty_expr: &SpannedTypeExpr,
        scope_type: ScopeType,
        lookup_pattern: LookupPattern,
        env: &ResolverEnv,
    ) -> Result<TypeId, PresetErr> {
        match &sp_ty_expr.ty_expr {
            //FIXME: If an error occurs while env.current_mod = extern_mod, it tries to report the
            //error from the external module instead of the actual module of origin.
            TypeExpr::Var(name_id) => {
                // Searching symbols because otherwise, the type of a variable would be valid
                // since it would just be looking at it's type, which is not a favorable allowable syntax
                // So, let x = 3, var-> field: x, would be valid if this weren't handled at the
                // symbol level here
                match scopes::find_sym_id(
                    self.compiler,
                    associated_scope,
                    *name_id,
                    scope_type,
                    lookup_pattern,
                ) {
                    Some((sym_id, _)) => {
                        match self.compiler.symbols[sym_id.id as usize].kind {
                            SymbolKind::Type(type_id) => {
                                // NOTE: Will probably error later in resolution but fine for now
                                let symbol = &self.compiler.symbols[sym_id.id as usize];

                                if let SymbolOrigin::Module(mod_origin_id) = symbol.sym_origin {
                                    if symbol.is_priv && mod_origin_id != env.current_mod {
                                        //FIX: Would need changes
                                        let current_mod = &self.compiler.mods[env.current_mod.id];
                                        let current_mod_name =
                                            self.interner.search(current_mod.name_id);
                                        let sym_name = self.interner.search(symbol.name_id);

                                        let core_msg = format!(
                                            "The type `{sym_name}` is private within namespace `{current_mod_name}`"
                                        );

                                        let src_diag = SourceDiagnostic::builder(
                                            DiagnosticLevel::Error,
                                            core_msg,
                                            env.region.path_id,
                                        )
                                        .add_annotation(
                                            sp_ty_expr.span,
                                            AnnotationKind::Primary,
                                            None,
                                        )
                                        .add_note(
                                            "Types declared can be exported if that was unintended"
                                                .into(),
                                        );

                                        return Err(PresetErr::General(src_diag));
                                    }
                                }

                                return Ok(type_id);
                            }
                            // Ok but what about, "core is a MODULE which is NOT a type?"
                            SymbolKind::Module(mod_id) => (),
                            SymbolKind::Variable(_) => (),
                            // Ok ok, but what about, "#warn is a DIRECTIVE which is NOT a type?"
                            SymbolKind::Directive(_) => (),
                            SymbolKind::Config(_) => unreachable!("Cannot lookup configs"),
                        }
                    }
                    None => (),
                }
                // Case of not finding any symbol

                // If we have main, that imports def, that imports other, it tries to search for
                // things in the "other" module even though it's defined in "def".
                //
                // Within "def", it tries to search "other" for everything declared even if "other"
                // is never used.
                let err_name = self.interner.search(*name_id);
                let core_msg = match associated_scope {
                    AssociatedScopeKind::Module(mod_id) => {
                        let err_mod = &self.compiler.mods[mod_id.id];
                        let err_mod_name = self.interner.search(err_mod.name_id);

                        format!(
                            "`{err_name}` is not defined as a type within the module `{err_mod_name}`"
                        )
                    }
                    AssociatedScopeKind::Scope(scope_id) => {
                        let scope_info = &self.compiler.scopes[scope_id.id];
                        // This is infailable because an associated scope having a scope variant
                        // means that the current search was performed by a namespace within a
                        // module, not a module directly.
                        let sym_owner = scope_info
                            .sym_owner
                            .expect("resolve_type_expr control flow broke");
                        let sym_name_id = self.compiler.symbols[sym_owner.id as usize].name_id;
                        let sym_name = self.interner.search(sym_name_id);

                        format!(
                            "The symbol `{sym_name}` does not contain a type with the the identifier `{err_name}`"
                        )
                    }
                };

                let src_diag = SourceDiagnostic::basic_builder(
                    DiagnosticLevel::Error,
                    core_msg,
                    env.region.path_id,
                    sp_ty_expr.span,
                );
                Err(PresetErr::General(src_diag))
            }
            // Generics can only be these types so this can stay for now
            TypeExpr::Generic(generic) => {
                //FIX: This is still using the old id matching but maybe it's ok since this is
                // actually supposed to be specifically only known data structures
                match BuiltinTypeKind::try_from_interned_id(generic.base.id) {
                    // Self referential type ids used here
                    Some(kind) => match kind {
                        BuiltinTypeKind::List | BuiltinTypeKind::Set => {
                            if generic.args.len() != 1 {
                                let core_msg =
                                    format!("Expected only 1 type within `{}`", kind.to_fmt());

                                let src_diag = SourceDiagnostic::basic_builder(
                                    DiagnosticLevel::Error,
                                    core_msg,
                                    env.region.path_id,
                                    sp_ty_expr.span,
                                );

                                return Err(PresetErr::General(src_diag));
                            }

                            let inner = self.resolve_type_expr(
                                associated_scope,
                                &generic.args[0],
                                scope_type,
                                LookupPattern::NoRestrictions,
                                env,
                            )?;

                            let ty = if kind == BuiltinTypeKind::List {
                                Type::BuiltinType(BuiltinType::List(inner))
                            } else {
                                Type::BuiltinType(BuiltinType::Set(inner))
                            };

                            let type_id = TypeId::new(self.compiler.types.len() as u32);

                            // TODO: Technically it's a structure owned by core, but it wasn't
                            // defined as core, but this can't be referenced directly anyways so it
                            // doesn't really make a difference
                            let ty_info =
                                TypeInfo::new(ty, self.compiler.intrinsic_registry.core_mod_id);
                            self.compiler.types.push(ty_info);

                            return Ok(type_id);
                        }
                        BuiltinTypeKind::Tuple => {
                            let mut elements: Vec<TypeId> = Vec::new();

                            for arg in &generic.args {
                                elements.push(self.resolve_type_expr(
                                    associated_scope,
                                    arg,
                                    scope_type,
                                    LookupPattern::NoRestrictions,
                                    env,
                                )?);
                            }

                            let type_id = TypeId::new(self.compiler.types.len() as u32);
                            let tuple = Type::BuiltinType(BuiltinType::Tuple(elements));

                            let ty_info =
                                TypeInfo::new(tuple, self.compiler.intrinsic_registry.core_mod_id);
                            self.compiler.types.push(ty_info);

                            return Ok(type_id);
                        }
                        BuiltinTypeKind::Map => {
                            if generic.args.len() != 2 {
                                let core_msg = format!("Expected only 2 types within `Map`",);
                                let src_diag = SourceDiagnostic::basic_builder(
                                    DiagnosticLevel::Error,
                                    core_msg,
                                    env.region.path_id,
                                    sp_ty_expr.span,
                                );

                                return Err(PresetErr::General(src_diag));
                            }

                            // Should it reset to current module if it has a new sesarch started?
                            let key = self.resolve_type_expr(
                                AssociatedScopeKind::Module(env.current_mod),
                                &generic.args[0],
                                scope_type,
                                LookupPattern::NoRestrictions,
                                env,
                            )?;

                            let val = self.resolve_type_expr(
                                AssociatedScopeKind::Module(env.current_mod),
                                &generic.args[1],
                                scope_type,
                                LookupPattern::NoRestrictions,
                                env,
                            )?;

                            let map = Type::BuiltinType(BuiltinType::Map(key, val));
                            let map_id = self.compiler.types.len() as u32;

                            let ty_info =
                                TypeInfo::new(map, self.compiler.intrinsic_registry.core_mod_id);
                            self.compiler.types.push(ty_info);

                            return Ok(TypeId::new(map_id));
                        }
                        // Returns nothing since both have the same error handling
                        _ => (),
                    },
                    None => (),
                }

                let err_name = self.interner.search(generic.base);

                let core_msg = format!(
                    "Found identifier \"{err_name}\" before generic parameters, but only `List`, `Set`, `Map`, and `Tuple` are valid data structures"
                );

                // No error codes please
                let src_diag = SourceDiagnostic::builder(
                    DiagnosticLevel::Error,
                    core_msg,
                    env.region.path_id,
                )
                .add_annotation(sp_ty_expr.span, AnnotationKind::Primary, None)
                .add_note(
                    "Generics and data structures are only usable through language primitives"
                        .into(),
                );
                Err(PresetErr::General(src_diag))
            }
            // This only allows something like, defs.Thing which can go to at most one type deep,
            // but no more. Will need change since something like i32.MAX could be "core.i32.MAX".
            //
            // Maybe not though since that would only be usable in expressions anyways which aren't
            // type expressions
            TypeExpr::Path(sp_path_segs) => {
                // maybe active_mod can be removed?
                let last_scope = self.resolve_static_access(
                    &sp_path_segs,
                    associated_scope,
                    scope_type,
                    true,
                    env,
                )?;
                let last_segment = &sp_path_segs[sp_path_segs.len() - 1];

                let inline_ty_expr = match &last_segment.kind {
                    PathSegment::Ident(interned_id) => {
                        SpannedTypeExpr::new(TypeExpr::Var(*interned_id), last_segment.span)
                    }
                    PathSegment::Generic(generic) => {
                        //FIXME: EVIL CLONING.
                        //Would need to a compability layer to allow for referenced inners, rather
                        //than only owned.
                        SpannedTypeExpr::new(TypeExpr::Generic(generic.clone()), last_segment.span)
                    }
                };

                self.resolve_type_expr(last_scope, &inline_ty_expr, scope_type, lookup_pattern, env)
            }
        }
    }
}
