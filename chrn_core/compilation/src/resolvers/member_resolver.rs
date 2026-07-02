//TEST:
//Seeing if members having a specific stage will help reduce the complexity of type resolution
//which is stacking infinitely (Infinitely as in the infinite sign here -> 🍔)

use chrn_utils::{
    chrn_settings::ChrnSettings,
    id_types::{AstId, InternedId, MemberId, TypeId},
    intern::Intern,
    source_map::source_diagnostic::{
        DiagnosticLevel, SourceDiagnostic, annotations::AnnotationKind,
    },
};

use crate::{
    lookup::scopes::{AssociatedScopeKind, LookupPattern, ScopeType},
    resolvers::{resolver_env::ResolverEnv, resolver_state::ResolverState},
    script_compiler::{self, ScriptCompiler},
    semantic::{
        hir::hir_concepts::{FieldRepre, MemberSymbolKind, SymbolKind, Type, VariantRepre},
        preset_reporter,
        resolve::{self, TypeExprResult},
    },
    user_defined::UserDefinedMetadata,
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
    settings: &'a ChrnSettings,
    interner: &'a Intern,
    compiler: &'a mut ScriptCompiler,
    envs: &'a [Option<ResolverEnv<'a>>],
    err_vec: Vec<SourceDiagnostic>,
}

impl MemberResolver<'_> {
    /// Instantiation requires that the compiler's state is valid and will panic otherwise
    pub fn new<'a>(
        settings: &'a ChrnSettings,
        envs: &'a [Option<ResolverEnv>],
        interner: &'a Intern,
        compiler: &'a mut ScriptCompiler,
    ) -> MemberResolver<'a> {
        debug_assert_eq!(ResolverState::MEMBER, compiler.resolver_state);
        compiler.resolver_state.advance();
        MemberResolver {
            settings,
            envs,
            interner,
            compiler,
            err_vec: Vec::new(),
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
    pub fn resolve(&mut self) -> Vec<SourceDiagnostic> {
        // A loop intended to move the required checks to see if a field can actually be resolved at
        // this stage outside the call site and turned into general metadata expected by the call sites.
        // This needs to be done in comparison to other resolvers because the ast implicitly proves
        // a user defined piece of data is being resolved.
        //
        // This probably will grow to other resolvers eventually since asts are probably not ALWAYS
        // best to be iterated upon.
        let mut all_user_defined: Vec<UserDefinedMetadata> = Vec::new();
        for (i, ty_info) in self.compiler.types.iter().enumerate() {
            let metadata = match &ty_info.ty {
                Type::Struct(struct_def) => {
                    let sym_id = struct_def.sym_id;
                    let sym = &self.compiler.symbols[sym_id.id as usize];
                    let kind = sym.kind;

                    //WARN: Builtins don't store their type id and may never so the acutal type id has to
                    // be gotten based off the index, which is the type id anyways so this doesn't
                    // change anything except an extra operation to create the type id again.
                    let type_id = TypeId::new(i as u32);
                    let mod_id = ty_info.owner;

                    let ast_id = match sym.ast_id {
                        Some(ast_id) => ast_id,
                        None => continue,
                    };

                    UserDefinedMetadata::new(sym_id, type_id, ast_id, mod_id, kind)
                }
                // Could be combined with the above but not right now
                Type::Enum(enum_def) => {
                    let sym_id = enum_def.sym_id;
                    let sym = &self.compiler.symbols[sym_id.id as usize];
                    let kind = sym.kind;

                    let type_id = TypeId::new(i as u32);
                    let sym = &self.compiler.symbols[sym_id.id as usize];
                    let mod_id = ty_info.owner;

                    let ast_id = match sym.ast_id {
                        Some(ast_id) => ast_id,
                        None => continue,
                    };

                    UserDefinedMetadata::new(sym_id, type_id, ast_id, mod_id, kind)
                }
                _ => continue,
            };

            all_user_defined.push(metadata);
        }

        for metadata in all_user_defined {
            // Modules and AstInfo are dense arrays so this is valid
            let env = self.envs[metadata.mod_id.id].as_ref().expect(
                "Previous loop failed to register user metadata OR dense array is misaligned from module startup",
            );

            match self.compiler.types[metadata.type_id.id as usize].ty {
                Type::Struct(_) => self.resolve_struct(metadata, env),
                Type::Enum(_) => self.resolve_enum(metadata, env),
                _ => unreachable!("Grug"),
            }
        }

        let mut diags = Vec::new();
        diags.append(&mut self.err_vec);
        diags
    }

    fn resolve_struct(&mut self, metadata: UserDefinedMetadata, env: &ResolverEnv) {
        let abs_struct = env.ast_info.get_struct(metadata.ast_id);
        let associated_scope = AssociatedScopeKind::Module(env.current_mod);
        let parent_sym_id = metadata.sym_id;

        let mut fields: Vec<MemberId> = Vec::new();
        // Tracks duplicate field identifiers
        // (ast field idx, name_id)
        let mut seen: Vec<(usize, InternedId)> = Vec::new();

        //TODO: global condition and argument setting.
        //field arg and cond settings.
        //same for enums.

        // Checking if there are duplicate name ids within the same struct along with resolution
        for (i, field_typedef) in abs_struct.fields.iter().enumerate() {
            let type_id = match resolve::resolve_type_expr(
                self.compiler,
                associated_scope,
                &field_typedef.sp_ty_expr,
                ScopeType::Nest,
                LookupPattern::NoRestrictions,
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
                        &mut self.err_vec,
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

            // Attempts to get a more accurate parent symbol location, this is not semantically required
            // anywhere. The idea behind this is that say, we had:
            //
            // struct Person {
            //      state: State
            // }
            //
            // If an error ocurred at state, and a diagnostic wanted a look at where it's actual
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

        let struct_def = self.compiler.get_struct_mut(parent_sym_id);
        debug_assert_eq!(struct_def.fields.len(), 0);
        struct_def.fields.append(&mut fields);
    }

    fn resolve_enum(&mut self, metadata: UserDefinedMetadata, env: &ResolverEnv) {
        let abs_enum = env.ast_info.get_enum(metadata.ast_id);
        let parent_sym_id = metadata.sym_id;

        let mut variants: Vec<MemberId> = Vec::new();
        // (ast variant idx, name_id)
        let mut seen: Vec<(usize, InternedId)> = Vec::new();

        let associated_scope = AssociatedScopeKind::Module(env.current_mod);

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
            let variant_repre = if let Some(spanned_ty_expr) = &variant.sp_ty_expr {
                let type_id = match resolve::resolve_type_expr(
                    self.compiler,
                    associated_scope,
                    &spanned_ty_expr,
                    ScopeType::Nest,
                    LookupPattern::NoRestrictions,
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
                            &mut self.err_vec,
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

        let enum_def = self.compiler.get_enum_mut(parent_sym_id);
        debug_assert_eq!(enum_def.variants.len(), 0);
        enum_def.variants.append(&mut variants);
    }
}
