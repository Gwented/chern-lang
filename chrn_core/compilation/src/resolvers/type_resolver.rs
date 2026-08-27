// Please split this...
// No
// Artisinal hand-coded slop
//! The reason there's so much code in this one file is because this resolution stage is supposed to
//! handle all deep semantic properties that need tracking machinery, and certain other type
//! resolution based parts.
//!
//! This entire file could very well be split into different resolution stages where now,
//! `TypeContext` is simply a composed set of pure functions that have a check and graph updating stage,
//! but that would also mean that later stages must iterate through all compilation units again,
//! then again, then again depending on the composition which seems wasteful, and is objectively
//! slower, just for the sake of composition. Iteration could be reduced by sorting and then each
//! composed stage goes through the different portions it wants, but that's still more allocations
//! and contexts to account for that don't really NEED to exist.
//!
//! May change in the future but right now this seems reasonable enough, even with the 4K+ LOC
mod cfg_ctx;
pub mod type_context;

use chrn_utils::chrn_config::ChrnConfig;
use chrn_utils::chrn_config::chrn_perf::ChrnPerfStage;
use chrn_utils::err_codes::ErrorCode;
use chrn_utils::id_types::{
    AstId, DirectiveId, ExprId, ImplId, ImplMemberId, InternedId, MemberId, ScopeId, SymbolId,
    TypeId, ValueId, VariableId,
};
use chrn_utils::intern::Intern;
use chrn_utils::source_map::source_diagnostic::annotations::AnnotationKind;
use chrn_utils::source_map::source_diagnostic::{
    DiagnosticLevel, SourceDiagnostic, SourceDiagnosticSink, SourceDiagnosticSummary,
};
use chrn_utils::source_map::source_span::{self, SourceSpan};
use chrn_utils::utils::containers::{SpannedContainer, SpannedContainerRef};
use lang::chrn_classifier::ChrnClassifier;
use lang::values::Value;

use crate::constraints::ArgConstraint;
use crate::lookup::member_lookup::{self, MemberLookupPattern, MemberLookupResult};
use crate::lookup::scopes;
use crate::lookup::scopes::scopes_concepts::{
    AssociatedScopeKind, ScopeLookupPattern, ScopeLookupPreferenceFlags, ScopeType,
    SymbolLookupOutput,
};
use crate::parser::ast::ast_concepts::{AbstractConfig, AbstractConfigKind, AbstractDirective};
use crate::parser::ast::ast_exprs::{AstExpr, PathSegment, SpannedExpr, TypeExpr};
use crate::parser::ast::ast_stmts::AbstractStmt;
use crate::resolvers::resolver_env::ResolverEnv;
use crate::resolvers::resolver_state::ResolverState;
use crate::resolvers::type_resolver::cfg_ctx::{
    ConfigMemberComplexContext, ConfigMemberContextKind, ConfigMemberOutput,
};
use crate::resolvers::typechecker;
use crate::script_compiler::{ScriptCompiler, compiler_constants};
use crate::semantic::checker_helpers::{DuplicateIdentResult, DuplicateTracker};
use crate::semantic::compilation_unit::CompilationUnit;
use crate::semantic::evaluator::UnaryOpResult;
use crate::semantic::hir::hir_concepts::{Type, TypeInfo};
use crate::semantic::hir::hir_exprs::{
    ExprHir, Param, PossibleMember, ResolvedExpr, ResolvedExprMetadata,
};
use crate::semantic::hir::hir_impls::{
    ConfigDefMember, ImplHirKind, ImplMemberKind, OptionAssignmentMember, OptionAssignmentRoot,
};
use crate::semantic::hir::hir_symbols::{
    Symbol, SymbolKind, SymbolOrigin, VarDef, VariableMetadata, VariableState,
};
use crate::semantic::hir::value_info::ValueInfo;
use crate::semantic::preset_reporter::preset_err::{LookupError, MathError, PresetErr};
use crate::semantic::resolution::resolution_concepts::StaticAccessOption;
use crate::semantic::resolution::resolution_helpers;
use crate::semantic::{checker_helpers, evaluator, inference, preset_reporter, resolution};

use crate::resolvers::type_resolver::type_context::{
    ParentInfo, ParentState, ParentStateBase, PendingExpr, PendingExprKind, PendingSymbol,
    StandingExprState, TypeContext,
};

/// Resolves types and builds the rest of any structs, enums, or expressions that can be const
/// evaluated. Does so by mutating the compiler given, and maintaining context to retain it's last
/// state.
pub struct TypeResolver<'a> {
    cfg: &'a mut ChrnConfig,
    interner: &'a mut Intern,
    compiler: &'a mut ScriptCompiler,
    ty_ctx: TypeContext,
    summary: SourceDiagnosticSummary,
}

impl<'res> TypeResolver<'res> {
    /// Instantiation requires that the compiler's state is valid and will panic otherwise
    pub fn new(
        cfg: &'res mut ChrnConfig,
        interner: &'res mut Intern,
        compiler: &'res mut ScriptCompiler,
    ) -> TypeResolver<'res> {
        debug_assert_eq!(ResolverState::TYPE, compiler.resolver_state);
        compiler.resolver_state.advance();
        TypeResolver {
            cfg,
            ty_ctx: TypeContext::new(),
            summary: SourceDiagnosticSummary::default(),
            interner,
            compiler,
        }
    }

    //TODO: Refactor complex. It should only allow for the top-level type to mutate parts like it's
    //acutal type identifier and casing. The inner should only go one layer deep inside the type
    //itself.
    //For example:
    //```
    //nest->
    //  struct Point {state1: State, state2: State}
    //  enum State {Low, Medium, High: i32}
    //complex->
    //  // CANNOT go any deeper. It can only mutate the field/variant naming and default value if
    //  present, but not the type of the inner itself in any regard.
    //  Point {x {} y {} }
    //
    //```
    //
    //The issue with this going deeper right now is that if x and y can mutate type `State`, who
    //takes priority? Do they just append to each others properties?
    //The biggest issue is actually that the split between who can implement is an unnecessary
    //complexity which turns a simple config into a question of if the behavior being seen in
    //serialized behavior is because more than one complex declaration configurates at the same time.
    //
    //This probably means that something like, "other::x {}" needs to be allowed, or allow other
    //modules to define config and keep that in mind for the script file/block being compiled.
    // Perhaps, if you re-configure from another module you can override, but not sure.
    //
    // Current idea is just: Only one layer deep of configs, configs are isolates, and maybe
    // cross-module config declarations.

    /// Mutates inner `ScriptCompiler` and `TypeContext` given the `env`.
    ///
    /// * `env`: The current environment the resolver is operating in. This being passed in
    /// explicitly allows for `TypeResolver` to maintain it's state throughout resolution while
    /// mutating off of given envs.
    pub fn resolve<'env>(&mut self, env: &'env ResolverEnv) -> SourceDiagnosticSummary {
        self.cfg.perf_tracker_mut().start();
        // Re-used hashet when identifiers are checked, like for configs, alias params, etc.
        let mut ident_tracker: DuplicateTracker<SpannedContainer<InternedId>> =
            DuplicateTracker::with_capacities(4, 0);

        // Everything skipped is not a factor in this compilation step.
        for comp_unit in env.compilation_syms.iter().cloned() {
            match comp_unit {
                CompilationUnit::Symbol(sym_id) => {
                    match self.compiler.symbols[sym_id].kind {
                        // This split is more so, users can define these set of symbols, and users cannot
                        // define the unreachables
                        SymbolKind::Type(type_id) => match &self.compiler.types[type_id].ty {
                            Type::Struct(_) => self.resolve_struct(sym_id, env),
                            Type::Enum(_) => self.resolve_enum(sym_id, env),
                            Type::Alias(_) => self.resolve_alias(sym_id, &mut ident_tracker, env),
                            Type::TypeDef(_) => self.resolve_typedef(sym_id, env),
                            // Not sure about this right now
                            // New functions cannot be declared as symbols, only the compiler creates them.
                            // None of these can be user-defined, but exist internally.
                            Type::Deferred(_)
                            | Type::Func(_)
                            | Type::Boundaries(_)
                            | Type::Unknown
                            | Type::BuiltinTypeInfo(_) => {
                                unreachable!()
                            }
                        },
                        // Still uses sym id since their actual ids make it a little more complicated to get
                        // to their ast id
                        SymbolKind::Variable(_) => self.resolve_var(sym_id, env),
                        // Users cannot define these but they exist internally.
                        SymbolKind::ExternType
                        | SymbolKind::Namespace
                        | SymbolKind::Directive(_) => unreachable!(),
                    }
                }
                CompilationUnit::Impl(impl_id) => match self.compiler.impls[impl_id].kind {
                    ImplHirKind::Config(_) => {
                        self.resolve_cfg_root(impl_id, &mut ident_tracker, env)
                    }
                },
            }
            ident_tracker.clear();
        }

        //NOTE: TRYING TO COMPRESS THE EXPLANATION BELOW.
        // This expression resolution dependency tracking system tracks whether the type or const value is
        // resolved using a T/F state. These two booleans are in held by `PendingSymbol`. This
        // structure is a `TypeContext` specific structure that only exists within the context of
        // resolving expression, as it is merely a wrapper around the expr for metadata
        // purposes (More info in struct's doc). The `Vec<PendingExpr>` stores all exprs that depend
        // on the pending symbol.
        //
        // Pending exprs register themselves by storing themselves inside the pending symbols list.
        // If we have:
        // ```
        // let a = b
        // let b = 3
        // ```
        // The resolution ends with b being having a const val of 3, and a having no value or type
        // because b wasn't resolved yet. But, when a was seen, a created the `PendingSymbol` for b,
        // then it put itself inside b's pending exprs as `b -> [a]`. If we also had "let c = b"
        // c would check if b is already a pending symbol, see that it is, push itself, with b now
        // having [a, c].
        // After b is seen, and has at least a const type and possibly a const value, b sets `needs_check`
        // to true in `TypeContext`, which is ONLY checked if new information attached to `TypeContext` is
        // found, otherwise it stays false and no checks are done to avoid wasting compute.
        // The loop then goes through all pending symbols, and checks the ones that have at least a
        // resolved type, as to also not waste compute. When the loop gets to b's pending symbol, it
        // finds `a`, which has a `ParentState::Unresolved`, meaning it should have it's resolution
        // attempted since we know `b` can help resolve `a`. Resolution traverses "let a = b" by
        // starting with the root `b`. The resolution ALWAYS starts by resolving the root, because
        // the root is always the pending symbol. Since in this scenario, "let a = b" only have b,
        // that means solving the root intrinsically solves a
        //
        //
        //
        // The ore idea behind this system is that everything uses a tree which ALWAYS start at the
        // root and stop at a `None` point
        // Given:
        // ```
        // let a = b + d // b -> b + d -> None | d -> b + d -> None
        // let b = c + e // c -> c + e -> None | e -> c + e -> None
        // let c = e // e -> None
        // let d = b // b -> None
        // let e = 3 // Const
        // ```
        // Every root has one job, go as far as possible up the tree.
        // If we have "let a = b + c" and `c` is unresolved, b resolves itself, goes b -> b + c, sees
        // that c is unresolved, then stops. If c is resolved, c -> b + c, c sees that both b and
        // itself are resolved, it creates the const value, c -> None, returns. This process
        // involves mutated already stored addresses, so this root's only job is to repair
        // everything that was not yet resolved.
        //
        //NOTE: TRYING TO COMPRESS THE EXPLANATION BELOW.

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
            // let name = self.interner.search(sym.name_id );
            // dbg!(name);
            //actually resolved already.
            self.ty_ctx.needs_check = false;
            // Giving ownership to a variable since the traversal chosen needs mutation while
            // traversing
            let mut pending_syms: Vec<(SymbolId, PendingSymbol)> =
                Vec::with_capacity(self.ty_ctx.sym_queue.len());

            pending_syms.extend(self.ty_ctx.sym_queue.drain());

            let mut removable_syms: Vec<SymbolId> = Vec::new();

            for (sym_id, pending_sym) in &mut pending_syms {
                // If there is no resolved type then there cannot be a const value
                if !pending_sym.has_resolved_ty {
                    continue;
                }

                match self.try_resolve_pending(*sym_id, pending_sym, env) {
                    //TODO: Can something be done with these?
                    //Succeeding just means no errors occurred, not that new information was found,
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
            //
            // We're allocating this each time, maybe we should declare this outside so that it can be
            // re-used
            let mut resolved_parents: Vec<(SymbolId, usize, ParentInfo)> = Vec::new();

            // Also needs to check if there exists a pending symbol which has ONLY stale
            // expressions inside, meaning it should be removed.

            // Finding all parents that recieved new information by checking if a pending expr has
            // the `Resolved` variant.
            for (pending_sym_id, pending_sym) in &self.ty_ctx.sym_queue {
                for (i, pending_expr) in pending_sym.pending_exprs.iter().enumerate() {
                    match &pending_expr.kind {
                        // If the pending expr has a parent base that means it can be updated
                        PendingExprKind::Parent(parent_base) => {
                            //NOTE: Assuming I'm not hallucinating, this is, on every
                            // iteration, checking all pending symbols, and if a pending expr inside
                            // of it is set as resolved, which can only happen in `traverse_expr`,
                            // it checks if the parent exists inside the symbol queue, if it does
                            // it alters the parent state to notified so that this loop doesn't
                            // increment resolved count. It's only changed from `Notified` to `Resolved`
                            // if  `traverse_expr` change it to `Resolved`. Since `Notified` is ONLY
                            // set if the `Resolved` state is accounted for, this prevents the loop
                            // from being infinite.
                            if let ParentState::Resolved {
                                has_resolved_ty,
                                has_const_val,
                            } = parent_base.state
                            {
                                // if it is within sym_queue then we should update it
                                if self
                                    .ty_ctx
                                    .sym_queue
                                    .contains_key(&parent_base.parent_sym_id)
                                {
                                    current_resolved_count += 1;
                                    let parent_info = ParentInfo::new(
                                        parent_base.parent_sym_id,
                                        has_resolved_ty,
                                        has_const_val,
                                    );

                                    resolved_parents.push((*pending_sym_id, i, parent_info));
                                }
                            }
                        }
                        //TEST:
                        //Doesn't need to update anything since it's a "Standing" expr which has no
                        //parent attached.
                        PendingExprKind::Standing(_) => (),
                    }
                }
            }

            // Loop that sets whatever resolution information regarding the parent to
            // true, so that it can actually be accounted for as a resolved pending symbol. Pending
            // symbol's expressions are never attempted for resolution unless they are marked to at
            // least have a resolved type. So, resolution trigerring is lazy and fully dependent on
            // signals.
            //
            // All are expects since the previous loop only builds up info that guarantees these
            // parts exist.
            //WARN: As of right now, this **ONLY** accounts for a parent attached pending expr.
            //If this were to ever need to expand this would probably be delegated to a method of
            //some sort so that it's a clear encoded operation rather than an inline loop.
            for (pending_sym_id, pending_expr_idx, parent_info) in resolved_parents {
                // Setting expr to `Notified`
                let pending_sym = self
                    .ty_ctx
                    .sym_queue
                    .get_mut(&pending_sym_id)
                    .expect("Previous loop failed");

                let PendingExprKind::Parent(parent_base) =
                    &mut pending_sym.pending_exprs[pending_expr_idx].kind
                else {
                    unreachable!()
                };

                parent_base.state = ParentState::Notified {
                    has_resolved_ty: parent_info.has_resolved_ty,
                    has_const_val: parent_info.has_const_val,
                };

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
            //
            // These are removed since if (for some reason) there are a lot of expressions that
            // need resolution tracked, this would hold memory longer than required.
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
        //         let name = self.interner.search(symbol.name_id );
        //         let ty = &self.compiler.types[type_id ];
        //         dbg!(name, &ty.ty);
        //     }
        //     SymbolKind::Val(value_id) => {
        //         let name = self.interner.search(symbol.name_id );
        //         let val_info = &self.compiler.values[value_id ];
        //         let ty_info = &self.compiler.types[val_info.type_id ];
        //
        //         dbg!(name, ty_info);
        //     }
        //     _ => todo!(),
        // };

        // if env.current_mod == self.compiler.mods[ModuleId::new(self.compiler.mods.len() - 2)].mod_id
        // {
        //     dbg!(&self.ty_ctx);
        //     for symbol in &self.compiler.symbols {
        //         if self.interner.search(symbol.name_id) == "y" {
        //             let name = self.interner.search(symbol.name_id);
        //             dbg!(name);
        //             match symbol.kind {
        //                 SymbolKind::Variable(var_id) => {
        //                     let state = &self.compiler.variables[var_id].state;
        //                     match state {
        //                         VariableState::ReservedTypeSlot(type_id) => {
        //                             dbg!("Reserved variable but not seen");
        //                         }
        //                         VariableState::Known(val_id) => {
        //                             let val = &self.compiler.values[*val_id];
        //                             let expr = &self.compiler.exprs[val.expr_id];
        //                             // dbg!(expr.val_id, expr);
        //                             dbg!(expr, val);
        //                         }
        //                     }
        //                 }
        //                 SymbolKind::Type(type_id) => {
        //                     let ty_info = &self.compiler.types[type_id];
        //                     match &ty_info.ty {
        //                         Type::BuiltinType(builtin_type) => {
        //                             dbg!(builtin_type);
        //                         }
        //                         Type::Struct(struct_def) => todo!(),
        //                         Type::Enum(enum_def) => todo!(),
        //                         Type::Func(func_def) => todo!(),
        //                         Type::Alias(alias_def) => todo!(),
        //                         Type::TypeDef(type_def) => {
        //                             let ty = &self.compiler.types[type_def.type_id];
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

        self.cfg
            .perf_tracker_mut()
            .stop(ChrnPerfStage::TypeResolver);

        let mut summary = SourceDiagnosticSummary::default();
        summary.append_summary(&mut self.summary);
        summary
    }

    //TODO: When override is done add it's routing to this. Please.
    //
    //
    // The lifetime used here is needed so that the vectors that are pushed into during the recursive
    // maintaining of seen identifiers know that their shortest lifetime is more than long enough to
    // where the borrow cheker is satisfied.
    fn resolve_cfg_root<'env>(
        &mut self,
        parent_impl_id: ImplId,
        ident_tracker: &mut DuplicateTracker<SpannedContainer<InternedId>>,
        env: &'env ResolverEnv,
    ) {
        let initial_scope = AssociatedScopeKind::Module(env.current_mod);
        let ast_id = self.compiler.impls[parent_impl_id]
            .ast_id
            .expect("Should be user impls only");
        let abs_cfg_root = env.ast_info.get_cfg_root(ast_id);

        let scope_type = self.compiler.impls[parent_impl_id].scope_origin;
        let lookup_pat = abs_cfg_root.lookup_pat;

        let AbstractConfigKind::Root(sp_path_segs) = &abs_cfg_root.kind else {
            unreachable!()
        };

        // TODO: Complex can only take in type expressions and lookup types.
        // Override can do the same type lookup, while also being able to use it's intrinsic
        // namespaces like PYTHON.
        // This means that this config handling needs to encode accounting for the scope type in
        // regards to which can actually consume what.
        //
        // The current idea is to on override, search for the symbol with a preference of namspace OR
        // type, and for complex search for only namespace. But the difficult part is that one wants
        // a type id, one wants a symbol, so should it be a kind? Should the type id retrieval be
        // derived from config kind and extracted through known methods, or just keep TypeId?
        // I LOVE SPENDING AN INCONSIDERABLE AMOUNT OF TIME ON SEMANTIC QUESTIONS. JUST WRITE. THE.
        // CODE.
        // Ok :(

        //TODO: We need to get the scoping, given the pathing, and then search for the symbol id in
        //the given scope.
        let (lookup_pref, static_access_opt) = if lookup_pat == ScopeLookupPattern::OnlyIntrinsic {
            // Not that important, but since intrinsic only points to namespaces only (as of right
            // now at least) this is done
            let pref =
                ScopeLookupPreferenceFlags::new(ScopeLookupPreferenceFlags::NAMESPACE.into());
            (pref, StaticAccessOption::None)
        } else {
            let pref = ScopeLookupPreferenceFlags::new(
                (ScopeLookupPreferenceFlags::TYPE | ScopeLookupPreferenceFlags::NAMESPACE).into(),
            );
            (pref, StaticAccessOption::None)
        };

        let last_scope = match resolution_helpers::resolve_static_access_ret_preset(
            self.compiler,
            sp_path_segs,
            initial_scope,
            scope_type,
            lookup_pref,
            static_access_opt,
            self.interner,
            env,
        ) {
            Ok(scope) => scope,
            Err(preset_err) => {
                preset_reporter::report_preset(
                    self.compiler,
                    &mut self.summary,
                    preset_err,
                    env.region,
                    self.cfg,
                    self.interner,
                );
                // No valid symbol exists so nothing can actually be done here. Could typecheck the
                // options since they're always valid to check but not now.
                return;
            }
        };

        // Getting last segment to search in the static path
        let last_seg = &sp_path_segs[sp_path_segs.len() - 1];

        // Check if it's a namespace or type (that's contextually valid)
        // Could be generic so can't go by scopes::find_sym_id
        let sym_id_opt = match &last_seg.inner {
            PathSegment::Ident(interned_id) => {
                //TODO: This preset err for this maybe
                // Java uppser is being searched here, maybe because the previous is java
                // upper itself, and it's basically asking, is java upper in java upper,
                // given that we are inside java upper?
                match scopes::find_sym_id(
                    self.compiler,
                    last_scope,
                    *interned_id,
                    scope_type,
                    lookup_pat,
                    lookup_pref,
                ) {
                    Some(SymbolLookupOutput { found_sym_id, .. }) => found_sym_id.into(),
                    None => {
                        let lookup_err = LookupError::SymbolNotFound {
                            sp_invalid_name_id: SpannedContainer::new(*interned_id, last_seg.span),
                            scope_searched: last_scope,
                        };
                        preset_reporter::report_preset(
                            self.compiler,
                            &mut self.summary,
                            lookup_err.into(),
                            env.region,
                            self.cfg,
                            self.interner,
                        );
                        None
                    }
                }
            }
            PathSegment::Generic(_) => {
                let builder = SourceDiagnostic::builder(
                    ErrorCode::GenericsErr.into(),
                    DiagnosticLevel::Error,
                    "Config root must be a user defined type",
                    env.region.path_id,
                )
                .add_annotation(last_seg.span, AnnotationKind::Primary, None);
                self.summary.push_diag(builder.build());
                None
            }
        };

        // Is `Option` so the option exprs can be validated before exiting since those don't care
        // about whether or not the symbol or type id valid
        //
        // Is mutable so that if the typecheck fails, it can be set to `None`, which then allows for
        // the same return signal to be used on failure.

        let mut memb_stmts: Vec<ImplMemberId> = Vec::with_capacity(abs_cfg_root.abs_stmts.len());

        for abs_stmt in &abs_cfg_root.abs_stmts {
            //TODO: Um, method!
            match abs_stmt {
                AbstractStmt::OptAssignment(opt) => {
                    let sp_name_id = SpannedContainer::new(opt.name_id, opt.name_span);
                    ident_tracker.insert_or_store(sp_name_id);

                    let expr_id = match self.register_expr(
                        None,
                        &opt.array_expr,
                        None,
                        last_scope,
                        scope_type,
                        env,
                    ) {
                        Ok(expr_id) => expr_id,
                        Err(preset_err) => {
                            preset_reporter::report_preset(
                                &self.compiler,
                                &mut self.summary,
                                preset_err,
                                env.region,
                                self.cfg,
                                self.interner,
                            );
                            continue;
                        }
                    };

                    let impl_memb_id = ImplMemberId::new(self.compiler.impl_members.len() as u32);
                    let opt = OptionAssignmentRoot::new(
                        parent_impl_id,
                        impl_memb_id,
                        opt.name_id,
                        opt.name_span,
                        expr_id,
                    );

                    self.compiler
                        .impl_members
                        .push(ImplMemberKind::OptAssignmentRoot(opt));
                    memb_stmts.push(impl_memb_id);
                }
                AbstractStmt::MultiAssignType(multi_assign) => todo!(),
            }
        }

        for found in ident_tracker.found_dups.drain(..) {
            let preset_err = PresetErr::DuplicateIdents {
                sp_original: found.original,
                sp_dup: found.dup,
                classifier: ChrnClassifier::ConfigOption,
            };

            let builder = preset_reporter::create_diag_builder_preset(
                self.compiler,
                preset_err,
                env.region,
                self.cfg,
                self.interner,
            )
            .add_annotation(
                last_seg.span,
                AnnotationKind::Secondary,
                "Found inside this config root".to_string().into(),
            );
            self.summary.push_diag(builder.build());
        }
        // Clearing for cfg members to use for their options
        ident_tracker.clear();

        // No other scope types can hold configs. If this is reached than this is an internal error.

        let Some(found_sym_id) = sym_id_opt else {
            return;
        };

        if !typechecker::check_cfg_root(&self.compiler, found_sym_id, scope_type) {
            let classified = SymbolKind::to_fmt(&self.compiler, found_sym_id);
            let core_msg = format!("Cannot use type `{classified}` as a config root");

            let builder = SourceDiagnostic::builder(
                ErrorCode::ConfigDeclErr.into(),
                DiagnosticLevel::Error,
                core_msg,
                env.region.path_id,
            )
            .add_annotation(last_seg.span, AnnotationKind::Primary, None)
            .add_note(
                "Only user defined types and namespaces are valid config roots in `override`",
            );
            self.summary.push_diag(builder.build());
            return;
        };

        // Expected to be `ConfigDefMember`
        let mut cfg_def_members: Vec<ImplMemberId> =
            Vec::with_capacity(abs_cfg_root.cfg_members.len());

        // TEST: Tracks where this current config was positionally so that it can perform an O(1)
        // set_len call which will immediately ignore any other recursive call-site data
        let mut seen_cfg_len = 0;

        // Tracking duplicate identifiers for `AbstractConfig`
        let mut seen_cfg_idents: Vec<SpannedContainer<InternedId>> =
            Vec::with_capacity(abs_cfg_root.cfg_members.len());

        //NOTE: Maybe should be tracked from SymbolId/MemberId instead
        //
        // Tracks invalid recursive usage
        //
        // If we have Parent {p: parent} and the config is "Parent { p {} }" this is a recursive
        // error because the outer parent already defined what the Parent type should have as set
        // properties, making it a recursive definition of something that was already defined
        // let mut cfg_dfs: Vec<(TypeId, SourceSpan)> = vec![(found_type_id, sp_ty_expr.span)];

        for abs_inner_cfg in &abs_cfg_root.cfg_members {
            let AbstractConfigKind::Member(sp_memb_name_id, _) = abs_inner_cfg.kind.clone() else {
                unreachable!()
            };

            seen_cfg_idents.push(sp_memb_name_id.clone());
            seen_cfg_len += 1;

            // This member id is the member id that the member information in the specific config
            // being looked at has access to.
            //
            // This is **NOT** used beyond being assigned as the parent origin, for the `ConfigDefMember`
            // that will be created inside the recursive resolution method.
            // let scope = &self.compiler.scopes[ScopeId::new(7)].scope;

            let memb_id = match member_lookup::lookup_member(
                self.compiler,
                todo!(),
                //TODO: CHANGE THIS
                sp_memb_name_id.inner,
                MemberLookupPattern::NoRestrictions,
            ) {
                // These are split so that the theoretical ok and err paths are able to reduce
                // boilerplate where needed
                MemberLookupResult::Found(memb_id) => memb_id,
                lookup_res => {
                    // In case the lookup error points to an issue with the actual symbol we found
                    // rather than the member not existing or some non-terminal lookup error
                    //
                    // This is done because the validity of the symbol isn't checked before we
                    // actually lookup it's members
                    let mut should_break = false;

                    let src_diag = match lookup_res {
                        MemberLookupResult::ImpossibleTypeMemberAccess(type_id) => {
                            should_break = true;
                            //FIX: This has odd phrasing and pointers
                            // If we get a variable, this is matched, but the error is more so, you
                            // cannot use a variable in config, rather than the member
                            // access itself
                            let decl_span = self.compiler.get_span_from_type_id(todo!()).expect(
                                "Should have a span since it has members and was searched for",
                            );

                            //FIX:
                            let found_type_name_id = self
                                .compiler
                                .get_name_id_from_type_id(type_id)
                                .expect("NOT DONE YET");
                            let found_name = self.interner.search(found_type_name_id);

                            let preset_err = PresetErr::Lookup(
                                LookupError::ImpossibleTypeMemberAccess(SpannedContainer::new(
                                    Type::to_fmt(&self.compiler.types, type_id),
                                    decl_span,
                                )),
                            );

                            // 4th paste. 4th paste.
                            let spans: Vec<SourceSpan> =
                                sp_path_segs.iter().map(|s| s.span).collect();
                            let sp_path_span = source_span::merge_spans(&spans)
                                .expect("Path segments require at least one span");

                            preset_reporter::create_diag_builder_preset(
                                &self.compiler,
                                preset_err,
                                env.region,
                                self.cfg,
                                self.interner,
                            )
                            .add_annotation(
                                sp_path_span,
                                AnnotationKind::Secondary,
                                format!("`{found_name}` used here").into(),
                            )
                            .add_annotation(
                                sp_memb_name_id.span,
                                AnnotationKind::Secondary,
                                "member searched for".to_string().into(),
                            )
                            .add_help(format!("If this was meant to reference a `var` defined variable, prefix with \"var {found_name}\""))
                            .build()
                        }
                        MemberLookupResult::MemberNotFoundInType(type_id) => {
                            let decl_span = self
                                .compiler
                                .get_span_from_type_id(type_id)
                                .expect("Should have a span since it has members and was searched");
                            let fmtted_ty = Type::to_fmt(&self.compiler.types, type_id);

                            let found_type = &self.compiler.types[type_id];
                            //FIX:
                            let found_type_name_id = self
                                .compiler
                                .get_name_id_from_type_id(type_id)
                                .expect("NOT DONE YET");

                            let spans: Vec<SourceSpan> =
                                sp_path_segs.iter().map(|s| s.span).collect();
                            let sp_path_span = source_span::merge_spans(&spans)
                                .expect("Path segments require at least one span");

                            // Needs to be done otherwise typedefs, given "x: State" will emit the
                            // type as `x` rather than `State`
                            // let name_id =
                            //     if abs_cfg_root.lookup_pat == ScopeLookupPattern::NamespaceOnly {
                            //         abs_cfg_root.name_id
                            //     } else {
                            //         // TODO: Needs change
                            //         abs_cfg_root.name_id
                            //     };

                            let preset_err = PresetErr::Lookup(LookupError::MemberNotFound {
                                searched_type_id: type_id,
                                sp_searched_type_name_id: SpannedContainer::new(
                                    found_type_name_id,
                                    sp_path_span,
                                ),
                                not_found_name_id: sp_memb_name_id.inner,
                            });

                            preset_reporter::create_diag_builder_preset(
                                &self.compiler,
                                preset_err,
                                env.region,
                                self.cfg,
                                self.interner,
                            )
                            .add_annotation(
                                decl_span,
                                AnnotationKind::Secondary,
                                format!("{} defined here", fmtted_ty).into(),
                            )
                            .add_annotation(
                                sp_memb_name_id.span,
                                AnnotationKind::Secondary,
                                "Searched for this member".to_string().into(),
                            )
                            .build()
                        }
                        // TODO: This is reached and should probably result in continue since if
                        // it's unknown that means a previous stage reported it more likely than
                        // not. (Its 100%)
                        // Um. When is this case met?
                        MemberLookupResult::Unknown(type_id) => {
                            // let var = self.compiler.get_var(found_sym_id);
                            // let name = self.interner.search(var.name_id);

                            // dbg!(&self.compiler.types[var.type_id ]);
                            todo!("RUST_BACKTRACE=1");
                        }
                        MemberLookupResult::Found(_) => unreachable!(),
                    };

                    self.summary.push_diag(src_diag);

                    if should_break {
                        break;
                    }

                    continue;
                }
            };

            let override_ctx = ConfigMemberComplexContext::new(todo!());
            let ctx = ConfigMemberContextKind::Complex(override_ctx);

            let cfg_member_id = self.resolve_cfg_member(
                parent_impl_id,
                // The type expr is derivative of path segments which may or may not be a valid type
                // expr hence this is using last segment
                last_seg.span,
                &ctx,
                // sp_path_segs,
                // &mut cfg_dfs,
                &mut seen_cfg_idents,
                // NOTE: Opt ident tracker
                ident_tracker,
                memb_id,
                abs_inner_cfg,
                scope_type,
                1,
                env,
            );

            cfg_def_members.push(cfg_member_id);
            seen_cfg_idents.truncate(seen_cfg_len);
        }

        //NOTE: Maybe it's worth using function-specific trait-bounded concepts to where it CAN
        // generically examine it's defined name id where it simply reports back the duplicate found
        // and the caller still reports it since spans and messages vary.
        for (i, current_cfg) in seen_cfg_idents.iter().enumerate() {
            // Since this root cfg's made `seen_cfg_vec` it does not need any deeper checks
            if let Some((_, original_cfg)) = seen_cfg_idents
                .iter()
                .enumerate()
                // If the other index was declared after the current index and they have the same identifier
                //
                // Since this iteration specifically checks if the current was declared after the
                // last and the iteration terminates upon the first match, this correctly points at
                // the original field for all duplicates.
                .find(|(other_i, cfg)| *other_i < i && current_cfg.inner == cfg.inner)
            {
                let dup_name = self.interner.search(current_cfg.inner);

                let orig_span = original_cfg.span;
                let current_cfg_span = current_cfg.span;

                let core_msg = format!("More than one config member has identifier `{dup_name}`");

                let spans: Vec<SourceSpan> = sp_path_segs.iter().map(|s| s.span).collect();
                let sp_path_span = source_span::merge_spans(&spans)
                    .expect("Path segments require at least one span");

                // Maybe give `None` here..
                let src_diag = SourceDiagnostic::builder(
                    ErrorCode::ConfigDeclErr.into(),
                    DiagnosticLevel::Error,
                    core_msg,
                    env.region.path_id,
                )
                .add_annotation(
                    sp_path_span,
                    AnnotationKind::Secondary,
                    "Found inside this config root".to_string().into(),
                )
                .add_annotation(
                    orig_span,
                    AnnotationKind::Secondary,
                    format!("Original usage of `{dup_name}` here").into(),
                )
                .add_annotation(current_cfg_span, AnnotationKind::Primary, None)
                .build();

                self.summary.push_diag(src_diag);
            }
        }
        todo!("Hi members(s)");

        let cfg_root = self.compiler.get_cfg_root_mut(parent_impl_id);

        debug_assert!(matches!(cfg_root.linked_sym_id, None));
        debug_assert_eq!(cfg_root.memb_stmts.len(), 0);
        debug_assert_eq!(cfg_root.common.cfg_members.len(), 0);
        debug_assert!(matches!(
            cfg_root.common.lookup_pat,
            ScopeLookupPattern::NamespaceOnly
                | ScopeLookupPattern::OnlyVar
                | ScopeLookupPattern::OnlyNest
                | ScopeLookupPattern::OnlyIntrinsic
        ));

        cfg_root.linked_sym_id = Some(found_sym_id);
        cfg_root.memb_stmts = memb_stmts;
        cfg_root.common.cfg_members = cfg_def_members;
        todo!()
    }

    //TODO: For resolve config member, pass in kind, which routes to declared methods. Call method
    //inside initial, then inside the router after getting root which is required.

    /// Routing
    fn resolve_cfg_root_complex<'env>(
        &mut self,
        complex_impl_id: ImplId,
        abs_cfg_root: &'env AbstractConfig,
        sp_path_segs: &[SpannedContainer<PathSegment>],
        opt_ident_tracker: &mut DuplicateTracker<SpannedContainer<InternedId>>,
        last_seg: &SpannedContainer<PathSegment>,
        last_scope: AssociatedScopeKind,
        lookup_pat: ScopeLookupPattern,
        env: &'env ResolverEnv,
    ) {
        //Make this a helper? Might be going too far making a type expr specific static
        //access helper
        // let type_id_res = match &last_seg.inner {
        //     PathSegment::Ident(interned_id) => {
        //         let inline_ty_expr =
        //             SpannedContainer::new(TypeExpr::Var(*interned_id), last_seg.span);
        //         resolution_helpers::resolve_type_expr_ret_preset(
        //             self.compiler,
        //             last_scope,
        //             &inline_ty_expr,
        //             scope_type,
        //             lookup_pat,
        //             self.interner,
        //             env,
        //         )
        //     }
        //     PathSegment::Generic(generic) => resolution_helpers::resolve_generic_ret_preset(
        //         self.compiler,
        //         generic,
        //         last_scope,
        //         last_seg.span,
        //         scope_type,
        //         self.interner,
        //         env,
        //     ),
        // };
        //
        // let type_id_opt = match type_id_res {
        //     Ok(type_id) => type_id.into(),
        //     Err(preset_err) => {
        //         preset_reporter::report_preset(
        //             self.compiler,
        //             &mut self.summary,
        //             preset_err,
        //             env.region,
        //             self.cfg,
        //             self.interner,
        //         );
        //         None
        //     }
        // };

        // Is `Option` so the option exprs can be validated before exiting since those don't care
        // about whether or not the symbol or type id valid
        //
        // Is mutable so that if the typecheck fails, it can be set to `None`, which then allows for
        // the same return signal to be used on failure.

        // NOTE: Parser does not allow complex to consume anything but option assignments
        // let mut memb_stmts: Vec<ImplMemberId> = Vec::with_capacity(abs_cfg_root.abs_stmts.len());
        //
        // for abs_stmt in &abs_cfg_root.abs_stmts {
        //     let AbstractStmt::OptAssignment(opt) = abs_stmt else {
        //         unreachable!();
        //     };
        //     let sp_name_id = SpannedContainer::new(opt.name_id, opt.name_span);
        //     opt_ident_tracker.insert_or_store(sp_name_id);
        //
        //     let expr_id = match self.register_expr(
        //         None,
        //         &opt.array_expr,
        //         None,
        //         last_scope,
        //         scope_type,
        //         env,
        //     ) {
        //         Ok(expr_id) => expr_id,
        //         Err(preset_err) => {
        //             preset_reporter::report_preset(
        //                 &self.compiler,
        //                 &mut self.summary,
        //                 preset_err,
        //                 env.region,
        //                 self.cfg,
        //                 self.interner,
        //             );
        //             continue;
        //         }
        //     };
        //
        //     let impl_member_id = ImplMemberId::new(self.compiler.impl_members.len() as u32);
        //     let opt = OptionAssignmentRoot::new(
        //         complex_impl_id,
        //         impl_member_id,
        //         opt.name_id,
        //         opt.name_span,
        //         expr_id,
        //     );
        //
        //     self.compiler
        //         .impl_members
        //         .push(ImplMemberKind::OptAssignmentRoot(opt));
        //     memb_stmts.push(impl_member_id);
        // }
        //
        // for found in opt_ident_tracker.found_dups.drain(..) {
        //     let preset_err = PresetErr::DuplicateIdents {
        //         sp_original: found.original,
        //         sp_dup: found.dup,
        //         classifier: ChrnClassifier::ConfigOption,
        //     };
        //
        //     let builder = preset_reporter::create_diag_builder_preset(
        //         self.compiler,
        //         preset_err,
        //         env.region,
        //         self.cfg,
        //         self.interner,
        //     )
        //     .add_annotation(
        //         last_seg.span,
        //         AnnotationKind::Secondary,
        //         "Found inside this config root".to_string().into(),
        //     );
        //     self.summary.push_diag(builder.build());
        // }
        // // Clearing for cfg members to use for their options
        // opt_ident_tracker.clear();
        //
        // let Some(found_type_id) = type_id_opt else {
        //     return;
        // };
        //
        // // Suspicious workaround required but ok for now
        // let sym_id = self
        //     .compiler
        //     .get_sym_id_from_type_id(found_type_id)
        //     .expect("Earlier in-method match enforces the sym id exists");
        //
        // if !typechecker::check_cfg_root(&self.compiler, sym_id, scope_type) {
        //     let fmtted_ty = Type::to_fmt(&self.compiler.types, found_type_id);
        //     let core_msg = format!("Cannot use type `{fmtted_ty}` as a config root");
        //
        //     let spans: Vec<SourceSpan> = sp_path_segs.iter().map(|s| s.span).collect();
        //     let sp_path_span =
        //         source_span::merge_spans(&spans).expect("Path segments require at least one span");
        //
        //     let builder = SourceDiagnostic::builder(
        //         ErrorCode::ConfigDeclErr.into(),
        //         DiagnosticLevel::Error,
        //         core_msg,
        //         env.region.path_id,
        //     )
        //     .add_annotation(sp_path_span, AnnotationKind::Primary, None)
        //     .add_note("Only user defined types are valid config roots in `complex`");
        //     self.summary.push_diag(builder.build());
        //
        //     // Failed so it should be marked as not found to follow suit in return
        //     return;
        // };
        //
        // // Expected to be `ConfigDefMember`
        // let mut cfg_def_members: Vec<ImplMemberId> =
        //     Vec::with_capacity(abs_cfg_root.cfg_members.len());
        //
        // // TEST: Tracks where this current config was positionally so that it can perform an O(1)
        // // set_len call which will immediately ignore any other recursive call-site data
        // let mut seen_cfg_len = 0;
        //
        // // Tracking duplicate identifiers for `AbstractConfig`
        // let mut seen_cfg_idents: Vec<SpannedContainer<InternedId>> =
        //     Vec::with_capacity(abs_cfg_root.cfg_members.len());
        //
        // //NOTE: Maybe should be tracked from SymbolId/MemberId instead
        // //
        // // Tracks invalid recursive usage
        // //
        // // If we have Parent {p: parent} and the config is "Parent { p {} }" this is a recursive
        // // error because the outer parent already defined what the Parent type should have as set
        // // properties, making it a recursive definition of something that was already defined
        // // let mut cfg_dfs: Vec<(TypeId, SourceSpan)> = vec![(found_type_id, sp_ty_expr.span)];
        //
        // for abs_inner_cfg in &abs_cfg_root.cfg_members {
        //     let AbstractConfigKind::Member(sp_memb_name_id, _) = abs_inner_cfg.kind.clone() else {
        //         unreachable!()
        //     };
        //
        //     seen_cfg_idents.push(sp_memb_name_id.clone());
        //     seen_cfg_len += 1;
        //
        //     // This member id is the member id that the member information in the specific config
        //     // being looked at has access to.
        //     //
        //     // This is **NOT** used beyond being assigned as the parent origin, for the `ConfigDefMember`
        //     // that will be created inside the recursive resolution method.
        //     // let scope = &self.compiler.scopes[ScopeId::new(7)].scope;
        //
        //     let memb_id = match member_lookup::lookup_member(
        //         self.compiler,
        //         found_type_id,
        //         //TODO: CHANGE THIS
        //         sp_memb_name_id.inner,
        //         MemberLookupPattern::NoRestrictions,
        //     ) {
        //         // These are split so that the theoretical ok and err paths are able to reduce
        //         // boilerplate where needed
        //         MemberLookupResult::Found(memb_id) => memb_id,
        //         lookup_res => {
        //             // In case the lookup error points to an issue with the actual symbol we found
        //             // rather than the member not existing or some non-terminal lookup error
        //             //
        //             // This is done because the validity of the symbol isn't checked before we
        //             // actually lookup it's members
        //             let mut should_break = false;
        //
        //             let src_diag = match lookup_res {
        //                 MemberLookupResult::ImpossibleTypeMemberAccess(type_id) => {
        //                     should_break = true;
        //                     //FIX: This has odd phrasing and pointers
        //                     // If we get a variable, this is matched, but the error is more so, you
        //                     // cannot use a variable in config, rather than the member
        //                     // access itself
        //                     let decl_span = self.compiler.get_span_from_type_id(todo!()).expect(
        //                         "Should have a span since it has members and was searched for",
        //                     );
        //
        //                     //FIX:
        //                     let found_type_name_id = self
        //                         .compiler
        //                         .get_name_id_from_type_id(type_id)
        //                         .expect("NOT DONE YET");
        //                     let found_name = self.interner.search(found_type_name_id);
        //
        //                     let preset_err = PresetErr::Lookup(
        //                         LookupError::ImpossibleTypeMemberAccess(SpannedContainer::new(
        //                             Type::to_fmt(&self.compiler.types, type_id),
        //                             decl_span,
        //                         )),
        //                     );
        //
        //                     // 4th paste. 4th paste.
        //                     let spans: Vec<SourceSpan> =
        //                         sp_path_segs.iter().map(|s| s.span).collect();
        //                     let sp_path_span = source_span::merge_spans(&spans)
        //                         .expect("Path segments require at least one span");
        //
        //                     preset_reporter::create_diag_builder_preset(
        //                         &self.compiler,
        //                         preset_err,
        //                         env.region,
        //                         self.cfg,
        //                         self.interner,
        //                     )
        //                     .add_annotation(
        //                         sp_path_span,
        //                         AnnotationKind::Secondary,
        //                         format!("`{found_name}` used here").into(),
        //                     )
        //                     .add_annotation(
        //                         sp_memb_name_id.span,
        //                         AnnotationKind::Secondary,
        //                         "member searched for".to_string().into(),
        //                     )
        //                     .add_help(format!("If this was meant to reference a `var` defined variable, prefix with \"var {found_name}\""))
        //                     .build()
        //                 }
        //                 MemberLookupResult::MemberNotFoundInType(type_id) => {
        //                     let decl_span = self
        //                         .compiler
        //                         .get_span_from_type_id(type_id)
        //                         .expect("Should have a span since it has members and was searched");
        //                     let fmtted_ty = Type::to_fmt(&self.compiler.types, type_id);
        //
        //                     let found_type = &self.compiler.types[type_id];
        //                     //FIX:
        //                     let found_type_name_id = self
        //                         .compiler
        //                         .get_name_id_from_type_id(type_id)
        //                         .expect("NOT DONE YET");
        //
        //                     let spans: Vec<SourceSpan> =
        //                         sp_path_segs.iter().map(|s| s.span).collect();
        //                     let sp_path_span = source_span::merge_spans(&spans)
        //                         .expect("Path segments require at least one span");
        //
        //                     // Needs to be done otherwise typedefs, given "x: State" will emit the
        //                     // type as `x` rather than `State`
        //                     // let name_id =
        //                     //     if abs_cfg_root.lookup_pat == ScopeLookupPattern::NamespaceOnly {
        //                     //         abs_cfg_root.name_id
        //                     //     } else {
        //                     //         // TODO: Needs change
        //                     //         abs_cfg_root.name_id
        //                     //     };
        //
        //                     let preset_err = PresetErr::Lookup(LookupError::MemberNotFound {
        //                         searched_type_id: type_id,
        //                         sp_searched_type_name_id: SpannedContainer::new(
        //                             found_type_name_id,
        //                             sp_path_span,
        //                         ),
        //                         not_found_name_id: sp_memb_name_id.inner,
        //                     });
        //
        //                     preset_reporter::create_diag_builder_preset(
        //                         &self.compiler,
        //                         preset_err,
        //                         env.region,
        //                         self.cfg,
        //                         self.interner,
        //                     )
        //                     .add_annotation(
        //                         decl_span,
        //                         AnnotationKind::Secondary,
        //                         format!("{} defined here", fmtted_ty).into(),
        //                     )
        //                     .add_annotation(
        //                         sp_memb_name_id.span,
        //                         AnnotationKind::Secondary,
        //                         "Searched for this member".to_string().into(),
        //                     )
        //                     .build()
        //                 }
        //                 // TODO: This is reached and should probably result in continue since if
        //                 // it's unknown that means a previous stage reported it more likely than
        //                 // not. (Its 100%)
        //                 // Um. When is this case met?
        //                 MemberLookupResult::Unknown(type_id) => {
        //                     // let var = self.compiler.get_var(found_sym_id);
        //                     // let name = self.interner.search(var.name_id);
        //
        //                     // dbg!(&self.compiler.types[var.type_id ]);
        //                     todo!("RUST_BACKTRACE=1");
        //                 }
        //                 MemberLookupResult::Found(_) => unreachable!(),
        //             };
        //
        //             self.summary.push_diag(src_diag);
        //
        //             if should_break {
        //                 break;
        //             }
        //
        //             continue;
        //         }
        //     };
        //
        //     let override_ctx = ConfigMemberComplexContext::new(found_type_id);
        //     let ctx = ConfigMemberContextKind::Complex(override_ctx);
        //
        //     let cfg_member_id = self.resolve_cfg_member(
        //         complex_impl_id,
        //         // The type expr is derivative of path segments which may or may not be a valid type
        //         // expr hence this is using last segment
        //         last_seg.span,
        //         &ctx,
        //         // sp_path_segs,
        //         // &mut cfg_dfs,
        //         &mut seen_cfg_idents,
        //         // NOTE: Opt ident tracker
        //         opt_ident_tracker,
        //         memb_id,
        //         abs_inner_cfg,
        //         scope_type,
        //         1,
        //         env,
        //     );
        //
        //     cfg_def_members.push(cfg_member_id);
        //     seen_cfg_idents.truncate(seen_cfg_len);
        // }
        //
        // //NOTE: Maybe it's worth using function-specific trait-bounded concepts to where it CAN
        // // generically examine it's defined name id where it simply reports back the duplicate found
        // // and the caller still reports it since spans and messages vary.
        // for (i, current_cfg) in seen_cfg_idents.iter().enumerate() {
        //     // Since this root cfg's made `seen_cfg_vec` it does not need any deeper checks
        //     if let Some((_, original_cfg)) = seen_cfg_idents
        //         .iter()
        //         .enumerate()
        //         // If the other index was declared after the current index and they have the same identifier
        //         //
        //         // Since this iteration specifically checks if the current was declared after the
        //         // last and the iteration terminates upon the first match, this correctly points at
        //         // the original field for all duplicates.
        //         .find(|(other_i, cfg)| *other_i < i && current_cfg.inner == cfg.inner)
        //     {
        //         let dup_name = self.interner.search(current_cfg.inner);
        //
        //         let orig_span = original_cfg.span;
        //         let current_cfg_span = current_cfg.span;
        //
        //         let core_msg = format!("More than one config member has identifier `{dup_name}`");
        //
        //         let spans: Vec<SourceSpan> = sp_path_segs.iter().map(|s| s.span).collect();
        //         let sp_path_span = source_span::merge_spans(&spans)
        //             .expect("Path segments require at least one span");
        //
        //         // Maybe give `None` here..
        //         let src_diag = SourceDiagnostic::builder(
        //             ErrorCode::ConfigDeclErr.into(),
        //             DiagnosticLevel::Error,
        //             core_msg,
        //             env.region.path_id,
        //         )
        //         .add_annotation(
        //             sp_path_span,
        //             AnnotationKind::Secondary,
        //             "Found inside this config root".to_string().into(),
        //         )
        //         .add_annotation(
        //             orig_span,
        //             AnnotationKind::Secondary,
        //             format!("Original usage of `{dup_name}` here").into(),
        //         )
        //         .add_annotation(current_cfg_span, AnnotationKind::Primary, None)
        //         .build();
        //
        //         self.summary.push_diag(src_diag);
        //     }
        // }
        // todo!("Hi members(s)");
        //
        // let cfg_complex = self.compiler.get_cfg_root_mut(complex_impl_id);
        //
        // debug_assert!(matches!(cfg_complex.linked_type_id, None));
        // debug_assert_eq!(cfg_complex.impl_stmts.len(), 0);
        // debug_assert_eq!(cfg_complex.common.cfg_members.len(), 0);
        // debug_assert!(matches!(
        //     cfg_complex.common.lookup_pat,
        //     ScopeLookupPattern::NamespaceOnly
        //         | ScopeLookupPattern::OnlyVar
        //         | ScopeLookupPattern::OnlyNest
        // ));
        //
        // cfg_complex.linked_type_id = Some(found_type_id);
        // cfg_complex.impl_stmts = memb_stmts;
        // cfg_complex.common.cfg_members = cfg_def_members;
        todo!()
    }

    // /// Routing
    // fn resolve_cfg_root_override<'env>(
    //     &mut self,
    //     override_impl_id: ImplId,
    //     abs_cfg_root: &'env AbstractConfig,
    //     sp_path_segs: &[SpannedContainer<PathSegment>],
    //     opt_ident_tracker: &mut DuplicateTracker<SpannedContainer<InternedId>>,
    //     last_seg: &SpannedContainer<PathSegment>,
    //     last_scope: AssociatedScopeKind,
    //     lookup_pat: ScopeLookupPattern,
    //     lookup_pref: ScopeLookupPreferenceFlags,
    //     env: &'env ResolverEnv,
    // ) {
    //     let scope_type = ScopeType::Override;
    //
    //     // Check if it's a namespace or type (that's contextually valid)
    //     // Could be generic so can't go by scopes::find_sym_id
    //     let sym_id_opt = match &last_seg.inner {
    //         PathSegment::Ident(interned_id) => {
    //             //TODO: This preset err for this maybe
    //             // Java uppser is being searched here, maybe because the previous is java
    //             // upper itself, and it's basically asking, is java upper in java upper,
    //             // given that we are inside java upper?
    //             match scopes::find_sym_id(
    //                 self.compiler,
    //                 last_scope,
    //                 *interned_id,
    //                 scope_type,
    //                 lookup_pat,
    //                 lookup_pref,
    //             ) {
    //                 Some(SymbolLookupOutput { found_sym_id, .. }) => found_sym_id.into(),
    //                 None => {
    //                     let lookup_err = LookupError::SymbolNotFound {
    //                         sp_invalid_name_id: SpannedContainer::new(*interned_id, last_seg.span),
    //                         scope_searched: last_scope,
    //                     };
    //                     preset_reporter::report_preset(
    //                         self.compiler,
    //                         &mut self.summary,
    //                         lookup_err.into(),
    //                         env.region,
    //                         self.cfg,
    //                         self.interner,
    //                     );
    //                     None
    //                 }
    //             }
    //         }
    //         PathSegment::Generic(_) => {
    //             let builder = SourceDiagnostic::builder(
    //                 ErrorCode::GenericsErr.into(),
    //                 DiagnosticLevel::Error,
    //                 "Config root must be a user defined type",
    //                 env.region.path_id,
    //             )
    //             .add_annotation(last_seg.span, AnnotationKind::Primary, None);
    //             self.summary.push_diag(builder.build());
    //             None
    //         }
    //     };
    //
    //     // Is `Option` so the option exprs can be validated before exiting since those don't care
    //     // about whether or not the symbol or type id valid
    //     //
    //     // Is mutable so that if the typecheck fails, it can be set to `None`, which then allows for
    //     // the same return signal to be used on failure.
    //
    //     let mut impl_memb_stmts: Vec<ImplMemberId> =
    //         Vec::with_capacity(abs_cfg_root.abs_stmts.len());
    //
    //     for abs_stmt in &abs_cfg_root.abs_stmts {
    //         //TODO: Um, method!
    //         match abs_stmt {
    //             AbstractStmt::OptAssignment(opt) => {
    //                 let sp_name_id = SpannedContainer::new(opt.name_id, opt.name_span);
    //                 opt_ident_tracker.insert_or_store(sp_name_id);
    //
    //                 let expr_id = match self.register_expr(
    //                     None,
    //                     &opt.array_expr,
    //                     None,
    //                     last_scope,
    //                     scope_type,
    //                     env,
    //                 ) {
    //                     Ok(expr_id) => expr_id,
    //                     Err(preset_err) => {
    //                         preset_reporter::report_preset(
    //                             &self.compiler,
    //                             &mut self.summary,
    //                             preset_err,
    //                             env.region,
    //                             self.cfg,
    //                             self.interner,
    //                         );
    //                         continue;
    //                     }
    //                 };
    //
    //                 let impl_memb_id = ImplMemberId::new(self.compiler.impl_members.len() as u32);
    //                 let opt = OptionAssignmentRoot::new(
    //                     override_impl_id,
    //                     impl_memb_id,
    //                     opt.name_id,
    //                     opt.name_span,
    //                     expr_id,
    //                 );
    //
    //                 self.compiler
    //                     .impl_members
    //                     .push(ImplMemberKind::OptAssignmentRoot(opt));
    //                 impl_memb_stmts.push(impl_memb_id);
    //             }
    //             AbstractStmt::MultiAssignType(multi_assign) => todo!(),
    //         }
    //     }
    //
    //     for found in opt_ident_tracker.found_dups.drain(..) {
    //         let preset_err = PresetErr::DuplicateIdents {
    //             sp_original: found.original,
    //             sp_dup: found.dup,
    //             classifier: ChrnClassifier::ConfigOption,
    //         };
    //
    //         let builder = preset_reporter::create_diag_builder_preset(
    //             self.compiler,
    //             preset_err,
    //             env.region,
    //             self.cfg,
    //             self.interner,
    //         )
    //         .add_annotation(
    //             last_seg.span,
    //             AnnotationKind::Secondary,
    //             "Found inside this config root".to_string().into(),
    //         );
    //         self.summary.push_diag(builder.build());
    //     }
    //     // Clearing for cfg members to use for their options
    //     opt_ident_tracker.clear();
    //
    //     // No other scope types can hold configs. If this is reached than this is an internal error.
    //
    //     let Some(found_sym_id) = sym_id_opt else {
    //         return;
    //     };
    //
    //     if !typechecker::check_cfg_root(&self.compiler, found_sym_id, scope_type) {
    //         let fmtted_sym = SymbolKind::to_fmt(&self.compiler, found_sym_id);
    //         let core_msg = format!("Cannot use type `{fmtted_sym}` as a config root");
    //
    //         let builder = SourceDiagnostic::builder(
    //             ErrorCode::ConfigDeclErr.into(),
    //             DiagnosticLevel::Error,
    //             core_msg,
    //             env.region.path_id,
    //         )
    //         .add_annotation(last_seg.span, AnnotationKind::Primary, None)
    //         .add_note(
    //             "Only user defined types and namespaces are valid config roots in `override`",
    //         );
    //         self.summary.push_diag(builder.build());
    //         return;
    //     };
    //
    //     // Expected to be `ConfigDefMember`
    //     let mut cfg_def_members: Vec<ImplMemberId> =
    //         Vec::with_capacity(abs_cfg_root.cfg_members.len());
    //
    //     // TEST: Tracks where this current config was positionally so that it can perform an O(1)
    //     // set_len call which will immediately ignore any other recursive call-site data
    //     let mut seen_cfg_len = 0;
    //
    //     // Tracking duplicate identifiers for `AbstractConfig`
    //     let mut seen_cfg_idents: Vec<SpannedContainer<InternedId>> =
    //         Vec::with_capacity(abs_cfg_root.cfg_members.len());
    //
    //     //NOTE: Maybe should be tracked from SymbolId/MemberId instead
    //     //
    //     // Tracks invalid recursive usage
    //     //
    //     // If we have Parent {p: parent} and the config is "Parent { p {} }" this is a recursive
    //     // error because the outer parent already defined what the Parent type should have as set
    //     // properties, making it a recursive definition of something that was already defined
    //     // let mut cfg_dfs: Vec<(TypeId, SourceSpan)> = vec![(found_type_id, sp_ty_expr.span)];
    //
    //     for abs_inner_cfg in &abs_cfg_root.cfg_members {
    //         let AbstractConfigKind::Member(sp_memb_name_id, _) = abs_inner_cfg.kind.clone() else {
    //             unreachable!()
    //         };
    //
    //         seen_cfg_idents.push(sp_memb_name_id.clone());
    //         seen_cfg_len += 1;
    //
    //         // This member id is the member id that the member information in the specific config
    //         // being looked at has access to.
    //         //
    //         // This is **NOT** used beyond being assigned as the parent origin, for the `ConfigDefMember`
    //         // that will be created inside the recursive resolution method.
    //         // let scope = &self.compiler.scopes[ScopeId::new(7)].scope;
    //         todo!();
    //         let member_id = match member_lookup::lookup_member(
    //             self.compiler,
    //             todo!(),
    //             // found_type_id,
    //             //TODO: CHANGE THIS
    //             sp_memb_name_id.inner,
    //             MemberLookupPattern::NoRestrictions,
    //         ) {
    //             // These are split so that the theoretical ok and err paths are able to reduce
    //             // boilerplate where needed
    //             MemberLookupResult::Found(memb_id) => memb_id,
    //             lookup_res => {
    //                 // In case the lookup error points to an issue with the actual symbol we found
    //                 // rather than the member not existing or some non-terminal lookup error
    //                 //
    //                 // This is done because the validity of the symbol isn't checked before we
    //                 // actually lookup it's members
    //                 let mut should_break = false;
    //
    //                 let src_diag = match lookup_res {
    //                     MemberLookupResult::ImpossibleTypeMemberAccess(type_id) => {
    //                         should_break = true;
    //                         //FIX: This has odd phrasing and pointers
    //                         // If we get a variable, this is matched, but the error is more so, you
    //                         // cannot use a variable in config, rather than the member
    //                         // access itself
    //                         let decl_span = self.compiler.get_span_from_type_id(todo!()).expect(
    //                             "Should have a span since it has members and was searched for",
    //                         );
    //
    //                         //FIX:
    //                         let found_type_name_id = self
    //                             .compiler
    //                             .get_name_id_from_type_id(type_id)
    //                             .expect("NOT DONE YET");
    //                         let found_name = self.interner.search(found_type_name_id);
    //
    //                         let preset_err = PresetErr::Lookup(
    //                             LookupError::ImpossibleTypeMemberAccess(SpannedContainer::new(
    //                                 Type::to_fmt(&self.compiler.types, type_id),
    //                                 decl_span,
    //                             )),
    //                         );
    //
    //                         // 4th paste. 4th paste.
    //                         let spans: Vec<SourceSpan> =
    //                             sp_path_segs.iter().map(|s| s.span).collect();
    //                         let sp_path_span = source_span::merge_spans(&spans)
    //                             .expect("Path segments require at least one span");
    //
    //                         preset_reporter::create_diag_builder_preset(
    //                             &self.compiler,
    //                             preset_err,
    //                             env.region,
    //                             self.cfg,
    //                             self.interner,
    //                         )
    //                         .add_annotation(
    //                             sp_path_span,
    //                             AnnotationKind::Secondary,
    //                             format!("`{found_name}` used here").into(),
    //                         )
    //                         .add_annotation(
    //                             sp_memb_name_id.span,
    //                             AnnotationKind::Secondary,
    //                             "member searched for".to_string().into(),
    //                         )
    //                         .add_help(format!("If this was meant to reference a `var` defined variable, prefix with \"var {found_name}\""))
    //                         .build()
    //                     }
    //                     MemberLookupResult::MemberNotFoundInType(type_id) => {
    //                         let decl_span = self
    //                             .compiler
    //                             .get_span_from_type_id(todo!())
    //                             .expect("Should have a span since it has members and was searched");
    //                         let fmtted_ty = Type::to_fmt(&self.compiler.types, type_id);
    //
    //                         let found_type = &self.compiler.types[todo!()];
    //                         //FIX:
    //                         let found_type_name_id = self
    //                             .compiler
    //                             .get_name_id_from_type_id(type_id)
    //                             .expect("NOT DONE YET");
    //
    //                         let spans: Vec<SourceSpan> =
    //                             sp_path_segs.iter().map(|s| s.span).collect();
    //                         let sp_path_span = source_span::merge_spans(&spans)
    //                             .expect("Path segments require at least one span");
    //
    //                         // Needs to be done otherwise typedefs, given "x: State" will emit the
    //                         // type as `x` rather than `State`
    //                         // let name_id =
    //                         //     if abs_cfg_root.lookup_pat == ScopeLookupPattern::NamespaceOnly {
    //                         //         abs_cfg_root.name_id
    //                         //     } else {
    //                         //         // TODO: Needs change
    //                         //         abs_cfg_root.name_id
    //                         //     };
    //
    //                         let preset_err = PresetErr::Lookup(LookupError::MemberNotFound {
    //                             searched_type_id: type_id,
    //                             sp_searched_type_name_id: SpannedContainer::new(
    //                                 found_type_name_id,
    //                                 sp_path_span,
    //                             ),
    //                             not_found_name_id: sp_memb_name_id.inner,
    //                         });
    //
    //                         preset_reporter::create_diag_builder_preset(
    //                             &self.compiler,
    //                             preset_err,
    //                             env.region,
    //                             self.cfg,
    //                             self.interner,
    //                         )
    //                         .add_annotation(
    //                             decl_span,
    //                             AnnotationKind::Secondary,
    //                             format!("{} defined here", fmtted_ty).into(),
    //                         )
    //                         .add_annotation(
    //                             sp_memb_name_id.span,
    //                             AnnotationKind::Secondary,
    //                             "Searched for this member".to_string().into(),
    //                         )
    //                         .build()
    //                     }
    //                     // TODO: This is reached and should probably result in continue since if
    //                     // it's unknown that means a previous stage reported it more likely than
    //                     // not. (Its 100%)
    //                     // Um. When is this case met?
    //                     MemberLookupResult::Unknown(type_id) => {
    //                         // let var = self.compiler.get_var(found_sym_id);
    //                         // let name = self.interner.search(var.name_id);
    //
    //                         // dbg!(&self.compiler.types[var.type_id ]);
    //                         todo!("RUST_BACKTRACE=1");
    //                     }
    //                     MemberLookupResult::Found(_) => unreachable!(),
    //                 };
    //
    //                 self.summary.push_diag(src_diag);
    //
    //                 if should_break {
    //                     break;
    //                 }
    //
    //                 continue;
    //             }
    //         };
    //
    //         let cfg_member_id = self.resolve_cfg_member(
    //             override_impl_id,
    //             last_seg.span,
    //             todo!(),
    //             // sp_path_segs,
    //             // &mut cfg_dfs,
    //             &mut seen_cfg_idents,
    //             opt_ident_tracker,
    //             member_id,
    //             abs_inner_cfg,
    //             scope_type,
    //             1,
    //             env,
    //         );
    //
    //         cfg_def_members.push(cfg_member_id);
    //         seen_cfg_idents.truncate(seen_cfg_len);
    //     }
    //
    //     if let DuplicateIdentResult::Duplicate {
    //         sp_original,
    //         sp_dup,
    //     } = checker_helpers::check_duplicate_ident(&seen_cfg_idents)
    //     {
    //         let preset_err = PresetErr::DuplicateIdents {
    //             sp_original,
    //             sp_dup,
    //             classifier: ChrnClassifier::ConfigRoot,
    //         };
    //
    //         let spans: Vec<SourceSpan> = sp_path_segs.iter().map(|s| s.span).collect();
    //         let sp_path_span =
    //             source_span::merge_spans(&spans).expect("Path segments require at least one span");
    //
    //         let builder = preset_reporter::create_diag_builder_preset(
    //             self.compiler,
    //             preset_err,
    //             env.region,
    //             self.cfg,
    //             self.interner,
    //         )
    //         .add_annotation(
    //             sp_path_span,
    //             AnnotationKind::Secondary,
    //             "Found inside this config root".to_string().into(),
    //         );
    //         self.summary.push_diag(builder.build());
    //     };
    //
    //     todo!("Hi members");
    //
    //     let cfg_override = self.compiler.get_cfg_root_override_mut(override_impl_id);
    //
    //     debug_assert!(matches!(cfg_override.linked_sym_id, None));
    //     debug_assert_eq!(cfg_override.impl_memb_stmts.len(), 0);
    //     debug_assert_eq!(cfg_override.common.cfg_members.len(), 0);
    //     debug_assert!(matches!(
    //         cfg_override.common.lookup_pat,
    //         ScopeLookupPattern::NamespaceOnly
    //             | ScopeLookupPattern::OnlyVar
    //             | ScopeLookupPattern::OnlyNest
    //     ));
    //
    //     cfg_override.linked_sym_id = Some(found_sym_id);
    //     cfg_override.impl_memb_stmts = impl_memb_stmts;
    //     cfg_override.common.cfg_members = cfg_def_members;
    // }

    // Complex can only take types so it's identifier MUST match to a symbol which must be a type, which
    // must have members.
    //
    // Override should have the same type logic, with the namespace being possible as well.
    // The root maybe dictates the allowance because in override if we start with a namespace, we have
    // no members. But if the root is a type, the namespace parts must just route the already present
    // type info, only switching the context being applied.
    //
    // This is actionable but is this the right abstraction? This is inserting an entirely different
    // concept inside structures, which could be "types{}" or any other added intrinsic, which could
    // stack and get very confusing. The scope traversal also may be semantically a bit confusing
    // since it's types {} possibly inside the struct parent, inside the members, possible other
    // override members, maybe we just remove the types {} namespace? It's here to make sure any
    // future updates can just add a namespace to override, but
    fn lookup_cfg_memb(
        &mut self,
        sp_path_segs: &[SpannedContainer<PathSegment>],
        scope_type: ScopeType,
    ) -> ConfigMemberOutput {
        // let sym_id_opt = match &last_seg.inner {
        //     PathSegment::Ident(interned_id) => {
        //         //TODO: This preset err for this maybe
        //         // Java uppser is being searched here, maybe because the previous is java
        //         // upper itself, and it's basically asking, is java upper in java upper,
        //         // given that we are inside java upper?
        //         match scopes::find_sym_id(
        //             self.compiler,
        //             last_scope,
        //             *interned_id,
        //             scope_type,
        //             lookup_pat,
        //             lookup_pref,
        //         ) {
        //             Some(SymbolLookupOutput { found_sym_id, .. }) => found_sym_id.into(),
        //             None => {
        //                 let lookup_err = LookupError::SymbolNotFound {
        //                     sp_invalid_name_id: SpannedContainer::new(*interned_id, last_seg.span),
        //                     scope_searched: last_scope,
        //                 };
        //                 preset_reporter::report_preset(
        //                     self.compiler,
        //                     &mut self.summary,
        //                     lookup_err.into(),
        //                     env.region,
        //                     self.cfg,
        //                     self.interner,
        //                 );
        //                 None
        //             }
        //         }
        //     }
        //     PathSegment::Generic(_) => {
        //         let builder = SourceDiagnostic::builder(
        //             ErrorCode::GenericsErr.into(),
        //             DiagnosticLevel::Error,
        //             "Config root must be a user defined type",
        //             env.region.path_id,
        //         )
        //         .add_annotation(last_seg.span, AnnotationKind::Primary, None);
        //         self.summary.push_diag(builder.build());
        //         None
        //     }
        // };
        todo!()
    }

    //TODO: For resolve config member, pass in kind, which routes to declared methods. Call method
    //inside initial, then inside the router after getting root which is required.
    //
    //Both use options.
    //
    //Complex:
    // `TypeId` only as root. members must be field or enum members.
    //
    //Override:
    //`SymbolId` as root, which can be a namespace or type.

    /// Method that recursively resolves `ConfigDefMember` and `OptionAssignmentMember`
    ///
    /// This has no failure case because unknown fields have a diagnostic given to them then they're
    /// ignored, meaning there is no real discernment. May change if needed.
    fn resolve_cfg_member<'env>(
        &mut self,
        // For resolve_expr
        root_parent_impl_id: ImplId,
        root_span: SourceSpan,
        cfg_ctx: &ConfigMemberContextKind,
        // For tracking invalid recursive usage
        // Recursive errors no longer exist at the moment because override can only access known
        // configs like "types" inside of "RUST { types {} }".
        //
        // `complex` can only go two nesting levels so recursion isn't an issue there besides the
        // parent symbol which should be accounted for since it no longer innately is with this
        // being removed.
        // cfg_dfs: &mut Vec<(TypeId, SourceSpan)>,
        //TEST: For identifier tracking right now. Trying out something questionable.
        seen_cfg_idents: &mut Vec<SpannedContainer<InternedId>>,
        // Carried over and reset through cfg member recursive resolution to track duplicate
        // identifiers.
        seen_opt_idents: &mut DuplicateTracker<SpannedContainer<InternedId>>,
        parent_member_id: MemberId,
        parent_abs_cfg: &'env AbstractConfig,
        scope_type: ScopeType,
        depth: u8,
        env: &ResolverEnv,
    ) -> ImplMemberId {
        let AbstractConfigKind::Member(sp_parent_name_id, meta_kind) = parent_abs_cfg.kind.clone()
        else {
            unreachable!()
        };

        // Does this have to be reserved?
        //
        // Reserving spot since this is a recursive function
        let current_cfg_member_id = ImplMemberId::new(self.compiler.impl_members.len() as u32);
        self.compiler.impl_members.push(ImplMemberKind::Unknown {
            sp_name_id: sp_parent_name_id.clone(),
            reserved_member_id: current_cfg_member_id,
        });

        //TODO: MAKE THIS, UM, NOT THIS. LOOKS BAD.
        // THIS IS PROBABLY NOT GOING TO BE THAT BAD SINCE OVERRIDE WOULD HAVE AN ENTIRELY
        // DIFFERENT PROCESS FOR HOW IT CONSUMES DATA. I AM SCARED.

        //NOTE: How do we account for override?
        // Override doens't exist yet, but maybe, override has it's own specific method of
        // resolution, which makes the check make sense because it ONLY fails if it wasn't delegated
        // override, since override wouldn't account for depth.

        let associated_scope = AssociatedScopeKind::Module(env.current_mod);

        // Expected to be `OptionAssignmentMember`
        let mut opt_assignments: Vec<ImplMemberId> =
            Vec::with_capacity(parent_abs_cfg.abs_stmts.len());
        // Expected to be `ConfigDefMember`
        let mut cfg_members: Vec<ImplMemberId> =
            Vec::with_capacity(parent_abs_cfg.cfg_members.len());

        // Whether or not the parent config has a type doesn't matter for options since they only
        // apply to the current config, so this is fine.
        // &parent_abs_cfg.abs_stmts
        for abs_stmt in &parent_abs_cfg.abs_stmts {
            match abs_stmt {
                AbstractStmt::OptAssignment(abs_opt) => {
                    let sp_name_id = SpannedContainer::new(abs_opt.name_id, abs_opt.name_span);
                    seen_opt_idents.insert_or_store(sp_name_id);

                    let expr_id = match self.register_expr(
                        None,
                        &abs_opt.array_expr,
                        None,
                        associated_scope,
                        // This purposeful setting is done on purpose.
                        scope_type,
                        env,
                    ) {
                        Ok(expr_id) => expr_id,
                        Err(preset_err) => {
                            preset_reporter::report_preset(
                                &self.compiler,
                                &mut self.summary,
                                preset_err,
                                env.region,
                                self.cfg,
                                self.interner,
                            );

                            continue;
                        }
                    };

                    let impl_member_id = ImplMemberId::new(self.compiler.impl_members.len() as u32);
                    let opt = OptionAssignmentMember::new(
                        parent_member_id,
                        impl_member_id,
                        abs_opt.name_id,
                        abs_opt.name_span,
                        expr_id,
                    );

                    self.compiler
                        .impl_members
                        .push(ImplMemberKind::OptAssignmentMember(opt));
                    opt_assignments.push(impl_member_id);
                }
                AbstractStmt::MultiAssignType(abstract_type_multi_assign) => todo!(),
            }
        }

        for found in seen_opt_idents.found_dups.drain(..) {
            let preset_err = PresetErr::DuplicateIdents {
                sp_original: found.original,
                sp_dup: found.dup,
                classifier: ChrnClassifier::ConfigOption,
            };

            let builder = preset_reporter::create_diag_builder_preset(
                self.compiler,
                preset_err,
                env.region,
                self.cfg,
                self.interner,
            )
            .add_annotation(
                sp_parent_name_id.span,
                AnnotationKind::Secondary,
                "Found inside this config member".to_string().into(),
            );
            self.summary.push_diag(builder.build());
        }

        // Clearing for cfg members to use for their options
        seen_opt_idents.clear();

        // len() to truncate from `seen_cfg_member`
        let mut seen_cfg_len = seen_cfg_idents.len();
        // So that it knows where to start slicing up to len
        let seen_cfg_start = seen_cfg_len;

        // Variants with no type, like "enum State {Ready}" will have no type, so we need
        // to ensure there is actually a type id to look for.
        //
        // Only variants and fields can be config altered members right now.
        //
        // WARN: This may allow for some transient issues to exist where the thing SHOULD have a
        // type, but a bug happened earlier, which would be ignored from this if let silently
        // because some consumers actually do need this. We'll see.
        let parent_type_id_opt = self.compiler.get_type_id_from_member_id(parent_member_id);
        if let Some(parent_type_id) = parent_type_id_opt {
            for abs_cfg_member in &parent_abs_cfg.cfg_members {
                let AbstractConfigKind::Member(sp_member_name_id, _) = abs_cfg_member.kind.clone()
                else {
                    unreachable!()
                };

                seen_cfg_len += 1;
                seen_cfg_idents.push(sp_member_name_id.clone());

                //Complex
                let member_id = if scope_type == ScopeType::Complex {
                    // -- DEPTH HANDLING START --
                    // If the scope is complex, it's not override (which is an exception for deeper
                    // nesting in complex), and depth + 1 is 2 then err.
                    //
                    // This is handled in this specific context on purpose. If this were handled AFTER
                    // recursively going deeper instead of + 1, it would lose spanning information for
                    // the actual useful context to point at, and implementing history is likely not
                    // worth it to report one error.

                    // If the CURRENT config member, is NOT using override semantics, account for
                    // nesting depth.
                    //TODO: Maybe make this a method
                    if depth + 1 == lang::CFG_MAX_COMPLEX_NEST_LEVEL {
                        // Is this confusing?
                        // Maybe from the perspective of ownership this could make more sense?
                        let core_msg =
                            "Nesting level of 2 is too deep for a `complex` scope config";

                        let builder = SourceDiagnostic::builder(
                            ErrorCode::ConfigDeclErr.into(),
                            DiagnosticLevel::Error,
                            core_msg,
                            env.region.path_id,
                        )
                        // Pointing first nesting point
                        .add_annotation(
                            sp_parent_name_id.span,
                            AnnotationKind::Secondary,
                            "One level nesting".to_string().into(),
                        )
                        // Pointing to root
                        .add_annotation(
                            root_span,
                            AnnotationKind::Secondary,
                            "Root".to_string().into(),
                        )
                        // Pointing second nesting point
                        .add_annotation(
                            sp_member_name_id.span,
                            AnnotationKind::Primary,
                            "Two level nesting is too deep".to_string().into(),
                        )
                        // Is this ok to add?
                        // It's hard to tell what information to add since we have the type name and
                        // could point out just making a different top level config for it but - !{}}}
                        // Ok maybe that should happen
                        .add_help("Prefer defining another config root instead");
                        self.summary.push_diag(builder.build());

                        // Breaks instead of returning so that the present information about the current
                        // member can still be returned.
                        break;
                    }
                    // -- DEPTH HANDLING END --

                    match member_lookup::lookup_member(
                        self.compiler,
                        parent_type_id,
                        sp_member_name_id.inner,
                        MemberLookupPattern::NoRestrictions,
                    ) {
                        // These are split so that the theoretical ok and err paths are able to reduce
                        // boilerplate where needed
                        MemberLookupResult::Found(mem_id) => mem_id,
                        lookup_res => {
                            // For if something like an impossible member access is attempted, where it
                            // would duplicate diagnostics if there are more `ConfigDefMember`s associated
                            // with the current config member.
                            let mut should_break = false;

                            let parent_member_fmtted_ty =
                                Type::to_fmt(&self.compiler.types, parent_type_id);

                            //WARN: COMPLEX SPAN ROUTING FOR ALL OF THESE SO COULD NEED ALTERING
                            let src_diag = match lookup_res {
                                MemberLookupResult::ImpossibleTypeMemberAccess(type_id) => {
                                    should_break = true;

                                    // NOTE: These cannot be defined earlier because not all lookups have
                                    // the same guarantees
                                    //
                                    let parent_member_span =
                                        self.compiler.get_span_from_member_id(parent_member_id);

                                    let preset_err =
                                        PresetErr::Lookup(LookupError::ImpossibleTypeMemberAccess(
                                            SpannedContainer::new(
                                                Type::to_fmt(&self.compiler.types, type_id),
                                                parent_member_span,
                                            ),
                                        ));

                                    preset_reporter::create_diag_builder_preset(
                                        &self.compiler,
                                        preset_err,
                                        env.region,
                                        self.cfg,
                                        self.interner,
                                    )
                                    .add_annotation(
                                        sp_parent_name_id.span,
                                        AnnotationKind::Secondary,
                                        format!("Is type `{parent_member_fmtted_ty}`").into(),
                                    )
                                    .add_annotation(
                                        sp_member_name_id.span,
                                        AnnotationKind::Secondary,
                                        "Impossible member access".to_string().into(),
                                    )
                                    //TODO:
                                    .build()
                                }
                                MemberLookupResult::MemberNotFoundInType(type_id) => {
                                    //WARN: I BELIEVE this is fine because for this lookup result to be reached,
                                    // that would mean the type found CAN hold members, but it just didn't
                                    // have the identifier specified, which means it must be a symbol of
                                    // some kind.

                                    // Start of type declaration info
                                    let ty_sym_id = self
                                        .compiler
                                        .get_sym_id_from_type_id(type_id)
                                        .expect("NOT DONE YET");

                                    // May change depending on if structural types are compiler built-in to
                                    // where they don't have an innate attached span anymore.
                                    let ty_name_id = self.compiler.symbols[ty_sym_id].name_id;
                                    let ty_span = self
                                        .compiler
                                        .get_span_from_type_id(type_id)
                                        .expect("NOT DONE YET");
                                    // End of type declaration info

                                    let preset_err =
                                        PresetErr::Lookup(LookupError::MemberNotFound {
                                            searched_type_id: type_id,
                                            sp_searched_type_name_id: SpannedContainer::new(
                                                ty_name_id,
                                                sp_parent_name_id.span,
                                            ),
                                            not_found_name_id: sp_member_name_id.inner,
                                        });

                                    //TODO: RECURSIVELY TRACKING ENDS UP HERE FIX SHOULD BE APPLIED HERE IF
                                    //NEEDED

                                    // List available members?
                                    preset_reporter::create_diag_builder_preset(
                                        &self.compiler,
                                        preset_err,
                                        env.region,
                                        self.cfg,
                                        self.interner,
                                    )
                                    .add_annotation(
                                        //WARN: WAS  parent_ty_span
                                        ty_span,
                                        AnnotationKind::Secondary,
                                        format!("{} defined here", parent_member_fmtted_ty).into(),
                                    )
                                    .add_annotation(
                                        sp_member_name_id.span,
                                        AnnotationKind::Secondary,
                                        "Searched for this member".to_string().into(),
                                    )
                                    .build()
                                }
                                MemberLookupResult::Unknown(_) => {
                                    // NOTE: This is skipped right now because this will emit
                                    // incomprehensive errors deterministically. If the member parent is
                                    // unknown, and some random config member like "l" was typed, that would
                                    // make this emit `Type is not known` when "l" is just a random identifier.
                                    //
                                    // let core_msg = "Type is not known".to_string();
                                    // SourceDiagnostic::builder(
                                    //     DiagnosticLevel::Error,
                                    //     core_msg,
                                    //     env.region.path_id,
                                    // )
                                    // .add_annotation(abs_cfg_member.name_span, AnnotationKind::Primary, None)
                                    // .build()
                                    continue;
                                }
                                MemberLookupResult::Found(_) => unreachable!(),
                            };

                            self.summary.push_diag(src_diag);

                            if should_break {
                                break;
                            }

                            continue;
                        }
                    }
                } else {
                    panic!("We'll see");
                };

                let cfg_member_id = self.resolve_cfg_member(
                    root_parent_impl_id,
                    root_span,
                    cfg_ctx,
                    // cfg_dfs,
                    seen_cfg_idents,
                    seen_opt_idents,
                    member_id,
                    abs_cfg_member,
                    scope_type,
                    depth + 1,
                    env,
                );

                cfg_members.push(cfg_member_id);

                // If any cfg was added during the recursive descent, this truncates so that the vector
                // can be re-used where it left off.
                seen_cfg_idents.truncate(seen_cfg_len);
            }
        // Branch of no type being present within the parent config member.
        } else {
            // If this is the case, that means inner config members aren't allowed because a config
            // member is directly tied to the type's members, but if the type literally doesn't
            // exist then it cannot have members.
            //
            // Not sure what to do with override because override is supposed to act off
            // intrinsics, which can't be invalid. Type or no type override is the same so it should
            // probably just delegate to a method.
            if scope_type == ScopeType::Complex {
                // Only possible error for a complex semantic section. Members without types can
                // only use options.
                if let Some(first) = parent_abs_cfg.cfg_members.first() {
                    let AbstractConfigKind::Member(sp_member_name_id, _) = first.kind.clone()
                    else {
                        unreachable!()
                    };

                    let core_msg = "Cannot define config members for a type that has no members";
                    let builder = SourceDiagnostic::builder(
                        ErrorCode::ConfigDeclErr.into(),
                        DiagnosticLevel::Error,
                        core_msg,
                        env.region.path_id,
                    )
                    .add_annotation(
                        sp_parent_name_id.span,
                        AnnotationKind::Primary,
                        "Has no members".to_string().into(),
                    )
                    // Also Into<String>?
                    .add_annotation(
                        sp_member_name_id.span,
                        AnnotationKind::Secondary,
                        // 💀💀
                        "Can't exist".to_string().into(),
                    );
                    self.summary.push_diag(builder.build());
                }
            }
        }

        // Explicit declaration of slice range for duplicate naming to check for
        //
        // Start is the only variable that needs to be tracked here since the len being truncated
        // inherently sets end
        let seen_cfg_slice = &seen_cfg_idents[seen_cfg_start..];

        //TODO: Need cfg root span
        if let DuplicateIdentResult::Duplicate {
            sp_original,
            sp_dup,
        } = checker_helpers::check_duplicate_ident(seen_cfg_slice)
        {
            let preset_err = PresetErr::DuplicateIdents {
                sp_original,
                sp_dup,
                classifier: ChrnClassifier::ConfigRoot,
            };

            // let spans: Vec<SourceSpan> = sp_path_segs.iter().map(|s| s.span).collect();
            // let sp_path_span =
            //     source_span::merge_spans(&spans).expect("Path segments require at least one span");

            let builder = preset_reporter::create_diag_builder_preset(
                self.compiler,
                preset_err,
                env.region,
                self.cfg,
                self.interner,
            )
            .add_annotation(
                todo!(),
                AnnotationKind::Secondary,
                "Found inside this config root".to_string().into(),
            );
            self.summary.push_diag(builder.build());
        };

        for (i, current_cfg) in seen_cfg_slice.iter().enumerate() {
            if let Some((_, original_cfg)) = seen_cfg_slice
                .iter()
                .enumerate()
                // If the other index was declared after the current index and they have the same identifier
                //
                // Since this iteration specifically checks if the current was declared after the
                // last and the iteration terminates upon the first match, this correctly points at
                // the original field for all duplicates.
                .find(|(other_i, cfg)| *other_i < i && current_cfg.inner == cfg.inner)
            {
                let dup_name = self.interner.search(current_cfg.inner);

                let orig_span = original_cfg.span;
                let current_cfg_span = current_cfg.span;

                let core_msg = format!("More than one config member has identifier `{dup_name}`");

                let src_diag = SourceDiagnostic::builder(
                    None,
                    DiagnosticLevel::Error,
                    core_msg,
                    env.region.path_id,
                )
                .add_annotation(
                    sp_parent_name_id.span,
                    AnnotationKind::Secondary,
                    "Found inside this config member".to_string().into(),
                )
                .add_annotation(
                    orig_span,
                    AnnotationKind::Secondary,
                    format!("Original usage of `{dup_name}` here").into(),
                )
                .add_annotation(current_cfg_span, AnnotationKind::Primary, None)
                .build();

                self.summary.push_diag(src_diag);
            }
        }

        // Final step of assigning the actual config member

        let cfg_member = ConfigDefMember::new(
            sp_parent_name_id.inner,
            sp_parent_name_id.span,
            current_cfg_member_id,
            parent_member_id,
            parent_type_id_opt,
            meta_kind,
            parent_abs_cfg.lookup_pat,
            opt_assignments,
            cfg_members,
        );

        self.compiler.impl_members[current_cfg_member_id] =
            ImplMemberKind::ConfigDefMember(cfg_member);

        // Always returns a `ConfigDefMember` no matter how broken since diagnostics are already pushed
        current_cfg_member_id
    }

    fn try_resolve_pending(
        &mut self,
        resolved_sym_id: SymbolId,
        pending_sym: &mut PendingSymbol,
        env: &ResolverEnv,
        // Eyes
        // No actually why did this say eyes?
        // I feel close to why this said eyes. Still thinking.
    ) -> Result<bool, ()> {
        // Tells the caller if the given pending symbol is fully resolved to where it can be
        // removed as a pending symbol
        let mut can_remove = false;
        // (Idx insinde `pending_sym`, Corresponding Expr)
        let mut queue: Vec<(usize, ExprId)> = Vec::new();

        //Suspicious
        for (i, pending_expr) in pending_sym.pending_exprs.iter().enumerate() {
            match &pending_expr.kind {
                PendingExprKind::Parent(parent_base) => {
                    // Error being treated the same as a resolved expression since it can't be mutated
                    // further
                    //
                    // If fully resolved or err (impossible to solve) skip
                    if matches!(
                        parent_base.state,
                        ParentState::Notified {
                            has_resolved_ty: true,
                            has_const_val: true
                        } | ParentState::Error
                    ) {
                        continue;
                    }
                }
                PendingExprKind::Standing(state) => {
                    // Standing version of same check
                    if matches!(
                        //WARN: Make sure I work
                        state,
                        StandingExprState::Resolved {
                            has_resolved_ty: true,
                            has_const_val: true
                        } | StandingExprState::Error
                    ) {
                        continue;
                    }
                }
            }

            //WARN: Changed to store the tuple BEFORE the queue iteration since if an earlier part
            //of the resolution where to ever be skipped, the indexing into the queue would be wrong
            //because enumerate() over the queue may be smaller and misaligned with `pending_exprs`
            queue.push((i, pending_expr.pending_id));
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
        for (i, root_id) in queue.iter().copied() {
            // Still need to repair root expr
            let root_expr = &mut self.compiler.exprs[root_id];
            match self.compiler.symbols[resolved_sym_id].kind {
                SymbolKind::Variable(var_id) => {
                    let var = &self.compiler.variables[var_id];
                    let VariableState::Known(val_id) = var.state else {
                        continue;
                    };

                    if pending_sym.has_resolved_ty {
                        let val_info = &self.compiler.values[val_id];
                        let other_type_id = val_info.type_id;

                        // Avoid creating a self-referential `Deferred` type when the pending
                        // reference expression shares the same type slot as the symbol it depends on
                        //
                        // Could be a real issue that this needs to do this in the first place, but
                        // it's fine for now
                        if root_expr.type_id != other_type_id {
                            self.compiler.types[root_expr.type_id].ty =
                                Type::Deferred(other_type_id);
                        }

                        let inner_val = &mut self.compiler.values[root_expr.val_id];
                        if inner_val.type_id != other_type_id {
                            self.compiler.types[inner_val.type_id].ty =
                                Type::Deferred(other_type_id);
                        }
                    }

                    if pending_sym.has_const_val {
                        let val_info = &self.compiler.values[val_id];
                        // Clone could be circumvented by creating ANOTHER arena, which only
                        // contains `Value` types, which would mean the metadata of values is
                        // not directly tied to the const value, but not sure if that's worth it here.
                        let const_val_opt = val_info.const_val.clone();

                        let inner_val = &mut self.compiler.values[root_expr.val_id];
                        inner_val.const_val = const_val_opt;
                    }
                }
                // - NOT SURE WHAT THIS WAS RELATED TO -
                // NOTE: Since expressions are initialized as `ReservedTypeSlot`, if there is say,
                // a cyclic dependency error, the error will exist and emit later, but this
                // technically still exists and needs to be ignored. Not currently aware of any
                // direct issues with this. Maybe an Error tag on a pending expression could help?
                // - NOT SURE WHAT THIS WAS RELATED TO -
                //
                // These are unreachable because their symbols are never delayed in resolution.
                // Only expressions have a complex instantiation process.
                SymbolKind::ExternType
                | SymbolKind::Type(_)
                | SymbolKind::Namespace
                | SymbolKind::Directive(_) => {
                    unreachable!("Not possible")
                }
            }

            if let Some(user) = root_expr.user {
                // NOTE: This has yet to have had a bug in a long time even with the tests
                // surrounding it, which test particular types of dependency chains that put more pressure
                // on the more subject to error parts like the children notifying parents.
                //
                // WARN: Check if I work please
                match self.traverse_expr(user) {
                    Ok((has_resolved_ty, has_const_val)) => {
                        let pending_expr = &mut pending_sym.pending_exprs[i];

                        match &mut pending_expr.kind {
                            PendingExprKind::Parent(parent_base) => {
                                let has_new_info = match parent_base.state {
                                    ParentState::Unresolved => true,
                                    // Only value matters here since being resolved previous means there at
                                    // least is a resolved type present.
                                    ParentState::Resolved {
                                        has_resolved_ty: _,
                                        has_const_val: old_val,
                                    }
                                    | ParentState::Notified {
                                        has_resolved_ty: _,
                                        has_const_val: old_val,
                                    } => {
                                        // If we found a const and the value wasn't const previously
                                        // then this is a new const and returns true for new info.
                                        has_const_val && !old_val
                                    }
                                    ParentState::Error => false,
                                };

                                // NOTE: Can't remember why resolved type is checked
                                if has_new_info && has_resolved_ty {
                                    // Setting as resolved so main loop knows to update the parent
                                    parent_base.state = ParentState::Resolved {
                                        has_resolved_ty,
                                        has_const_val,
                                    };
                                }
                            }
                            PendingExprKind::Standing(state) => {
                                let has_new_info = match &state {
                                    StandingExprState::Unresolved => true,
                                    StandingExprState::Resolved {
                                        has_resolved_ty: _,
                                        has_const_val: old_val,
                                        // If we found a const and the value wasn't const previously
                                        // then this is a new const and returns true for new info.
                                    } => has_const_val && !old_val,
                                    StandingExprState::Error => false,
                                };

                                //WARN: Missing?
                                if has_new_info {
                                    // Setting as resolved so it can be removed if needed during
                                    // initial queue check
                                    *state = StandingExprState::Resolved {
                                        has_resolved_ty,
                                        has_const_val,
                                    };
                                }
                            }
                        };
                    }
                    // WARN: This case is not hit yet
                    // Reports the error and continues
                    Err(preset_err) => {
                        // Extracting module of origin from the pending expression by using the symbol
                        // attached to the expression upon it's creation
                        //WARN: Suspicious

                        preset_reporter::report_preset(
                            &self.compiler,
                            &mut self.summary,
                            preset_err,
                            env.region,
                            self.cfg,
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

                match &mut pending_expr.kind {
                    PendingExprKind::Parent(parent_base) => {
                        let has_new_info = match parent_base.state {
                            ParentState::Unresolved => true,
                            // Only value matters here since being resolved previous means there at
                            // least is a resolved type present.
                            ParentState::Resolved {
                                has_resolved_ty: _,
                                has_const_val: old_val,
                            }
                            | ParentState::Notified {
                                has_resolved_ty: _,
                                has_const_val: old_val,
                            } => {
                                // If we found a const and the value wasn't const previously
                                // then this is a new const and returns true for new info.
                                has_const_val && !old_val
                            }
                            ParentState::Error => false,
                        };

                        // NOTE: Can't remember why resolved type is checked
                        if has_new_info && has_resolved_ty {
                            // Setting as resolved so main loop knows to update the parent
                            parent_base.state = ParentState::Resolved {
                                has_resolved_ty,
                                has_const_val,
                            };
                        }
                    }
                    PendingExprKind::Standing(state) => {
                        let has_new_info = match state {
                            StandingExprState::Unresolved => true,
                            StandingExprState::Resolved {
                                has_resolved_ty: _,
                                has_const_val: old_val,
                                // If we found a const and the value wasn't const previously
                                // then this is a new const and returns true for new info.
                            } => has_const_val && !*old_val,
                            StandingExprState::Error => false,
                        };

                        //WARN: Missing?
                        if has_new_info {
                            // Setting as resolved so it can be removed if needed during
                            // initial queue check
                            *state = StandingExprState::Resolved {
                                has_resolved_ty,
                                has_const_val,
                            };
                        }
                    }
                };

                // -- ORIGINAL --
                // let has_new_info = match pending_expr.parent_state {
                //     ParentState::Unresolved => true,
                //     // Only value matters here since being resolved previous means there at
                //     // least is a resolved type present.
                //     ParentState::Notified(_, old_val) | ParentState::Resolved(_, old_val) => {
                //         has_const_val && !old_val
                //     }
                //     ParentState::Error => false,
                // };
                //
                // if has_new_info {
                //     pending_expr.parent_state =
                //         ParentState::Resolved(has_resolved_ty, has_const_val);
                // }
                // -- ORIGINAL --

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
        let expr = &self.compiler.exprs[current_expr_id];
        let val_info = &self.compiler.values[expr.val_id];

        //TEST:
        // Maybe types could always be inferred better? Although that doesn't really make sense
        // since if there is a type already inferred, if the types don't match then that's going to
        // error anyways depending on if the operation is applied
        let mut has_resolved_ty = !self.compiler.check_unknown(expr.type_id);
        let mut has_const_val = val_info.const_val.is_some();

        // But doesn't the queue disallow expressions that are resolved fully anyways? Wouldn't
        // this only need a const value check? Maybe.

        //TODO: Should use the booleans to prevent costly traversal operations
        match &self.compiler.exprs[current_expr_id].expr_hir {
            ExprHir::Val(val_id) => {
                // The root before traversal MUST be a singular expr that has a SymbolId inside of
                // it, which means anything further up the tree cannot reach that singular symbol
                // point again.
                unreachable!();
                // This is unreachable
                let val_info = &self.compiler.values[*val_id];

                let new_type_id = val_info.type_id;
                let const_val_opt = val_info.const_val.clone();

                has_resolved_ty = self.compiler.check_unknown(new_type_id);
                has_const_val = const_val_opt.is_some();

                let expr = &mut self.compiler.exprs[current_expr_id];
                // Mutating the type address so that it is now deferred to it's real type
                self.compiler.types[expr.type_id].ty = Type::Deferred(new_type_id);

                let inner_val = &mut self.compiler.values[expr.val_id];
                self.compiler.types[inner_val.type_id].ty = Type::Deferred(new_type_id);

                inner_val.type_id = new_type_id;
                inner_val.const_val = const_val_opt;

                todo!("Make sure this is ok")
            }
            ExprHir::Unary { op, operand } => {
                // Getting the operand that could be resolved (Might be guarnteed but um..e)
                let operand_expr = &self.compiler.exprs[*operand];

                let is_unknown = self.compiler.check_unknown(operand_expr.type_id);
                // This means that we reached an expression inside of a resolved expression that is
                // not fully resolved yet, which is fine.
                if is_unknown {
                    return Ok((false, false));
                }

                has_resolved_ty = true;

                let operand_val_info = &self.compiler.values[operand_expr.val_id];

                // Basic validation of expression to see if it's const or runtime
                let const_val_opt = if let Some(const_val) = &operand_val_info.const_val {
                    has_const_val = true;
                    let operand_span = operand_expr.meta.expect_user();
                    let sp_const =
                        SpannedContainerRef::new(const_val, operand_expr.meta.expect_user());
                    match evaluator::apply_unary_op(*op, sp_const) {
                        UnaryOpResult::Output(val) => Some(val),
                        UnaryOpResult::Invalid => {
                            return Err(MathError::UnaryOpMismatch {
                                sp_operand: SpannedContainer::new(const_val.kind(), operand_span),
                                op: *op,
                            })?;
                        }
                    }
                } else {
                    None
                };

                let new_type_id = operand_expr.type_id;

                // Should this be deferred or new?
                //
                // Mutating expression's type so that the symbol using this expr reflects the new
                // information
                let expr = &mut self.compiler.exprs[current_expr_id];
                self.compiler.types[expr.type_id].ty = Type::Deferred(new_type_id);

                // Mutating inner value so that the symbol using this value reflects the new
                // information
                let inner_val = &mut self.compiler.values[expr.val_id];
                self.compiler.types[inner_val.type_id].ty = Type::Deferred(new_type_id);
                inner_val.const_val = const_val_opt;
            }
            ExprHir::BinaryExpr { lhs, op, rhs } => {
                //TODO: Considering a span vector so that they dont need to be duplicated or
                //computed by going inside items anymore.

                let lhs_expr = &self.compiler.exprs[*lhs];
                let rhs_expr = &self.compiler.exprs[*rhs];

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
                let lhs_val_opt = self.compiler.values[lhs_expr.val_id].const_val.as_ref();

                let rhs_val_opt = self.compiler.values[rhs_expr.val_id].const_val.as_ref();

                // This just checks if both are const, not if they were comptaible in the first
                // place. So, if it's not a comptaible binary, that could either mean 2 + "hi" or 2
                // + x where we just don't know x yet
                let const_val_opt: Option<Value> = match (lhs_val_opt, rhs_val_opt) {
                    (Some(lhs_const), Some(rhs_const)) => {
                        //TODO: Handle BigInt
                        has_const_val = true;
                        let lhs_span = lhs_expr.meta.expect_user();
                        let rhs_span = rhs_expr.meta.expect_user();

                        let sp_lhs_const = SpannedContainerRef::new(lhs_const, lhs_span);
                        let sp_rhs_const = SpannedContainerRef::new(rhs_const, rhs_span);
                        match evaluator::apply_binary_op(
                            sp_lhs_const,
                            *op,
                            sp_rhs_const,
                            self.interner,
                        ) {
                            evaluator::BinaryOpResult::Output(val) => Some(val),
                            evaluator::BinaryOpResult::DivideByZero => {
                                return Err(MathError::DivideByZero { lhs_span, rhs_span }.into());
                            }
                            evaluator::BinaryOpResult::Invalid => {
                                return Err(MathError::BinaryOpMismatch {
                                    sp_lhs: SpannedContainer::new(lhs_const.kind(), lhs_span),
                                    sp_rhs: SpannedContainer::new(rhs_const.kind(), rhs_span),
                                    op: *op,
                                })?;
                            }
                        }
                    }
                    _ => None,
                };

                //WARN: Suspicious
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
                let expr = &mut self.compiler.exprs[current_expr_id];
                // Assigning directly since this is a newly created type id..
                expr.type_id = new_type_id;
                // dbg!(expr.type_id, new_type_id);
                // self.compiler.types[expr.type_id ].ty = panic!();

                let inner_val = &mut self.compiler.values[expr.val_id];
                inner_val.type_id = new_type_id;
                // self.compiler.types[inner_val.type_id ].ty = Type::Deferred(new_type_id);
                inner_val.const_val = const_val_opt;
            }
            ExprHir::Call(expr_id, expr_ids) => todo!(),
            ExprHir::Var(sym_id) => {
                // The root before traversal MUST be a singular expr that has a SymbolId inside of
                // it, which means anything further up the tree cannot reach that singular symbol
                // point again.
                unreachable!()
            }
            ExprHir::Default(sym_id, expr_id) => {
                todo!("Default not finished")
            }
            ExprHir::Array(expr_ids) => {
                //TODO: Need to require const here
                // So, maybe need to look at the context at some point later, or just typecheck.
                // tybejeg TYPE check
                let array = &self.compiler.exprs[current_expr_id];
                let array_len = expr_ids.len();

                let mut type_id_opt: Option<TypeId> = None;
                let mut found_const_vals = 0;

                // If unknown then try to find an element that has a type inferred
                if self.compiler.check_unknown(array.type_id) {
                    for expr_id in expr_ids {
                        let expr = &self.compiler.exprs[*expr_id];

                        //WARN: Need to typecheck this too later
                        if !self.compiler.check_unknown(expr.type_id) && type_id_opt.is_none() {
                            type_id_opt = Some(expr.type_id);
                        }

                        let val_info = &self.compiler.values[expr.val_id];
                        if val_info.const_val.is_some() {
                            found_const_vals += 1;
                        }
                    }
                }

                if !has_const_val && found_const_vals == array_len {
                    has_const_val = true;

                    let mut values: Vec<Value> = Vec::with_capacity(expr_ids.len());
                    for expr_id in expr_ids {
                        let val_id = &self.compiler.exprs[*expr_id].val_id;
                        // Is cloned so that the value can be owned in memory by the array itself.
                        // This could theoretically be avoided by adding a separation between value
                        // info vector and the actual value, where value ids would purely contain
                        // the value and not ruin associated metadata, but not done right now.
                        let val = self.compiler.values[*val_id]
                            .const_val
                            .as_ref()
                            .expect("Previous loop failed")
                            .clone();

                        values.push(val);
                    }

                    let array_expr = &mut self.compiler.exprs[current_expr_id];
                    let array_val = &mut self.compiler.values[array_expr.val_id];
                    array_val.const_val = Some(Value::Array(values));
                }

                // This is setting a type id everytime. May be concerning.
                if !has_resolved_ty {
                    if let Some(new_type_id) = type_id_opt {
                        let array = &mut self.compiler.exprs[current_expr_id];
                        array.type_id = new_type_id;
                        has_resolved_ty = true;
                    }
                }
            }
        }

        // Traversing up tree
        let expr = &self.compiler.exprs[current_expr_id];
        //WARN: Seems to be working
        if let Some(user) = expr.user {
            return self.traverse_expr(user);
        }

        Ok((has_resolved_ty, has_const_val))
    }

    fn resolve_var(&mut self, parent_sym_id: SymbolId, env: &ResolverEnv) {
        let ast_id = self.compiler.symbols[parent_sym_id]
            .ast_id
            .expect("Should be user symbols only");
        let abs_var = env.ast_info.get_var(ast_id);

        let associated_scope = AssociatedScopeKind::Module(env.current_mod);
        let scope_type = self.compiler.symbols[parent_sym_id].scope_origin;

        //NOTE: Pipeline where expressions are always returned, just that some may have
        //unresolved parts, which are put into the queue, not the variable itself.
        let expr_id = match self.register_expr(
            parent_sym_id.into(),
            &abs_var.spanned_expr,
            None,
            associated_scope,
            scope_type,
            env,
        ) {
            Ok(expr_id) => expr_id,
            Err(preset_err) => {
                preset_reporter::report_preset(
                    &self.compiler,
                    &mut self.summary,
                    preset_err,
                    env.region,
                    self.cfg,
                    self.interner,
                );

                //TODO: use AstExpr::Error
                return;
            }
        };

        let expr = &self.compiler.exprs[expr_id];
        let val = &self.compiler.values[expr.val_id];

        //                      NOT unknown
        let has_resolved_ty = !self.compiler.check_unknown(expr.type_id);
        let has_const_val = val.const_val.is_some();

        // The top expr is given as the value innately for the variable.
        let val_id = expr.val_id;

        // Sets the symbol's value to be the last expression's value so that later, if it's
        // expression is resolved further, since it's already pointing the the same expression it
        // will by proxy be updated

        //WARN: MAKE SURE EXPRESSION RESOLUTION IS NOT BROKEN FROM VAR CHANGES
        // dbg!(&self.compiler.types[TypeId::new(43)]);
        // panic!();
        let var = self.compiler.get_var_mut(parent_sym_id);
        var.state = VariableState::Known(val_id);

        // If the symbol that was just examined is a pending symbol AND it was actually resolved,
        // then it'll be marked as resolved
        if let Some(pending_sym) = self.ty_ctx.sym_queue.get_mut(&parent_sym_id) {
            // Three flags for resolver use
            pending_sym.has_resolved_ty = has_resolved_ty;
            pending_sym.has_const_val = has_const_val;

            self.ty_ctx.needs_check = true;
        }
    }

    fn resolve_typedef(&mut self, parent_sym_id: SymbolId, env: &ResolverEnv) {
        let ast_id = self.compiler.symbols[parent_sym_id]
            .ast_id
            .expect("Should be user symbols only");
        let abs_typedef = env.ast_info.get_typedef(ast_id);
        let associated_scope = AssociatedScopeKind::Module(env.current_mod);
        let scope_type = self.compiler.symbols[parent_sym_id].scope_origin;

        let type_id = match resolution_helpers::resolve_type_expr_ret_preset(
            &mut self.compiler,
            AssociatedScopeKind::Module(env.current_mod),
            &abs_typedef.sp_ty_expr,
            scope_type,
            ScopeLookupPattern::NoRestrictions,
            self.interner,
            env,
        ) {
            Ok(type_id) => type_id,
            Err(preset_err) => {
                preset_reporter::report_preset(
                    &self.compiler,
                    &mut self.summary,
                    preset_err,
                    env.region,
                    self.cfg,
                    self.interner,
                );
                TypeId::new(compiler_constants::CORE_UNKNOWN)
            }
        };

        let mut conds: Vec<ExprId> = Vec::with_capacity(abs_typedef.conds.len());
        for spanned_expr in &abs_typedef.conds {
            //FIX: Scope type is a little wrong here since it's a condition
            match self.register_expr(
                parent_sym_id.into(),
                spanned_expr,
                None,
                associated_scope,
                scope_type,
                env,
            ) {
                // For allowing for more diagnostics instead of just leaving the rest of the struct
                // unfinished upon singular errors
                Ok(c) => conds.push(c),
                Err(preset_err) => {
                    preset_reporter::report_preset(
                        &self.compiler,
                        &mut self.summary,
                        preset_err,
                        env.region,
                        self.cfg,
                        self.interner,
                    );
                }
            }
        }

        let (directives, preset_errs) = self.handle_directives(&abs_typedef.directives, env);
        preset_reporter::report_preset_vec(
            &self.compiler,
            &mut self.summary,
            preset_errs,
            env.region,
            self.cfg,
            self.interner,
        );

        let type_def = self.compiler.get_typedef_mut(parent_sym_id);

        debug_assert_eq!(type_def.conds.len(), 0);
        debug_assert_eq!(type_def.directives.len(), 0);

        // Assinging from `Unknown` to it's actual type if found
        //TODO: Constraints should check if this is unknown
        type_def.type_id = type_id;
        type_def.conds = conds;
        // Maybe directives will stay defined here
        type_def.directives = directives;
    }

    fn resolve_struct(&mut self, parent_sym_id: SymbolId, env: &ResolverEnv) {
        let ast_id = self.compiler.symbols[parent_sym_id]
            .ast_id
            .expect("Should be user symbols only");
        let abs_struct = env.ast_info.get_struct(ast_id);
        // Not sure of if this should stay a Field type or just be a TypeDef since their intent
        // somewhat conflicts. For now, typedef is just consumed differently depending on if it's a
        // field declared in var-> or not since var-> fields may be made possible to reference, but
        // fields in structures can't. Will possibly just be unified in the future.

        //TODO: global condition and argument setting.
        //field arg and cond settings.
        //same for enums.

        let associated_scope = AssociatedScopeKind::Module(env.current_mod);
        let scope_type = self.compiler.symbols[parent_sym_id].scope_origin;

        let fields: Vec<MemberId> = self.compiler.get_struct(parent_sym_id).fields.clone();

        for (i, current_member_id) in fields.iter().enumerate() {
            let abs_field = &abs_struct.fields[i];
            let mut conds: Vec<ExprId> = Vec::with_capacity(abs_field.conds.len());

            for cond in &abs_field.conds {
                match self.register_expr(
                    parent_sym_id.into(),
                    &cond,
                    None,
                    associated_scope,
                    scope_type,
                    env,
                ) {
                    Ok(c) => conds.push(c),
                    Err(preset_err) => {
                        preset_reporter::report_preset(
                            &self.compiler,
                            &mut self.summary,
                            preset_err,
                            env.region,
                            self.cfg,
                            self.interner,
                        );
                    }
                };
            }

            let (directives, preset_errs) = self.handle_directives(&abs_field.directives, env);
            preset_reporter::report_preset_vec(
                &self.compiler,
                &mut self.summary,
                preset_errs,
                env.region,
                self.cfg,
                self.interner,
            );

            let field = self.compiler.get_field_mut(*current_member_id);

            debug_assert_eq!(field.conds.len(), 0);
            debug_assert_eq!(field.directives.len(), 0);

            field.conds = conds;
            field.directives = directives;
        }

        let mut glob_conds: Vec<ExprId> = Vec::with_capacity(abs_struct.glob_conds.len());

        for cond in &abs_struct.glob_conds {
            match self.register_expr(
                parent_sym_id.into(),
                cond,
                None,
                associated_scope,
                scope_type,
                env,
            ) {
                Ok(c) => glob_conds.push(c),
                Err(preset_err) => {
                    preset_reporter::report_preset(
                        &self.compiler,
                        &mut self.summary,
                        preset_err,
                        env.region,
                        self.cfg,
                        self.interner,
                    );
                }
            }
        }

        let (glob_directives, preset_errs) =
            self.handle_directives(&abs_struct.glob_directives, env);

        preset_reporter::report_preset_vec(
            &self.compiler,
            &mut self.summary,
            preset_errs,
            env.region,
            self.cfg,
            self.interner,
        );

        let struct_def = self.compiler.get_struct_mut(parent_sym_id);

        debug_assert_eq!(struct_def.glob_conds.len(), 0);
        debug_assert_eq!(struct_def.glob_directives.len(), 0);

        struct_def.glob_conds = glob_conds;
        struct_def.glob_directives = glob_directives;
    }

    fn resolve_enum(&mut self, parent_sym_id: SymbolId, env: &ResolverEnv) {
        let ast_id = self.compiler.symbols[parent_sym_id]
            .ast_id
            .expect("Should be user symbols only");
        let abs_enum = env.ast_info.get_enum(ast_id);
        let associated_scope = AssociatedScopeKind::Module(env.current_mod);
        let scope_type = self.compiler.symbols[parent_sym_id].scope_origin;

        // Clone needed so iteration doesn't make the compiler borrow itself twice
        let variants = self.compiler.get_enum(parent_sym_id).variants.clone();

        for (i, current_member_id) in variants.iter().enumerate() {
            let abs_variant = &abs_enum.variants[i];
            let mut conds: Vec<ExprId> = Vec::with_capacity(abs_variant.conds.len());

            for cond in &abs_variant.conds {
                let cond_opt = match self.register_expr(
                    parent_sym_id.into(),
                    &cond,
                    None,
                    associated_scope,
                    scope_type,
                    env,
                ) {
                    Ok(c) => Some(c),
                    Err(preset_err) => {
                        preset_reporter::report_preset(
                            &self.compiler,
                            &mut self.summary,
                            preset_err,
                            env.region,
                            self.cfg,
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
                &self.compiler,
                &mut self.summary,
                preset_errs,
                env.region,
                self.cfg,
                self.interner,
            );

            let variant = self.compiler.get_variant_mut(*current_member_id);

            debug_assert_eq!(variant.conds.len(), 0);
            debug_assert_eq!(variant.directives.len(), 0);

            variant.conds = conds;
            variant.directives = directives;
        }

        let mut glob_conds: Vec<ExprId> = Vec::with_capacity(abs_enum.glob_conds.len());
        for cond in &abs_enum.glob_conds {
            let cond_opt = match self.register_expr(
                parent_sym_id.into(),
                cond,
                None,
                associated_scope,
                scope_type,
                env,
            ) {
                Ok(c) => Some(c),
                Err(preset_err) => {
                    preset_reporter::report_preset(
                        &self.compiler,
                        &mut self.summary,
                        preset_err,
                        env.region,
                        self.cfg,
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
            &self.compiler,
            &mut self.summary,
            preset_errs,
            env.region,
            self.cfg,
            self.interner,
        );

        let enum_def = self.compiler.get_enum_mut(parent_sym_id);

        debug_assert_eq!(enum_def.glob_conds.len(), 0);
        debug_assert_eq!(enum_def.glob_directives.len(), 0);
        enum_def.glob_conds = glob_conds;
        enum_def.glob_directives = glob_directives;
    }

    fn resolve_alias(
        &mut self,
        parent_sym_id: SymbolId,
        ident_tracker: &mut DuplicateTracker<SpannedContainer<InternedId>>,
        env: &ResolverEnv,
    ) {
        let ast_id = self.compiler.symbols[parent_sym_id]
            .ast_id
            .expect("Should be user symbols only");
        let abs_alias = env.ast_info.get_alias(ast_id);
        let associated_scope = AssociatedScopeKind::Module(env.current_mod);
        let local_scope_id = self.compiler.get_alias(parent_sym_id).local_scope_id;
        let scope_type = self.compiler.symbols[parent_sym_id].scope_origin;

        let mut params: Vec<Param> = Vec::with_capacity(abs_alias.params.len());

        // Just a bit crowded in here..
        // WARN: Ok this just looks like an inlined function now
        for (i, abs_param) in abs_alias.params.iter().enumerate() {
            let sp_name_id = SpannedContainer::new(abs_param.name_id, abs_param.name_span);
            ident_tracker.insert_or_store(sp_name_id);

            //TODO: SHOULD THIS BE A VARIABLE?
            let expr_id = ExprId::new(self.compiler.exprs.len() as u32);
            let val_id = ValueId::new(self.compiler.values.len() as u32);

            let type_id = match resolution_helpers::resolve_type_expr_ret_preset(
                self.compiler,
                AssociatedScopeKind::Module(env.current_mod),
                &abs_param.sp_ty_expr,
                scope_type,
                ScopeLookupPattern::NoRestrictions,
                self.interner,
                env,
            ) {
                Ok(type_id) => type_id,
                Err(preset_err) => {
                    preset_reporter::report_preset(
                        &self.compiler,
                        &mut self.summary,
                        preset_err,
                        env.region,
                        self.cfg,
                        self.interner,
                    );
                    TypeId::new(compiler_constants::CORE_UNKNOWN)
                }
            };

            let param_sym_id = SymbolId::new(self.compiler.symbols.len() as u32);
            let var_id = VariableId::new(self.compiler.variables.len() as u32);

            let var = VarDef::new(
                param_sym_id,
                abs_param.name_id,
                VariableMetadata::User(abs_param.name_span),
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
            let resolved_expr = ResolvedExpr::new(
                type_id,
                expr_hir,
                val_id,
                ResolvedExprMetadata::User(abs_param.name_span),
                Vec::new(),
            );

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

        for found in ident_tracker.found_dups.drain(..) {
            let preset_err = PresetErr::DuplicateIdents {
                sp_original: found.original,
                sp_dup: found.dup,
                classifier: ChrnClassifier::Parameter,
            };

            let builder = preset_reporter::create_diag_builder_preset(
                self.compiler,
                preset_err,
                env.region,
                self.cfg,
                self.interner,
            )
            .add_annotation(
                abs_alias.name_span,
                AnnotationKind::Secondary,
                "Found inside this alias".to_string().into(),
            );
            self.summary.push_diag(builder.build());
        }

        let mut conds: Vec<ExprId> = Vec::with_capacity(abs_alias.conds.len());
        for spanned_expr in &abs_alias.conds {
            let cond_opt = match self.register_expr(
                parent_sym_id.into(),
                spanned_expr,
                Some(local_scope_id),
                //NOTE: Could this change?
                associated_scope,
                scope_type,
                env,
            ) {
                Ok(c) => Some(c),
                Err(preset_err) => {
                    preset_reporter::report_preset(
                        &self.compiler,
                        &mut self.summary,
                        preset_err,
                        env.region,
                        self.cfg,
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
            &self.compiler,
            &mut self.summary,
            preset_errs,
            env.region,
            self.cfg,
            self.interner,
        );

        //TODO: Arg constraint and option tpe constraint.
        //Could technically happen in constraint resolver since it. Yes.
        let alias_def = self.compiler.get_alias_mut(parent_sym_id);
        let param_count = params.len() as u32;

        debug_assert_eq!(alias_def.conds.len(), 0);
        debug_assert_eq!(alias_def.directives.len(), 0);

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
    }
    // These params are getting a little inflated so maybe a ctx struct for this environment could
    // be @()@$_ something

    /// On `Ok`, Creates a HIR expression type and returns the `ExprId` which is either going to be
    /// fully resolved, or marked as pending to be resolved later if possible.
    // The innate `parent_sym_id` may be weird depending on the context
    // Should it be replaced with VariableId?
    fn register_expr(
        &mut self,
        // Is `Option` because not all expressions being registered are attached to variables.
        // So, in "let x = y" we would want the `SymbolId` of `x` to check for cyclic deps, but if
        // we were just typing "[x, y]" there are no cycles because there is no assignment
        parent_sym_id_opt: Option<SymbolId>,
        spanned_expr: &SpannedExpr,
        // Only usable with something like, alias(x) where x is local, not section local overall
        // like var->
        local_scope_id: Option<ScopeId>,
        associated_scope: AssociatedScopeKind,
        scope_type: ScopeType,
        env: &ResolverEnv,
    ) -> Result<ExprId, PresetErr> {
        let lookup_pref =
            ScopeLookupPreferenceFlags::new(ScopeLookupPreferenceFlags::VARIABLE.into());
        match &spanned_expr.expr {
            AstExpr::Var(name_id) => {
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
                        let expr = match self.compiler.symbols[local_sym_id].kind {
                            SymbolKind::Variable(var_id) => {
                                let var = &self.compiler.variables[var_id];
                                let expr_hir = ExprHir::Var(local_sym_id);

                                let VariableState::Known(val_id) = var.state else {
                                    unreachable!("Not possible right now")
                                };

                                let type_id = self.compiler.values[val_id].type_id;

                                ResolvedExpr::new(
                                    type_id,
                                    expr_hir,
                                    val_id,
                                    ResolvedExprMetadata::User(spanned_expr.span),
                                    Vec::new(),
                                )
                            }
                            // Local scopes can't reach these right now
                            SymbolKind::Type(type_id) => todo!(),
                            SymbolKind::Namespace => todo!(),
                            SymbolKind::Directive(directive_id) => todo!(),
                            SymbolKind::ExternType => todo!(),
                        };

                        self.compiler.exprs.push(expr);

                        return Ok(expr_id);
                    }
                }

                // Searching if the given identifier is within the current environment
                if let Some(SymbolLookupOutput { found_sym_id, .. }) = scopes::find_sym_id(
                    self.compiler,
                    associated_scope,
                    *name_id,
                    scope_type,
                    // Should this be no restrictions?
                    ScopeLookupPattern::NoRestrictions,
                    lookup_pref,
                ) {
                    //WARN: Constant iteration upon seeing any symbol instead of a single check
                    //elsewhere
                    // Code duplication reduction
                    //
                    // If
                    if let Some(parent_sym_id) = parent_sym_id_opt {
                        self.check_cycle(parent_sym_id, found_sym_id, env)?;

                        //NOTE: Only the PendingSymbol struct carries the PendingExpr struct, meaning
                        //there is no way to check for cycles outside of `TypeContext`, so this has to
                        //pick up the edge case of, "let x = x". Could change.
                        if found_sym_id == parent_sym_id {
                            let name = self
                                .interner
                                .search(self.compiler.symbols[found_sym_id].name_id);

                            let core_msg = format!("Cannot declare symbol `{name}` as itself");

                            let dup_span = spanned_expr.span;

                            //FIX: Not failable since the exntire expression has to be placed in one module,
                            // to error to begin with, but should still operate off stored spans
                            let parent_ast_id = self.compiler.symbols[parent_sym_id]
                                .ast_id
                                .expect("Should be user symbol");

                            let parent_span = env.ast_info.get_name_span(parent_ast_id);

                            let src_diag = SourceDiagnostic::builder(
                                None,
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
                    }

                    let symbol = &self.compiler.symbols[found_sym_id];
                    let expr_id = ExprId::new(self.compiler.exprs.len() as u32);

                    // I don't think this is needed since types are already known
                    let resolved_expr = match symbol.kind {
                        //WARN: Should this be the same?
                        SymbolKind::Type(type_id) => {
                            // Not sure what to do with this yet
                            // This would make types expressions, which wasn't true before
                            let ty_info = &self.compiler.types[type_id];
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
                                        ResolvedExprMetadata::User(spanned_expr.span),
                                        Vec::new(),
                                    )
                                }
                                Type::BuiltinTypeInfo(_)
                                | Type::Struct(_)
                                | Type::Enum(_)
                                | Type::TypeDef(_)
                                | Type::Unknown => {
                                    let core_msg =
                                        "Cannot have a type within expressions".to_string();

                                    let src_diag = SourceDiagnostic::builder(
                                        None,
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
                                Type::Boundaries(ty_constraint) => todo!(),
                                Type::Deferred(type_id) => todo!("Is this possible?"),
                            }
                        }
                        SymbolKind::Variable(var_id) => {
                            let var = &self.compiler.variables[var_id];

                            match var.state {
                                // A value is attached to the variable found
                                VariableState::Known(val_id) => {
                                    let val_info = &self.compiler.values[val_id];
                                    let ty = &self.compiler.types[val_info.type_id].ty;

                                    // The type of the variable is unknown meaning it still needs
                                    // to await
                                    //WARN:
                                    if let Type::Unknown = ty {
                                        let pending_kind = if let Some(id) = parent_sym_id_opt {
                                            let parent_base =
                                                ParentStateBase::new(id, ParentState::Unresolved);
                                            PendingExprKind::Parent(parent_base)
                                        } else {
                                            PendingExprKind::Standing(StandingExprState::Unresolved)
                                        };

                                        let pending_expr = PendingExpr::new(expr_id, pending_kind);
                                        self.ty_ctx.store_pending_expr(found_sym_id, pending_expr);
                                    }

                                    let expr_hir = ExprHir::Var(found_sym_id);

                                    ResolvedExpr::new(
                                        val_info.type_id,
                                        expr_hir,
                                        val_id,
                                        ResolvedExprMetadata::User(spanned_expr.span),
                                        Vec::new(),
                                    )
                                }
                                VariableState::ReservedTypeSlot(reserved_ty_id) => {
                                    let expr_id = ExprId::new(self.compiler.exprs.len() as u32);
                                    let expr_hir = ExprHir::Var(found_sym_id);

                                    let pending_kind = if let Some(id) = parent_sym_id_opt {
                                        let parent_base =
                                            ParentStateBase::new(id, ParentState::Unresolved);
                                        PendingExprKind::Parent(parent_base)
                                    } else {
                                        PendingExprKind::Standing(StandingExprState::Unresolved)
                                    };

                                    let pending_expr = PendingExpr::new(expr_id, pending_kind);

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
                                        ResolvedExprMetadata::User(spanned_expr.span),
                                        Vec::new(),
                                    )
                                }
                            }
                        }
                        SymbolKind::Namespace => {
                            let err_mod_name = self.interner.search(*name_id);
                            // TODO: Should send help, which should be done after re-doing how
                            // errors are rendered
                            let core_msg = format!(
                                "`{err_mod_name}` is a namspace, which cannot be assigned as an expression"
                            );

                            let src_diag = SourceDiagnostic::builder(
                                None,
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
                        // Not possible
                        SymbolKind::Directive(_) => unreachable!("We'll see"),
                        // FIXME: There may need to be an expr result specifically for an extern type.
                        SymbolKind::ExternType => todo!(),
                    };

                    self.compiler.exprs.push(resolved_expr);

                    Ok(expr_id)
                } else {
                    let ident = self.interner.search(*name_id);

                    let module = &self.compiler.mods[env.current_mod];
                    let mod_name = self.interner.search(module.name_id);

                    let and_local = if local_scope_id.is_some() {
                        " and local scopes"
                    } else {
                        ""
                    };

                    let core_msg = format!("`{ident}` not found in module `{mod_name}`{and_local}");

                    let src_diag = SourceDiagnostic::builder(
                        ErrorCode::ScopeErr.into(),
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
            AstExpr::Integer(name_id, _) => {
                if let Ok(num) = self.interner.search(*name_id).parse::<i64>() {
                    // Getting what it's spot would be when it's expression and value parts are
                    // pushed
                    let expr_id = ExprId::new(self.compiler.exprs.len() as u32);
                    let val_id = ValueId::new(self.compiler.values.len() as u32);

                    // Creating it's default type to the literal value of integer, as well as it's
                    // expression of just being a singular value type
                    let expr_hir = ExprHir::Val(val_id);
                    let type_id = TypeId::new(compiler_constants::CORE_I64);

                    let resolved_expr = ResolvedExpr::new(
                        type_id,
                        expr_hir,
                        val_id,
                        ResolvedExprMetadata::User(spanned_expr.span),
                        Vec::new(),
                    );

                    // Creating the actual value portion of the expression
                    let val = Value::I64(num);
                    let val_info = ValueInfo::new(type_id, expr_id, Some(val));

                    self.compiler.values.push(val_info);
                    self.compiler.exprs.push(resolved_expr);

                    Ok(expr_id)
                } else {
                    Err(PresetErr::NumericOverflow {
                        sp_num: SpannedContainer::new(*name_id, spanned_expr.span),
                        fmtted_ty: ChrnClassifier::Integer,
                    })
                }
            }
            AstExpr::Float(name_id, _) => {
                // No BigFloat yet
                if let Ok(num) = self.interner.search(*name_id).parse::<f64>() {
                    let expr_id = ExprId::new(self.compiler.exprs.len() as u32);
                    let val_id = ValueId::new(self.compiler.values.len() as u32);

                    let expr_hir = ExprHir::Val(val_id);
                    let type_id = TypeId::new(compiler_constants::CORE_F64);
                    let expr = ResolvedExpr::new(
                        type_id,
                        expr_hir,
                        val_id,
                        ResolvedExprMetadata::User(spanned_expr.span),
                        Vec::new(),
                    );

                    let val = Value::F64(num);
                    let val_info = ValueInfo::new(type_id, expr_id, Some(val));

                    self.compiler.values.push(val_info);
                    self.compiler.exprs.push(expr);

                    Ok(expr_id)
                } else {
                    Err(PresetErr::NumericOverflow {
                        sp_num: SpannedContainer::new(*name_id, spanned_expr.span),
                        fmtted_ty: ChrnClassifier::Float,
                    })
                }
            }
            AstExpr::BinaryExpr { lhs, op, rhs } => {
                let lhs_id = self.register_expr(
                    parent_sym_id_opt,
                    &*lhs,
                    local_scope_id,
                    associated_scope,
                    scope_type,
                    env,
                )?;

                let rhs_id = self.register_expr(
                    parent_sym_id_opt,
                    &*rhs,
                    local_scope_id,
                    associated_scope,
                    scope_type,
                    env,
                )?;

                let lhs_expr = &self.compiler.exprs[lhs_id];
                let rhs_expr = &self.compiler.exprs[rhs_id];

                let lhs_is_unknown = self.compiler.check_unknown(lhs_expr.type_id);
                let rhs_is_unknown = self.compiler.check_unknown(rhs_expr.type_id);

                // Can't look at the word "clean" the same again.

                // Composing this so it can be matched cleanly for if const eval can be performed
                let lhs_val_opt = self.compiler.values[lhs_expr.val_id].const_val.as_ref();
                let rhs_val_opt = self.compiler.values[rhs_expr.val_id].const_val.as_ref();

                // This just checks if both are const, not if they were comptaible in the first
                // place. So, if it's not a comptaible binary, that could either mean 2 + "hi" or 2
                // + x where we just don't know x yet
                let const_val_opt: Option<Value> = match (lhs_val_opt, rhs_val_opt) {
                    //TODO: Should this try to catch invalid types being used here like functions?
                    //It ignores them right now since they don't have a const val, which just means
                    //the output is `None` rather than an error
                    (Some(lhs_const), Some(rhs_const)) => {
                        let lhs_span = lhs_expr.meta.expect_user();
                        let rhs_span = rhs_expr.meta.expect_user();

                        let sp_lhs_const = SpannedContainerRef::new(lhs_const, lhs_span);
                        let sp_rhs_const = SpannedContainerRef::new(rhs_const, rhs_span);
                        match evaluator::apply_binary_op(
                            sp_lhs_const,
                            *op,
                            sp_rhs_const,
                            self.interner,
                        ) {
                            evaluator::BinaryOpResult::Output(val) => Some(val),
                            evaluator::BinaryOpResult::DivideByZero => {
                                return Err(MathError::DivideByZero { lhs_span, rhs_span }.into());
                            }
                            // If either are unknown then that would mean it can't confidentally
                            // say the resolution failed since neither have definitive values yet.
                            evaluator::BinaryOpResult::Invalid
                                if !lhs_is_unknown && !rhs_is_unknown =>
                            {
                                return Err(MathError::BinaryOpMismatch {
                                    sp_lhs: SpannedContainer::new(lhs_const.kind(), lhs_span),
                                    sp_rhs: SpannedContainer::new(rhs_const.kind(), rhs_span),
                                    op: *op,
                                })?;
                            }
                            _ => None,
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

                let lhs_type_id = self.compiler.exprs[lhs_id].type_id;
                let rhs_type_id = self.compiler.exprs[lhs_id].type_id;
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

                // NOTE: Defer location
                //
                // If a type was inferred then we will use that, otherwise unknown is allocated
                //
                // This is allocated so that it can become `Deferred` where possible
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
                self.compiler.exprs[lhs_id].user = Some(expr_id);
                self.compiler.exprs[rhs_id].user = Some(expr_id);

                // Expression points to the value so the expr_id is returned alone.
                let resolved_expr = ResolvedExpr::new(
                    type_id,
                    expr_hir,
                    val_id,
                    ResolvedExprMetadata::User(spanned_expr.span),
                    vec![lhs_id, rhs_id],
                );

                let val_info = ValueInfo::new(type_id, expr_id, const_val_opt);

                self.compiler.exprs.push(resolved_expr);
                self.compiler.values.push(val_info);

                Ok(expr_id)
            }
            AstExpr::Char(c) => {
                let expr_id = ExprId::new(self.compiler.exprs.len() as u32);
                let val_id = ValueId::new(self.compiler.values.len() as u32);
                let type_id = TypeId::new(compiler_constants::CORE_CHAR);

                let val = Value::Char(*c);
                let val_info = ValueInfo::new(type_id, expr_id, Some(val));
                self.compiler.values.push(val_info);

                let expr_hir = ExprHir::Val(val_id);
                let resolved_expr = ResolvedExpr::new(
                    type_id,
                    expr_hir,
                    val_id,
                    ResolvedExprMetadata::User(spanned_expr.span),
                    Vec::new(),
                );
                self.compiler.exprs.push(resolved_expr);

                Ok(expr_id)
            }
            AstExpr::Default(ident_expr, spanned_expr) => {
                let expr_id = ExprId::new(self.compiler.exprs.len() as u32);
                let val_id = ValueId::new(self.compiler.values.len() as u32);

                //WARN: SUSPICIOUS
                let default_ident_expr_id = self.register_expr(
                    parent_sym_id_opt,
                    &ident_expr,
                    local_scope_id,
                    associated_scope,
                    scope_type,
                    env,
                )?;

                let default_val_expr_id = self.register_expr(
                    parent_sym_id_opt,
                    &spanned_expr,
                    local_scope_id,
                    associated_scope,
                    scope_type,
                    env,
                )?;

                // Need the entire alias to use this as it's type through checks
                let type_id = self.compiler.exprs[default_val_expr_id].type_id;

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
                    ResolvedExprMetadata::User(spanned_expr.span),
                    vec![default_val_expr_id],
                );

                self.compiler.exprs[default_ident_expr_id].user = Some(expr_id);
                self.compiler.exprs[default_val_expr_id].user = Some(expr_id);
                self.compiler.exprs.push(resolved_expr);

                Ok(expr_id)
            }
            AstExpr::Str(name_id) => {
                let expr_id = ExprId::new(self.compiler.exprs.len() as u32);
                let val_id = ValueId::new(self.compiler.values.len() as u32);

                let type_id = TypeId::new(compiler_constants::CORE_STR);

                let val = Value::InternedStr(*name_id);
                let val_info = ValueInfo::new(type_id, expr_id, Some(val));
                self.compiler.values.push(val_info);

                let expr_hir = ExprHir::Val(val_id);
                let resolved_expr = ResolvedExpr::new(
                    type_id,
                    expr_hir,
                    val_id,
                    ResolvedExprMetadata::User(spanned_expr.span),
                    Vec::new(),
                );
                self.compiler.exprs.push(resolved_expr);

                Ok(expr_id)
            }
            AstExpr::Unary(unary) => {
                let operand_id = self.register_expr(
                    parent_sym_id_opt,
                    &unary.spanned_expr,
                    local_scope_id,
                    associated_scope,
                    scope_type,
                    env,
                )?;

                let operand_expr = &self.compiler.exprs[operand_id];

                let is_unknown = self.compiler.check_unknown(operand_expr.type_id);

                let operand_val_opt = &self.compiler.values[operand_expr.val_id];

                let const_val_opt = if let Some(const_val) = &operand_val_opt.const_val {
                    let operand_span = operand_expr.meta.expect_user();

                    let sp_const = SpannedContainerRef::new(const_val, operand_span);
                    match evaluator::apply_unary_op(unary.op, sp_const) {
                        UnaryOpResult::Output(val) => Some(val),
                        UnaryOpResult::Invalid if !is_unknown => {
                            return Err(MathError::UnaryOpMismatch {
                                sp_operand: SpannedContainer::new(const_val.kind(), operand_span),
                                op: unary.op,
                            })?;
                        }
                        _ => None,
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

                //NOTE: Defer location
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
                    ResolvedExprMetadata::User(spanned_expr.span),
                    vec![operand_id],
                );

                self.compiler.exprs.push(resolved_expr);
                self.compiler.exprs[operand_id].user = Some(unary_expr_id);

                let val_info = ValueInfo::new(type_id, unary_expr_id, const_val_opt);
                self.compiler.values.push(val_info);

                Ok(unary_expr_id)
            }
            // What were we doing here?????
            // Also maybe bring back value pre-allocation
            AstExpr::Bool(boolean) => {
                //FIX:
                let type_id = TypeId::new(compiler_constants::CORE_BOOL);

                let expr_id = ExprId::new(self.compiler.exprs.len() as u32);
                let val_id = ValueId::new(self.compiler.values.len() as u32);

                let val = Value::Bool(*boolean);
                let val_info = ValueInfo::new(type_id, expr_id, Some(val));

                let expr_hir = ExprHir::Val(val_id);
                let resolved_expr = ResolvedExpr::new(
                    type_id,
                    expr_hir,
                    val_id,
                    ResolvedExprMetadata::User(spanned_expr.span),
                    vec![],
                );

                self.compiler.exprs.push(resolved_expr);
                self.compiler.values.push(val_info);

                Ok(expr_id)
            }
            AstExpr::Call(caller, arg_exprs) => {
                // The "Call" in "Call(x, y)"
                let caller_id = self.register_expr(
                    parent_sym_id_opt,
                    caller,
                    local_scope_id,
                    associated_scope,
                    scope_type,
                    env,
                )?;
                //WARN: Does this need something?
                let type_id = self.compiler.exprs[caller_id].type_id;
                let mut call_args: Vec<ExprId> = Vec::with_capacity(arg_exprs.len());

                for sp_expr in arg_exprs {
                    let arg = self.register_expr(
                        parent_sym_id_opt,
                        sp_expr,
                        local_scope_id,
                        associated_scope,
                        scope_type,
                        env,
                    )?;

                    call_args.push(arg);
                }

                let expr_id = ExprId::new(self.compiler.exprs.len() as u32);
                let val_id = ValueId::new(self.compiler.values.len() as u32);

                let inputs = call_args.clone();

                let expr_hir = ExprHir::Call(caller_id, call_args);
                // Are the arguments inputs if they are the expression itself?
                let resolved_expr = ResolvedExpr::new(
                    type_id,
                    expr_hir,
                    val_id,
                    ResolvedExprMetadata::User(spanned_expr.span),
                    inputs,
                );
                let val_info = ValueInfo::new(type_id, expr_id, None);

                self.compiler.exprs.push(resolved_expr);
                self.compiler.values.push(val_info);

                Ok(expr_id)
            }
            AstExpr::MemberAccess(abs_member_access) => {
                match self.resolve_member(
                    parent_sym_id_opt,
                    &abs_member_access.base,
                    local_scope_id,
                    associated_scope,
                    scope_type,
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
            AstExpr::StaticAccess(spanned_segments) => {
                let last_scope = resolution_helpers::resolve_static_access_ret_preset(
                    self.compiler,
                    spanned_segments,
                    associated_scope,
                    scope_type,
                    lookup_pref,
                    StaticAccessOption::Val,
                    self.interner,
                    env,
                )?;

                let last_seg = &spanned_segments[spanned_segments.len() - 1];

                // This is a little odd, but it technically isn't different from if it were
                // classified as an expr in the first place. This is done since, making paths take
                // in a generic "Expr" would be an insanely large amount of possibilites for
                // something that is enforced at parse-time, making it more confusing. But,
                // creating inline expressions is also confusing so, not sure.
                let inline_expr = match &last_seg.inner {
                    PathSegment::Ident(interned_id) => {
                        SpannedExpr::new(AstExpr::Var(*interned_id), last_seg.span)
                    }
                    PathSegment::Generic(_) => {
                        let core_msg = "Generics are only usable in type expressions".to_string();
                        let src_diag = SourceDiagnostic::builder(
                            // Maybe make this `None` since this is more so an obvious quick fix error
                            ErrorCode::GenericsErr.into(),
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
                    parent_sym_id_opt,
                    &inline_expr,
                    local_scope_id,
                    last_scope,
                    scope_type,
                    env,
                )
            }
            AstExpr::Array(array_expr) => {
                let mut array: Vec<ExprId> = Vec::with_capacity(array_expr.elements.len());

                let mut found_const_vals = 0;
                let mut type_id_opt = None;

                for sp_expr in &array_expr.elements {
                    // register as inputs?
                    let expr_id = self.register_expr(
                        parent_sym_id_opt,
                        sp_expr,
                        local_scope_id,
                        associated_scope,
                        scope_type,
                        env,
                    )?;

                    let expr = &self.compiler.exprs[expr_id];
                    let val_info = &self.compiler.values[expr.val_id];

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
                    let expr = &mut self.compiler.exprs[*expr_id];
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
                    let mut values: Vec<Value> = Vec::with_capacity(array_expr.elements.len());

                    for expr_id in &array {
                        let expr = &self.compiler.exprs[*expr_id];
                        let val_info = &self.compiler.values[expr.val_id];
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
                    ResolvedExprMetadata::User(spanned_expr.span),
                    inputs,
                );

                self.compiler.values.push(val_info);
                self.compiler.exprs.push(resolved_expr);

                Ok(array_expr_id)
            }
        }
    }

    // Umm...
    fn resolve_member(
        &mut self,
        sym_parent: Option<SymbolId>,
        member: &SpannedExpr,
        local_scope: Option<ScopeId>,
        associated_scope: AssociatedScopeKind,
        scope_type: ScopeType,
        env: &ResolverEnv,
    ) -> Result<PossibleMember, PresetErr> {
        let lookup_pref =
            ScopeLookupPreferenceFlags::new(ScopeLookupPreferenceFlags::VARIABLE.into());
        let res = self.register_expr(
            sym_parent,
            member,
            local_scope,
            associated_scope,
            scope_type,
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
            env,
        ) {
            let resolved_expr = &self.compiler.exprs[expr_id];

            todo!();
        }

        if let AstExpr::Var(name_id) = member.expr {
            if let Some(sym_id) = scopes::find_sym_id(
                self.compiler,
                todo!(),
                name_id,
                scope_type,
                ScopeLookupPattern::NoRestrictions,
                lookup_pref,
            ) {
                todo!();
                // let type_id = self.compiler.symbols[sym_id ];
                // return Ok(PossibleMember::Type(type_id));
            } else {
                let msg = format!(
                    "Could not find `{}` as a module or value",
                    self.interner.search(name_id)
                );

                return Err(PresetErr::General(todo!()));
            }
        }

        Err(PresetErr::UndefinedMember(member.span))
    }

    /// Helper that checks for dependency cycle
    /// * parent_sym_id: The root symbol being evaluated as an expr.
    /// (i.e. "let x = 3 + y" would mean x is the root and y would be `found_sym_id`)
    /// * found_sym_id: A symbol that was found during expression evaluation, which innately is
    /// always a candidate for being circular.
    fn check_cycle(
        &self,
        parent_sym_id: SymbolId,
        found_sym_id: SymbolId,
        env: &ResolverEnv,
    ) -> Result<(), PresetErr> {
        // If `found_sym_id` can reach `parent_sym_id`, adding the new reference
        // would close a cycle.
        let mut stack: Vec<SymbolId> = vec![found_sym_id];
        let mut visited: Vec<SymbolId> = Vec::new();

        while let Some(current) = stack.pop() {
            // Directly checking if a cycle was had
            if current == parent_sym_id {
                let current_sym = &self.compiler.symbols[parent_sym_id];
                let current_name = self.interner.search(current_sym.name_id);
                let current_ast_id = current_sym.ast_id.expect("Should be user symbols only");

                let cycled_sym = &self.compiler.symbols[found_sym_id];
                let cycled_ast_id = cycled_sym.ast_id.expect("Should be user symbols only");
                let cycled_name = self.interner.search(cycled_sym.name_id);

                let cycled_span = env.ast_info.get_name_span(cycled_ast_id);
                let current_span = env.ast_info.get_name_span(current_ast_id);

                let core_msg = format!(
                    "`{}` depends on itself through `{}`",
                    current_name, cycled_name
                );

                let src_diag = SourceDiagnostic::builder(
                    None,
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

            if visited.contains(&current) {
                continue;
            }
            visited.push(current);

            // Find every symbol that `current` depends on by scanning the pending queue.
            for (sym_id, pending_sym) in &self.ty_ctx.sym_queue {
                for pending_expr in &pending_sym.pending_exprs {
                    if let PendingExprKind::Parent(parent_base) = &pending_expr.kind {
                        if parent_base.parent_sym_id == current {
                            stack.push(*sym_id);
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Unknown or invalid directives produce a `PresetErr` which is returned alongside any
    /// successfully resolved directive ids.
    ///
    /// This is a helper.
    fn handle_directives(
        &self,
        abs_directives: &[AbstractDirective],
        env: &ResolverEnv,
    ) -> (Vec<SpannedContainer<DirectiveId>>, Vec<PresetErr>) {
        let mut directive_ids = Vec::with_capacity(abs_directives.len());
        let mut preset_errs = Vec::new();

        for abs_directive in abs_directives {
            match resolution::resolve_directive(abs_directive) {
                Some(dir) => {
                    let directive_id = compiler_constants::directive_to_id(&dir);
                    let sp_directive_id =
                        SpannedContainer::new(directive_id, abs_directive.sp_name_id.span);
                    directive_ids.push(sp_directive_id);
                }
                None => {
                    let preset_err = PresetErr::UnknownDirective(abs_directive.sp_name_id.clone());
                    preset_errs.push(preset_err);
                }
            }
        }

        (directive_ids, preset_errs)
    }
}
