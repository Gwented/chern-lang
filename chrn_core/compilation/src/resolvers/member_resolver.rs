//TEST:
//Seeing if members having a specific stage will help reduce the complexity of type resolution
//which is stacking infinitely (Infinitely as in the infinite sign here -> 🍔)

use chrn_utils::{
    chrn_config::ChrnConfig,
    id_types::{AstId, InternedId, MemberId, SymbolId, TypeId},
    intern::Intern,
    source_map::source_diagnostic::{
        DiagnosticLevel, SourceDiagnostic, SourceDiagnosticSink, SourceDiagnosticSummary,
        annotations::AnnotationKind,
    },
    utils::containers::SpannedContainer,
};
use lang::chrn_classifier::ChrnClassifier;

use crate::{
    lookup::scopes::scopes_concepts::{AssociatedScopeKind, ScopeLookupPattern, ScopeType},
    resolvers::{resolver_env::ResolverEnv, resolver_state::ResolverState, typechecker},
    script_compiler::{self, ScriptCompiler},
    semantic::{
        checker_helpers::DuplicateTracker,
        compilation_unit::CompilationUnit,
        hir::{
            hir_concepts::Type,
            hir_symbols::{FieldRepre, MemberSymbolKind, SymbolKind, VariantRepre},
        },
        preset_reporter::{self, preset_err::PresetErr},
        resolution::{self, resolution_concepts::TypeExprResult},
    },
};

// This doesn't account for general things it could do like directives since that would make it
// confusing and turn into a situation where SOME directives are appended, but SOME are ignored.
//
// It also doesn't iterate through all to achieve this because, there actually isn't a valid reason
// yet this would be fine.
/// Resolves Fields/Variants
///
/// Intended to allow for future stages to assume all inner parts of data have been processed.
pub struct MemberResolver<'a> {
    cfg: &'a ChrnConfig,
    interner: &'a Intern,
    compiler: &'a mut ScriptCompiler,
    summary: SourceDiagnosticSummary,
}

impl MemberResolver<'_> {
    /// Instantiation requires that the compiler's state is valid and will panic otherwise
    pub fn new<'a>(
        cfg: &'a ChrnConfig,
        interner: &'a Intern,
        compiler: &'a mut ScriptCompiler,
    ) -> MemberResolver<'a> {
        debug_assert_eq!(ResolverState::MEMBER, compiler.resolver_state);
        compiler.resolver_state.advance();
        MemberResolver {
            cfg,
            interner,
            compiler,
            summary: SourceDiagnosticSummary::default(),
        }
    }

    // Considering less general type arenas
    //
    // Maybe it's not a show of flaws since these checks are technically actual questions the
    // compiler has, should this be cached?
    //
    // Should this be incremental module-wise?
    /// Goes through all types within `self.compiler` and appends fields/variants where possible,
    /// only resolving their types
    ///
    /// If diagnostics > 0 then an error occured
    // Would options be ok here?
    pub fn resolve(&mut self, env: &ResolverEnv) -> SourceDiagnosticSummary {
        // Re-used hashet when identifiers are checked for members.
        let mut ident_tracker: DuplicateTracker<SpannedContainer<InternedId>> =
            DuplicateTracker::with_capacities(4, 4);

        // Goes through all symbols the current module has and only picks structs and enums to
        // append to.
        for comp_unit in env.compilation_syms.iter().cloned() {
            match comp_unit {
                CompilationUnit::Symbol(sym_id) => {
                    match self.compiler.symbols[sym_id].kind {
                        // This split is more so, users can define these set of symbols, and users cannot
                        // define the unreacables.
                        SymbolKind::Type(type_id) => match &self.compiler.types[type_id].ty {
                            Type::Struct(_) => self.resolve_struct(sym_id, &mut ident_tracker, env),
                            Type::Enum(_) => self.resolve_enum(sym_id, &mut ident_tracker, env),
                            _ => (),
                        },
                        // Still uses sym id since their actual ids make it a little more complicated to get
                        // to their ast id
                        // Users cannot define these but they exist internally.
                        _ => (),
                    }
                }
                CompilationUnit::Impl(_) => (),
            }
            ident_tracker.clear();
        }

        let mut summary = SourceDiagnosticSummary::default();
        summary.append_summary(&mut self.summary);
        summary
    }

    fn resolve_struct(
        &mut self,
        parent_sym_id: SymbolId,
        ident_tracker: &mut DuplicateTracker<SpannedContainer<InternedId>>,
        env: &ResolverEnv,
    ) {
        let ast_id = self.compiler.symbols[parent_sym_id]
            .ast_id
            .expect("Should be user symbols only");
        let abs_struct = env.ast_info.get_struct(ast_id);
        let associated_scope = AssociatedScopeKind::Module(env.current_mod);

        let mut fields: Vec<MemberId> = Vec::with_capacity(abs_struct.fields.len());

        //TODO: global condition and argument setting.
        //field arg and cond settings.
        //same for enums.

        // Checking if there are duplicate name ids within the same struct along with resolution
        for field_typedef in &abs_struct.fields {
            let type_id = match resolution::resolve_type_expr(
                self.compiler,
                associated_scope,
                &field_typedef.sp_ty_expr,
                ScopeType::Nest,
                ScopeLookupPattern::NoRestrictions,
                env,
            ) {
                TypeExprResult::Type(type_id) => {
                    if !typechecker::check_field_or_variant(&self.compiler.types, type_id) {
                        let fmtted_ty = Type::to_fmt(&self.compiler.types, type_id);
                        let core_msg = format!("Cannot use type `{fmtted_ty}` for a field");

                        let builder = SourceDiagnostic::builder(
                            None,
                            DiagnosticLevel::Error,
                            core_msg,
                            env.region.path_id,
                        )
                        .add_annotation(
                            field_typedef.sp_ty_expr.span,
                            AnnotationKind::Primary,
                            None,
                        );

                        self.summary.push_diag(builder.build());
                        TypeId::new(script_compiler::CORE_UNKNOWN)
                    } else {
                        type_id.into()
                    }
                }
                res => {
                    let preset_err = preset_reporter::type_expr_result_to_preset_err(
                        &self.compiler,
                        self.interner,
                        &res,
                        env,
                    )
                    .expect("Result enforced by `match`");

                    // `another` failed here
                    preset_reporter::report_preset(
                        &self.compiler,
                        &mut self.summary,
                        preset_err,
                        env.region,
                        self.cfg,
                        self.interner,
                    );

                    //NOTE: If this weren't done then it would ruin the alignment of fields with future
                    // ast to field alignment related checks. This could be circumvented eventually
                    // by making the ast flat so that it carries a member id to an ast member, which
                    // would never cause an issue here since it doesn't have to depend on an inner
                    // part hopefully existing in an ast.
                    TypeId::new(script_compiler::CORE_UNKNOWN)
                }
            };

            let sp_name_id = SpannedContainer::new(field_typedef.name_id, field_typedef.name_span);
            ident_tracker.insert_or_store(sp_name_id);

            let member_id = MemberId::new(self.compiler.sym_members.len() as u32);

            // Attempts to get a more accurate parent symbol location, this is not semantically required
            // anywhere. The idea behind this is that say, we had:
            //
            // struct Person {
            //      state: State
            // }
            //
            // If an error occurred at state, and a diagnostic wanted a look at where it's actual
            // original field declaration is, it would instead get "Person" which is NOT the
            // declaration location, but just the spot where the particular member was used.

            //TODO: The stored parent symbol id does in fact point to the nearest root basically,
            //but it probably still should hold it's local parent
            let field = FieldRepre::new(
                parent_sym_id,
                member_id,
                field_typedef.name_id,
                field_typedef.name_span,
                type_id,
            );

            self.compiler
                .sym_members
                .push(MemberSymbolKind::Field(field));
            fields.push(member_id);
        }

        for found in ident_tracker.found_dups.drain(..) {
            let preset_err = PresetErr::DuplicateIdents {
                sp_original: found.original,
                sp_dup: found.dup,
                classifier: ChrnClassifier::Field,
            };

            let builder = preset_reporter::create_diag_builder_preset(
                self.compiler,
                preset_err,
                env.region,
                self.cfg,
                self.interner,
            )
            .add_annotation(
                abs_struct.name_span,
                AnnotationKind::Secondary,
                "Found inside this struct".to_string().into(),
            );
            self.summary.push_diag(builder.build());
        }

        let struct_def = self.compiler.get_struct_mut(parent_sym_id);
        debug_assert_eq!(struct_def.fields.len(), 0);
        struct_def.fields.append(&mut fields);
    }

    fn resolve_enum(
        &mut self,
        parent_sym_id: SymbolId,
        ident_tracker: &mut DuplicateTracker<SpannedContainer<InternedId>>,
        env: &ResolverEnv,
    ) {
        let ast_id = self.compiler.symbols[parent_sym_id]
            .ast_id
            .expect("Should be user symbols only");
        let abs_enum = env.ast_info.get_enum(ast_id);

        let mut variants: Vec<MemberId> = Vec::with_capacity(abs_enum.variants.len());

        let associated_scope = AssociatedScopeKind::Module(env.current_mod);

        // Checking if there are duplicate name ids within the same enum
        for (i, variant) in abs_enum.variants.iter().enumerate() {
            let sp_name_id = SpannedContainer::new(variant.name_id, variant.name_span);
            ident_tracker.insert_or_store(sp_name_id);

            let member_id = MemberId::new(self.compiler.sym_members.len() as u32);
            let variant_repre = if let Some(sp_ty_expr) = &variant.sp_ty_expr {
                let type_id = match resolution::resolve_type_expr(
                    self.compiler,
                    associated_scope,
                    &sp_ty_expr,
                    ScopeType::Nest,
                    ScopeLookupPattern::NoRestrictions,
                    env,
                ) {
                    TypeExprResult::Type(type_id) => {
                        if !typechecker::check_field_or_variant(&self.compiler.types, type_id) {
                            let fmtted_ty = Type::to_fmt(&self.compiler.types, type_id);
                            let core_msg = format!("Cannot use type `{fmtted_ty}` for a variant");

                            let builder = SourceDiagnostic::builder(
                                None,
                                DiagnosticLevel::Error,
                                core_msg,
                                env.region.path_id,
                            )
                            .add_annotation(
                                sp_ty_expr.span,
                                AnnotationKind::Primary,
                                None,
                            );
                            self.summary.push_diag(builder.build());
                            TypeId::new(script_compiler::CORE_UNKNOWN)
                        } else {
                            type_id.into()
                        }
                    }
                    res => {
                        let preset_err = preset_reporter::type_expr_result_to_preset_err(
                            &self.compiler,
                            self.interner,
                            &res,
                            env,
                        )
                        .expect("Result enforced by `match`");

                        preset_reporter::report_preset(
                            &self.compiler,
                            &mut self.summary,
                            preset_err,
                            env.region,
                            self.cfg,
                            self.interner,
                        );

                        TypeId::new(script_compiler::CORE_UNKNOWN)
                    }
                };

                VariantRepre::new(
                    parent_sym_id,
                    member_id,
                    variant.name_id,
                    variant.name_span,
                    Some(type_id),
                    AstId::new(i as u32),
                )
                // No type case
            } else {
                VariantRepre::new(
                    parent_sym_id,
                    member_id,
                    variant.name_id,
                    variant.name_span,
                    None,
                    AstId::new(i as u32),
                )
            };

            self.compiler
                .sym_members
                .push(MemberSymbolKind::Variant(variant_repre));

            variants.push(member_id);
        }

        for found in ident_tracker.found_dups.drain(..) {
            let preset_err = PresetErr::DuplicateIdents {
                sp_original: found.original,
                sp_dup: found.dup,
                classifier: ChrnClassifier::Variant,
            };

            let builder = preset_reporter::create_diag_builder_preset(
                self.compiler,
                preset_err,
                env.region,
                self.cfg,
                self.interner,
            )
            .add_annotation(
                abs_enum.name_span,
                AnnotationKind::Secondary,
                "Found inside this enum".to_string().into(),
            );
            self.summary.push_diag(builder.build());
        }

        let enum_def = self.compiler.get_enum_mut(parent_sym_id);
        debug_assert_eq!(enum_def.variants.len(), 0);
        enum_def.variants.append(&mut variants);
    }
}
