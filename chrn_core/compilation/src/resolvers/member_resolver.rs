//TEST:
//Seeing if members having a specific stage will help reduce the complexity of type resolution
//which is stacking infinitely (Infinitely as in the infinite sign here -> 🍔)

use chrn_utils::{
    chrn_config::ChrnConfig,
    id_types::{AstId, MemberId, SymbolId, TypeId},
    intern::Intern,
    source_map::source_diagnostic::{
        DiagnosticLevel, SourceDiagnostic, SourceDiagnosticSummary, annotations::AnnotationKind,
    },
};

use crate::{
    lookup::scopes::{AssociatedScopeKind, ScopeLookupPattern, ScopeType},
    parser::ast::ast_concepts::{AbstractTypeDef, AbstractVariant},
    resolvers::{resolver_env::ResolverEnv, resolver_state::ResolverState},
    script_compiler::{self, ScriptCompiler},
    semantic::{
        hir::hir_concepts::{FieldRepre, MemberSymbolKind, SymbolKind, Type, VariantRepre},
        preset_reporter,
        resolve::{self, TypeExprResult},
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
    settings: &'a ChrnConfig,
    interner: &'a Intern,
    compiler: &'a mut ScriptCompiler,
    summary: SourceDiagnosticSummary,
}

impl MemberResolver<'_> {
    /// Instantiation requires that the compiler's state is valid and will panic otherwise
    pub fn new<'a>(
        settings: &'a ChrnConfig,
        interner: &'a Intern,
        compiler: &'a mut ScriptCompiler,
    ) -> MemberResolver<'a> {
        debug_assert_eq!(ResolverState::MEMBER, compiler.resolver_state);
        compiler.resolver_state.advance();
        MemberResolver {
            settings,
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
        // Goes through all symbols the current module has and only picks structs and enums to
        // append to.
        for sym_id in env.compilation_syms {
            match self.compiler.symbols[*sym_id].kind {
                SymbolKind::Type(type_id) => match self.compiler.types[type_id].ty {
                    Type::Struct(_) => self.resolve_struct(*sym_id, env),
                    Type::Enum(_) => self.resolve_enum(*sym_id, env),
                    _ => (),
                },
                _ => (),
            }
        }

        let mut summary = SourceDiagnosticSummary::default();
        summary.append_summary(&mut self.summary);
        summary
    }

    fn resolve_struct(&mut self, parent_sym_id: SymbolId, env: &ResolverEnv) {
        let ast_id = self.compiler.symbols[parent_sym_id]
            .ast_id
            .expect("Should be user symbols only");
        let abs_struct = env.ast_info.get_struct(ast_id);
        let associated_scope = AssociatedScopeKind::Module(env.current_mod);

        let mut fields: Vec<MemberId> = Vec::new();

        // Tracks duplicate field identifiers
        //
        // This is not a `HashSet` because it is not anticipated that a field of any kind in the
        // majority of scenarios will ever be so large to where a hash system is absolutely needed
        // over a linear scan.
        let mut seen: Vec<&AbstractTypeDef> = Vec::new();

        //TODO: global condition and argument setting.
        //field arg and cond settings.
        //same for enums.

        // Checking if there are duplicate name ids within the same struct along with resolution
        for field_typedef in &abs_struct.fields {
            let type_id = match resolve::resolve_type_expr(
                self.compiler,
                associated_scope,
                &field_typedef.sp_ty_expr,
                ScopeType::Nest,
                ScopeLookupPattern::NoRestrictions,
                env,
            ) {
                TypeExprResult::Type(type_id) => type_id,
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
                        &mut self.summary,
                        preset_err,
                        env.region,
                        self.settings,
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

            seen.push(&field_typedef);

            let member_id = MemberId::new(self.compiler.members.len() as u32);

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

            self.compiler.members.push(MemberSymbolKind::Field(field));
            fields.push(member_id);
        }

        for (i, current_field) in seen.iter().enumerate() {
            if let Some((_, original_field)) = seen
                .iter()
                .enumerate()
                // If the other index was declared after the current index and they have the same identifier
                //
                // Since this iteration specifically checks if the current was declared after the
                // last and the iteration terminates upon the first match, this correctly points at
                // the original field for all duplicates.
                .find(|(other_i, f)| *other_i < i && current_field.name_id == f.name_id)
            {
                let dup_name = self.interner.search(current_field.name_id);

                let orig_span = original_field.name_span;
                let current_field_span = current_field.name_span;

                let core_msg = format!("More than one field has the identifier \"{dup_name}\"");

                let src_diag = SourceDiagnostic::builder(
                    None,
                    DiagnosticLevel::Error,
                    core_msg,
                    env.region.path_id,
                )
                .add_annotation(
                    abs_struct.name_span,
                    AnnotationKind::Secondary,
                    "Found inside this struct".to_string().into(),
                )
                .add_annotation(
                    orig_span,
                    AnnotationKind::Secondary,
                    format!("Original usage of `{dup_name}` here").into(),
                )
                .add_annotation(current_field_span, AnnotationKind::Primary, None)
                .build();

                self.summary.push_diag(src_diag);
            }
        }

        let struct_def = self.compiler.get_struct_mut(parent_sym_id);
        debug_assert_eq!(struct_def.fields.len(), 0);
        struct_def.fields.append(&mut fields);
    }

    fn resolve_enum(&mut self, parent_sym_id: SymbolId, env: &ResolverEnv) {
        let ast_id = self.compiler.symbols[parent_sym_id]
            .ast_id
            .expect("Should be user symbols only");
        let abs_enum = env.ast_info.get_enum(ast_id);

        let mut variants: Vec<MemberId> = Vec::new();

        // For duplicate variant identifiers
        let mut seen: Vec<&AbstractVariant> = Vec::new();

        let associated_scope = AssociatedScopeKind::Module(env.current_mod);

        // Checking if there are duplicate name ids within the same enum
        for (i, variant) in abs_enum.variants.iter().enumerate() {
            seen.push(&variant);

            let member_id = MemberId::new(self.compiler.members.len() as u32);
            let variant_repre = if let Some(spanned_ty_expr) = &variant.sp_ty_expr {
                let type_id = match resolve::resolve_type_expr(
                    self.compiler,
                    associated_scope,
                    &spanned_ty_expr,
                    ScopeType::Nest,
                    ScopeLookupPattern::NoRestrictions,
                    env,
                ) {
                    TypeExprResult::Type(type_id) => type_id,
                    res => {
                        let preset_err = preset_reporter::type_expr_result_to_preset_err(
                            &self.compiler,
                            self.interner,
                            &res,
                            env,
                        )
                        .expect("Result enforced by `match`");

                        preset_reporter::report_preset(
                            &mut self.summary,
                            preset_err,
                            env.region,
                            self.settings,
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
                .members
                .push(MemberSymbolKind::Variant(variant_repre));

            variants.push(member_id);
        }

        for (i, current_variant) in seen.iter().enumerate() {
            if let Some((_, original_variant)) = seen
                .iter()
                .enumerate()
                // If the other index was declared after the current index and they have the same identifier
                //
                // Since this iteration specifically checks if the current was declared after the
                // last and the iteration terminates upon the first match, this correctly points at
                // the original field for all duplicates.
                .find(|(other_i, f)| *other_i < i && current_variant.name_id == f.name_id)
            {
                let dup_name = self.interner.search(current_variant.name_id);

                let orig_span = original_variant.name_span;
                let current_field_span = current_variant.name_span;

                // Preset error?
                let core_msg = format!("More than one variant has the identifier \"{dup_name}\"");

                let src_diag = SourceDiagnostic::builder(
                    None,
                    DiagnosticLevel::Error,
                    core_msg,
                    env.region.path_id,
                )
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
                .add_annotation(current_field_span, AnnotationKind::Primary, None)
                .build();

                self.summary.push_diag(src_diag);
            }
        }

        let enum_def = self.compiler.get_enum_mut(parent_sym_id);
        debug_assert_eq!(enum_def.variants.len(), 0);
        enum_def.variants.append(&mut variants);
    }
}
