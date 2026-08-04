mod engine;
mod engine_concepts;
pub mod preset_err;
mod static_enricher;

use crate::lookup::scopes::AssociatedScopeKind;
use crate::resolvers::resolver_env::ResolverEnv;
use crate::script_compiler::ScriptCompiler;
use crate::semantic::preset_reporter::engine_concepts::{
    AvailableKind, EngineOption, EngineOptionBase, ListAvailable,
};
use crate::semantic::preset_reporter::preset_err::{LookupError, MathError, PresetErr};
use crate::semantic::resolve::{StaticAccessResult, TypeExprResult};
use chrn_utils::chrn_config::ChrnConfig;
use chrn_utils::err_codes::ErrorCode;
use chrn_utils::s_suffix;
use chrn_utils::source_map::source_diagnostic::annotations::AnnotationKind;
use chrn_utils::source_map::source_diagnostic::{
    DiagnosticLevel, SourceDiagnosticBuilder, SourceDiagnosticSink,
};
use chrn_utils::{
    intern::Intern,
    source_map::{source_diagnostic::SourceDiagnostic, source_region::SourceRegion},
};
use lang::fmter::Formattable;

// These take ownership because `PresetErr::General` will clone otherwise, which isn't expensive.
// Ok maybe this should just be a reference.

/// Convenience function that creates a `SourceDiagnostic` from the given `preset_err` and pushes
/// into `diags`
pub(crate) fn report_preset<S: SourceDiagnosticSink>(
    // If we want a scenario where it doesn't require diagnostics, then we should make a trait where
    // the summary can choose to abide by summary rules.
    compiler: &ScriptCompiler,
    sink: &mut S,
    preset_err: PresetErr,
    region: &SourceRegion,
    cfg: &ChrnConfig,
    interner: &Intern,
) {
    let diag_builder = create_diag_builder_preset(compiler, preset_err, region, cfg, interner);
    sink.push_diagnostic(diag_builder.build());
}

// The s is like that on purpose
/// Takes an array of directives and evaluates as many as possible.
///
/// If any of the directives given are invalid, they will be skipped, and a diagnostic will be
/// created.
///
/// Returns a tuple of any directives and diagnostics found
pub(crate) fn report_preset_vec<S: SourceDiagnosticSink>(
    compiler: &ScriptCompiler,
    sink: &mut S,
    preset_errs: Vec<PresetErr>,
    region: &SourceRegion,
    cfg: &ChrnConfig,
    interner: &Intern,
) {
    for preset in preset_errs {
        let diag_builder = create_diag_builder_preset(compiler, preset, region, cfg, interner);
        sink.push_diagnostic(diag_builder.build());
    }
}

/// Creates `SourceDiagnostic` with the preset associated with it's `SemanticError`
pub(crate) fn create_diag_builder_preset(
    compiler: &ScriptCompiler,
    preset_err: PresetErr,
    region: &SourceRegion,
    cfg: &ChrnConfig,
    interner: &Intern,
) -> SourceDiagnosticBuilder {
    match preset_err {
        // Need to know which spans exactly now
        PresetErr::UnsupportedDirective {
            sp_directive,
            sym_span,
        } => {
            let directive_boundaries = sp_directive.inner.boundaries().to_fmt_vec();
            let mut boundaries_str = String::new();

            for (i, constraint) in directive_boundaries.iter().enumerate() {
                boundaries_str.push_str(&format!("`{}`", constraint));

                if i + 1 < directive_boundaries.len() {
                    boundaries_str.push_str(", ");
                }
            }

            let core_msg = format!(
                "Only types that satisfy {} can use the directive `#{}`",
                boundaries_str,
                sp_directive.inner.to_fmt()
            );

            SourceDiagnostic::builder(
                ErrorCode::DirectiveErr.into(),
                DiagnosticLevel::Error,
                core_msg,
                region.path_id,
            )
            .add_annotation(
                sp_directive.span,
                AnnotationKind::Secondary,
                "Required by this directive".to_string().into(),
            )
            .add_annotation(sym_span, AnnotationKind::Primary, None)
        }
        PresetErr::UnknownDirective(sp_interned_id) => {
            let err_name = interner.search(sp_interned_id.inner);
            let core_msg = format!("Unknown directive `#{err_name}`");

            let mut builder = SourceDiagnostic::builder(
                ErrorCode::DirectiveErr.into(),
                DiagnosticLevel::Error,
                core_msg,
                region.path_id,
            )
            .add_annotation(sp_interned_id.span, AnnotationKind::Primary, None);

            let similar_vec =
                lang::algo::fuzzy_match(err_name.as_bytes(), lang::algo::FuzzyMatch::Directive);

            //FIX: Every usage of this is very dis-organized
            if !similar_vec.is_empty() {
                let s_suffix = s_suffix!(similar_vec.len());
                let mut help = format!("Similar directive{s_suffix}: ");
                for (i, similar) in similar_vec.iter().enumerate() {
                    help.push_str(&format!("`{similar}`"));
                    if i + 1 != similar_vec.len() {
                        help.push_str(&format!(", "));
                    }
                }

                builder = builder.add_help(help);
            }

            builder
        }
        PresetErr::VagueDirective(sp_directive) => {
            let core_msg = format!(
                //FIXME: Still vague
                "\"#{}\" cannot be used for a `var->` defined variable that holds a `struct` or `enum`",
                sp_directive.inner.to_fmt()
            );

            SourceDiagnostic::builder(ErrorCode::DirectiveErr.into(), DiagnosticLevel::Error, core_msg,  region.path_id)
                    .add_annotation(sp_directive.span, AnnotationKind::Primary, None)
                    .add_note("This is not allowed since it would overlap with any specifics directives given to a defined type from `nest->`")
        }
        PresetErr::FuncConstraintMismatch {
            constraint,
            fmtted_ty,
            spans,
        } => {
            todo!();
            // let msg =
            //     format!("The type `{type_kind}` does not satisfy constraint `{constraint}`",);
            //
            // SourceDiagnostic::builder(DiagnosticLevel::Error, core_msg)
            //     .add_annotation(sp_directive.span, AnnotationKind::Primary, None)
            //     .build()
        }
        PresetErr::DirectiveCountMismatch {
            constraint: _,
            count: _,
            spans: _,
        } => {
            todo!();
            // let msg = format!("Expected {constraint}, found {count}");
            //
            // (msg, spans)
        }
        PresetErr::CircularDirective {
            sp_fmtted_parent,
            sp_directive,
            err_ty_span,
        } => {
            let core_msg = format!(
                // Suspicious error message
                "`#{}` cannot be applied to recursive types",
                sp_directive.inner.to_fmt()
            );

            SourceDiagnostic::builder(
                ErrorCode::DirectiveErr.into(),
                DiagnosticLevel::Error,
                core_msg,
                region.path_id,
            )
            .add_annotation(
                sp_fmtted_parent.span,
                AnnotationKind::Secondary,
                format!("{} defined here", sp_fmtted_parent.inner).into(),
            )
            .add_annotation(
                err_ty_span,
                AnnotationKind::Secondary,
                "Recursive".to_string().into(),
            )
            .add_annotation(
                sp_directive.span,
                AnnotationKind::Primary,
                "Conflicting directive here".to_string().into(),
            )
            // I feel like a note should be here though
            //
            // This is a little hard to do since now all circulars. maybe this should be inline then
            // .add_help(format!("Either `#{}` needs to be removed or `{}` needs to get rid of it's recursive field"))
        }
        // Should have the data type's cap shown as well
        //TODO: This shouldn't exist since it should use big int/float internally
        PresetErr::NumericOverflow { sp_num, fmtted_ty } => {
            let overflown_num = interner.search(sp_num.inner);
            let core_msg = format!(
                "The type `{fmtted_ty}` had an overflow with the value \"{}\" ",
                overflown_num
            );

            SourceDiagnostic::builder(None, DiagnosticLevel::Error, core_msg, region.path_id)
                .add_annotation(sp_num.span, AnnotationKind::Primary, None)
        }
        PresetErr::General(src_diag) => src_diag,
        PresetErr::Lookup(lookup_err) => match lookup_err {
            LookupError::ImpossibleTypeMemberAccess(sp_fmtted_ty) => {
                let core_msg = format!(
                    "Type `{}` does not have the ability to hold members",
                    sp_fmtted_ty.inner,
                );

                SourceDiagnostic::builder(None, DiagnosticLevel::Error, core_msg, region.path_id)
                    .add_annotation(sp_fmtted_ty.span, AnnotationKind::Primary, None)
            }
            LookupError::MemberNotFound {
                parent_type_id,
                sp_parent_name_id,
                nonexistent_member: member,
            } => {
                let ty_name = interner.search(sp_parent_name_id.inner);
                let member_name = interner.search(member);
                let core_msg = format!("No member `{member_name}` in type `{ty_name}`");

                let builder = SourceDiagnostic::builder(
                    None,
                    DiagnosticLevel::Error,
                    core_msg,
                    region.path_id,
                )
                .add_annotation(
                    sp_parent_name_id.span,
                    AnnotationKind::Primary,
                    format!("Is type `{ty_name}`").into(),
                );

                let list_opt = EngineOptionBase::builder(EngineOption::ListAvailable(
                    ListAvailable::new(parent_type_id, AvailableKind::Member),
                ))
                .build();

                let opts = &[list_opt];

                engine::enrich(compiler, interner, builder, opts)
            }
            LookupError::InvalidSymbolMemberAccess(sp_sym) => {
                let core_msg = format!("Symbol `{}` cannot use member access", sp_sym.inner);
                SourceDiagnostic::builder(None, DiagnosticLevel::Error, core_msg, region.path_id)
                    .add_annotation(sp_sym.span, AnnotationKind::Primary, None)
            }
        },
        PresetErr::Math(math_err) => match math_err {
            MathError::BinaryOpMismatch { sp_lhs, sp_rhs, op } => {
                let core_msg = format!(
                    "Type `{}` cannot apply `{}` to type `{}`",
                    sp_lhs.inner.to_fmt(),
                    op.to_fmt(),
                    sp_rhs.inner.to_fmt(),
                );

                let mut builder = SourceDiagnostic::builder(
                    None,
                    DiagnosticLevel::Error,
                    core_msg,
                    region.path_id,
                )
                .add_annotation(sp_lhs.span, AnnotationKind::Primary, None)
                .add_annotation(sp_rhs.span, AnnotationKind::Primary, None);

                static_enricher::enrich_binary_op(builder, sp_lhs.inner, op, sp_rhs.inner)
            }
            MathError::UnaryOpMismatch { sp_operand, op } => {
                let core_msg = format!(
                    "Cannot apply `{}` to type `{}`",
                    op.to_fmt(),
                    sp_operand.inner.to_fmt()
                );

                let builder = SourceDiagnostic::builder(
                    None,
                    DiagnosticLevel::Error,
                    core_msg,
                    region.path_id,
                )
                .add_annotation(sp_operand.span, AnnotationKind::Primary, None);
                static_enricher::enrich_unary_op(builder, op, sp_operand.inner)
            }
            //TODO: Link to "ADDRESS ME" elephant
            MathError::DivideByZero { lhs_span, rhs_span } => {
                let core_msg = format!("Cannot divide by zero");

                SourceDiagnostic::builder(None, DiagnosticLevel::Error, core_msg, region.path_id)
                    .add_annotation(lhs_span, AnnotationKind::Primary, None)
                    .add_annotation(rhs_span, AnnotationKind::Primary, None)
            }
        },
        // Maybe a type version too?
        PresetErr::TypeBoundaryMismatch {
            given_constraints,
            found_ty,
            spans: _,
        } => {
            let given_vec = given_constraints.to_fmt_vec();
            let mut given_str = String::new();

            for (i, constraint) in given_vec.iter().enumerate() {
                given_str.push_str(&format!("`{}`", constraint));

                if i + 1 < given_vec.len() {
                    given_str.push_str(", ");
                }
            }

            let msg = format!("`{}` does not satisfy constraint {}", found_ty, given_str);

            todo!();
        }
        //TODO: Not done
        PresetErr::UndefinedMember(span) => {
            let core_msg = format!("Cannot infer member access");
            SourceDiagnostic::builder(None, DiagnosticLevel::Error, core_msg, region.path_id)
                .add_annotation(
                    span,
                    AnnotationKind::Primary,
                    "Nothing is inferred to match this member"
                        .to_string()
                        .into(),
                )
        }
        PresetErr::TypeBoundaryBoundConflict {
            inferred: current_inferred,
            conflicting: conflicting_inferred,
            spans: _,
        } => {
            //FIXME: Fear inducing message.
            let current_bounds = current_inferred.to_fmt_vec();
            let conflicting_bounds = conflicting_inferred.to_fmt_vec();

            let mut current_msg = String::new();

            if current_bounds.len() > 1 {
                current_msg.push_str("constraints ");
            } else {
                current_msg.push_str("constraint ");
            }

            for (i, bound) in current_bounds.iter().enumerate() {
                current_msg.push_str(&format!("`{}`", bound));
                if i + 1 < current_bounds.len() {
                    current_msg.push_str(" + ");
                }
            }

            let mut conflicting_msg = String::new();
            for (i, bound) in conflicting_bounds.iter().enumerate() {
                conflicting_msg.push_str(&format!("`{}`", bound));
                if i + 1 < conflicting_bounds.len() {
                    conflicting_msg.push_str(" + ");
                }
            }

            let msg = format!(
                "Inferred {current_msg} conflicts with another expression's constraints of {conflicting_msg}"
            );

            todo!()
        }
    }
}

//TODO: Needs many changes
// fn try_help(&self, err_name: &str) -> Option<String> {
//     let found_kw = algo::fuzzy_match(err_name.as_bytes(), algo::FuzzyMatch::KW)?;
//
//     let msg = format!("Found similar keyword \"{}\"", found_kw);
//
//     let help = reporter::standardize_help(&msg,  settings.can_color);
//
//     Some(help)
// }

//TODO: Make enums first some of these so steps can be ran to check compiler internals procedurely
//like finding similar
/// Convenience function to create general errors associated with a `TypeExprResult`
pub fn type_expr_result_to_preset_err(
    compiler: &ScriptCompiler,
    interner: &Intern,
    res: &TypeExprResult,
    env: &ResolverEnv,
    // Should this just return the builder?
) -> Option<PresetErr> {
    match res {
        TypeExprResult::Type(_) => None,
        TypeExprResult::NotAType {
            sp_name_id, kind, ..
        } => {
            let name = interner.search(sp_name_id.inner);
            //WARN: I don't know about this msg
            let core_msg = format!("`{name}` is a {kind} not a type");

            let src_diag = SourceDiagnostic::basic_builder(
                None,
                DiagnosticLevel::Error,
                core_msg,
                env.region.path_id,
                sp_name_id.span,
            );

            Some(PresetErr::General(src_diag))
        }
        TypeExprResult::SymbolNotFound(sp_name_id, associated) => {
            let err_name = interner.search(sp_name_id.inner);
            let core_msg = match associated {
                AssociatedScopeKind::Module(mod_id) => {
                    let err_mod = &compiler.mods[*mod_id];
                    let err_mod_name = interner.search(err_mod.name_id);

                    //WARN: If we have "var State {}" in complex or override, and it's not in that
                    //scope solely, this will say that the ENTIRE module does not contain the given
                    //type, but that's not true. Maybe we need a little more info given or at least
                    //acknkowledgement of the scope question mark.
                    format!("No type `{err_name}` defined in module `{err_mod_name}`")
                }
                //NOTE: Not current symbol exists that has it's own scope except modules
                AssociatedScopeKind::Scope(scope_id) => {
                    let scope_info = &compiler.scopes[*scope_id];

                    // Expects since if the current associated scope is from a symbol, that means
                    // the previous stack frame was extracted from a symbol's namespace directly
                    let sym_owner = scope_info
                        .sym_owner
                        .expect("resolve_type_expr control flow broke");

                    let sym_name_id = compiler.symbols[sym_owner].name_id;
                    let sym_name = interner.search(sym_name_id);

                    format!("Namspace `{sym_name}` does not contain `{err_name}`")
                }
            };

            let src_diag = SourceDiagnostic::basic_builder(
                None,
                DiagnosticLevel::Error,
                core_msg,
                env.region.path_id,
                sp_name_id.span,
            );

            Some(PresetErr::General(src_diag))
        }
        TypeExprResult::PrivateTypeAccess {
            found_sym_id,
            current_mod,
            ty_expr_span,
            ..
        } => {
            // Um...
            let current_mod = &compiler.mods[*current_mod];
            let current_mod_name = interner.search(current_mod.name_id);

            let sym = &compiler.symbols[*found_sym_id];
            let sym_name = interner.search(sym.name_id);

            let core_msg = format!("Type `{sym_name}` is private in module `{current_mod_name}`");

            let src_diag = SourceDiagnostic::builder(
                ErrorCode::PrivacyErr.into(),
                DiagnosticLevel::Error,
                core_msg,
                env.region.path_id,
            )
            .add_annotation(*ty_expr_span, AnnotationKind::Primary, None)
            // Redundant?
            .add_help(format!(
                "Consider using `export` on `{sym_name}` if that was intended"
            ));

            Some(PresetErr::General(src_diag))
        }
        TypeExprResult::InvalidGenericArgCount {
            // Could make this kind specific but $#)%@^*)
            base,
            expected,
            inputs_span,
        } => {
            // The name based if confusing
            let name = interner.search(*base);
            // BRING S_IFIER IN HERE NOW
            let core_msg = format!("`{name}` expects {expected} input(s)");

            let src_diag = SourceDiagnostic::builder(
                ErrorCode::GenericsErr.into(),
                DiagnosticLevel::Error,
                core_msg,
                env.region.path_id,
            )
            .add_annotation(*inputs_span, AnnotationKind::Primary, None);

            Some(PresetErr::General(src_diag))
        }
        TypeExprResult::UnknownGenericIdent(sp_name_id) => {
            let name = interner.search(sp_name_id.inner);
            let core_msg = format!("Unknown generic identifier `{name}`");

            let src_diag = SourceDiagnostic::builder(
                ErrorCode::GenericsErr.into(),
                DiagnosticLevel::Error,
                core_msg,
                env.region.path_id,
            )
            .add_annotation(sp_name_id.span, AnnotationKind::Primary, None)
            // Redundant?
            .add_help(format!(
                "Only the data structures `List`, `Set`, `Map` and `Tuple` exist"
            ));

            Some(PresetErr::General(src_diag))
        }
        TypeExprResult::StaticAccessFailure(static_access_res) => {
            static_access_result_to_preset_err(interner, &static_access_res, env)
        }
    }
}

/// Convenience function to create general errors associated with a `StaticAccessResult`
pub fn static_access_result_to_preset_err(
    interner: &Intern,
    res: &StaticAccessResult,
    env: &ResolverEnv,
) -> Option<PresetErr> {
    match res {
        StaticAccessResult::Scope(_) => None,
        StaticAccessResult::SymNotFound {
            current_seg,
            prev_seg,
        } => {
            let current_seg_name = interner.search(current_seg.inner);
            let src_diag = if let Some(prev) = prev_seg {
                let prev_seg_name = interner.search(prev.inner);
                let core_msg = format!(
                    "Could not find `{}` in namespace `{}`",
                    current_seg_name, prev_seg_name
                );

                SourceDiagnostic::builder(
                    ErrorCode::ScopeErr.into(),
                    DiagnosticLevel::Error,
                    core_msg,
                    env.region.path_id,
                )
                .add_annotation(current_seg.span, AnnotationKind::Primary, None)
            } else {
                let core_msg = format!("Could not find namespace `{current_seg_name}`");

                SourceDiagnostic::builder(
                    ErrorCode::ScopeErr.into(),
                    DiagnosticLevel::Error,
                    core_msg,
                    env.region.path_id,
                )
                .add_annotation(current_seg.span, AnnotationKind::Primary, None)
            };

            Some(PresetErr::General(src_diag))
        }
        StaticAccessResult::NoNamespace(sp_name_id) => {
            let namespace_name = interner.search(sp_name_id.inner);
            let core_msg = format!("No namespace found in `{namespace_name}`");

            let src_diag = SourceDiagnostic::builder(
                ErrorCode::ScopeErr.into(),
                DiagnosticLevel::Error,
                core_msg,
                env.region.path_id,
            )
            .add_annotation(sp_name_id.span, AnnotationKind::Primary, None);

            Some(PresetErr::General(src_diag))
        }
        StaticAccessResult::GenericUsingStaticPath(generic_span) => {
            let core_msg = "Generics cannot contain namespaces".to_string();
            let src_diag = SourceDiagnostic::basic_builder(
                ErrorCode::GenericsErr.into(),
                DiagnosticLevel::Error,
                core_msg,
                env.region.path_id,
                *generic_span,
            );

            Some(PresetErr::General(src_diag))
        } // StaticAccessResult::GenericInExpr(generic_span) => {
          //     unreachable!("Isn't this impossible?");
          //     let core_msg = "Generics cannot be used inside of expressions".to_string();
          //     let src_diag = SourceDiagnostic::basic_builder(
          //         DiagnosticLevel::Error,
          //         core_msg,
          //         env.region.path_id,
          //         *generic_span,
          //     );
          //     todo!("TEST THIS");
          //
          //     Some(PresetErr::General(src_diag))
          // }
    }
}
