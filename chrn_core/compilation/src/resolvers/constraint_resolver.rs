//TODO: Schema validation
//TODO: Condition validation
//TODO: Proper directive validation
use chrn_utils::{
    chrn_config::ChrnConfig,
    err_codes::{self, ErrorCode},
    id_types::{
        ExprId, InternedId, MemberId, SpannedContainer, SpannedContainerRef, SymbolId, TypeId,
    },
    intern::Intern,
    source_map::{
        source_diagnostic::{DiagnosticLevel, SourceDiagnostic, annotations::AnnotationKind},
        source_span::SourceSpan,
    },
};
use lang::{
    config_schemas::{self, ConfigSchema, ConfigSchemaKind},
    directives::Directive,
    fmter::Formatted,
    types::{boundaries::TypeBoundaryFlags, builtins::BuiltinType},
    values::Value,
};

use crate::{
    constraints::ArgConstraint,
    lookup::schema_lookup::{self, SchemaResult},
    parser::ast::ast_concepts::{
        AbstractAlias, AbstractConfig, AbstractEnum, AbstractStruct, AbstractTypeDef, AbstractVar,
        Item,
    },
    resolvers::{resolver_env::ResolverEnv, resolver_state::ResolverState},
    script_compiler::ScriptCompiler,
    semantic::{
        hir::{
            hir_concepts::{MemberSymbolKind, OptionAssignmentRoot, SymbolKind, Type},
            hir_exprs::ExprHir,
        },
        preset_err::PresetErr,
        preset_reporter,
    },
};

/// This resolver is focused on ensuring correctness in the semantic information collected from
/// previous stages.
pub struct ConstraintResolver<'a> {
    pub(crate) cfg: &'a ChrnConfig,
    pub(crate) interner: &'a Intern,
    pub(crate) compiler: &'a mut ScriptCompiler,
    pub(crate) err_vec: Vec<SourceDiagnostic>,
}

impl<'a> ConstraintResolver<'a> {
    /// Instantiation requires that the compiler's state is valid and will panic otherwise
    pub fn new(
        cfg: &'a ChrnConfig,
        interner: &'a Intern,
        compiler: &'a mut ScriptCompiler,
    ) -> ConstraintResolver<'a> {
        debug_assert_eq!(ResolverState::CONSTRAINT, compiler.resolver_state);
        compiler.resolver_state.advance();
        ConstraintResolver {
            cfg,
            interner,
            compiler,
            err_vec: Vec::new(),
        }
    }

    pub fn resolve(&mut self, env: &ResolverEnv) -> Result<(), Vec<SourceDiagnostic>> {
        // Everything skipped is not a factor in this compilation step.
        for sym_id in env.compilation_syms.iter().cloned() {
            match self.compiler.symbols[sym_id].kind {
                // This split is more so, users can define these set of symbols, and users cannot
                // define the unreacables.
                SymbolKind::Type(type_id) => match &self.compiler.types[type_id].ty {
                    Type::Struct(_) => self.resolve_struct(sym_id, env),
                    Type::Enum(_) => self.resolve_enum(sym_id, env),
                    Type::Alias(_) => self.resolve_alias(sym_id, env),
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
                SymbolKind::Config(_) => self.resolve_cfg_root(sym_id, env),
                // Users cannot define these but they exist internally.
                SymbolKind::Namespace | SymbolKind::Directive(_) => unreachable!(),
            }
        }

        if !self.err_vec.is_empty() {
            let mut diags = Vec::new();
            diags.append(&mut self.err_vec);

            return Err(diags);
        }

        Ok(())
    }

    // Needs:
    //
    // Maybe we can privacy check here so semantic information is still present, and the error is
    // also present
    fn resolve_var(&mut self, parent_sym_id: SymbolId, env: &ResolverEnv) {
        // Not sure what this might need checked yet other than privacy
        let ast_id = self.compiler.symbols[parent_sym_id]
            .ast_id
            .expect("Should be user symbols only");
        let abs_var = env.ast_info.get_var(ast_id);

        // let val_info = self.compiler.get_var(sym_id);
        // let ty = &self.compiler.types[val_info.type_id ].ty;

        // Not sure what to do with this yet
        // if let Type::Unknown = ty {
        //     return Err(());
        // }
    }

    // The code below is far far worse than all prior because the concept of what a config is and
    // enforces is not 100% done, but the end-behavior exists so the specifics will be sorted later.
    fn resolve_cfg_root(&mut self, parent_sym_id: SymbolId, env: &ResolverEnv) {
        // let ast_id = self.compiler.symbols[parent_sym_id]
        //     .ast_id
        //     .expect("Should be user symbols only");
        // let abs_cfg_root = env.ast_info.get_cfg_root(ast_id);

        // leconstraint_reot module = &self.compiler.mods[env.current_mod];
        let cfg_root = self.compiler.get_cfg_def_root(parent_sym_id);

        let Some(linked_sym_id) = cfg_root.linked_sym_id else {
            return;
        };

        // We may need an invalid and valid marker for cached checks regarding if it was a type id
        // or not.
        // let cfg_sym = &self.compiler.symbols[linked_sym_id];

        // If the bar is ever moved to where the linking is possible even with an invalid config
        // this will be false.
        //
        // This should probably never change because the odds of linking a symbol id to such a
        // broken config being useful error message wise seems unlikely
        let linked_type_id = self
            .compiler
            .get_type_id_from_sym_id(linked_sym_id)
            .expect("`TypeResolver` should only give linked sym ids to valid configs");

        for opt_root_id in cfg_root.opt_assignments.iter().copied() {
            let opt_root = self.compiler.get_opt_assignment_root(opt_root_id);
            let schema = schema_lookup::get_schema_from_type_id(self.compiler, linked_type_id)
                .expect("`TypeResolver` should only give linked sym ids to valid configs");

            let sp_opt_name_id = SpannedContainer::new(opt_root.name_id, opt_root.name_span);
            let boundaries = Type::boundaries(self.compiler, linked_type_id);
            let type_name_id = self.compiler.get_name_id_from_type_id(linked_type_id);

            // let ty_name_id = self.compiler.get_span_from_type_id(linked_type_id).unwrap();
            if let Err(preset_err) = self.check_opt(
                schema,
                type_name_id,
                cfg_root.name_span,
                boundaries,
                &sp_opt_name_id,
                opt_root.array_expr_id,
                env,
            ) {
                // Maybe return ONE more present? Just 2? A small slice?
                // A SLICE?
                // Yeah sure
                preset_reporter::report_preset(
                    &mut self.err_vec,
                    preset_err,
                    env.region,
                    self.cfg,
                    self.interner,
                );
            };
        }

        // :( Clone
        for cfg_member_id in cfg_root.cfg_members.clone() {
            //WARN: Suspicious
            if self.compiler.members[cfg_member_id].is_unknown() {
                continue;
            }

            let cfg_member = self.compiler.get_cfg_def_member(cfg_member_id);
            //NOTE: The somewhat dangerous part of this staying `Option` is that it IS a real
            //reflection of the fact that for something like a variant, there COULD be no boundary
            //set, but it could also hide silent bugs, just like in the recursive resolution of
            //option members.
            let boundaries =
                MemberSymbolKind::boundaries(self.compiler, cfg_member.linked_member_id);

            //TODO: Given the scope type, should react differently to depths of members.
            //Or, maybe `TypeResolver` can just do this? This actually isn't that hard to check.
            for opt_member_id in cfg_member.opt_assignments.iter().copied() {
                // Variant and field specific schemas?
                let opt_member = self.compiler.get_opt_assignment_member(opt_member_id);
                let schema = config_schemas::get_cfg_schema(ConfigSchemaKind::Member);
                // let schema = schema_lookup::get_schema_from_type_id(self.compiler, linked_type_id)
                //     .expect("`TypeResolver` should only give linked sym ids to valid configs");
                let sp_opt_name_id =
                    SpannedContainer::new(opt_member.name_id, opt_member.name_span);
                //FIX:
                let member_ty_name_id_opt = self
                    .compiler
                    .get_type_id_from_member_id(cfg_member.linked_member_id)
                    .map(|id| self.compiler.get_name_id_from_type_id(id))
                    .flatten();

                if let Err(preset_err) = self.check_opt(
                    schema,
                    member_ty_name_id_opt,
                    cfg_member.name_span,
                    boundaries,
                    &sp_opt_name_id,
                    opt_member.array_expr_id,
                    env,
                ) {
                    // Maybe return ONE more present? Just 2? A small slice?
                    // No
                    // Slice as in [Option<PresetErr>;2]
                    // Ok sure
                    preset_reporter::report_preset(
                        &mut self.err_vec,
                        preset_err,
                        env.region,
                        self.cfg,
                        self.interner,
                    );
                };
            }

            // AAAAAAAAAAAAAHHHHHHHHHHHHHHHHHH
            // Recursively resolves inner members
            // self.resolve_cfg_member(cfg_member_id, env);

            // for thing in cfg_member.cfg_def_members.iter().cloned() {
            //     let mem = self.compiler.get_cfg_def_member(thing);
            //     dbg!(self.interner.search(mem.name_id));
            //     dbg!(mem);
            //     dbg!(thing);
            //     todo!("Ok");
            // }
        }
        // todo!("Recursively check inner cfg members");
    }

    fn resolve_cfg_member(&mut self, parent_member_id: MemberId, env: &ResolverEnv) {
        todo!()
    }

    // Coupled so it's not member option or root option specific
    //
    // For certain errors like the same type as user one, it needs to point at the member or symbol
    // it's attached to
    /// Convenience method that takes the required pieces of any option and forms `PresetErr` given
    /// any errors.
    fn check_opt(
        // We need the config details associated with the given option, but in pieces.
        &self,
        schema: &ConfigSchema,
        cfg_ty_name_id: Option<InternedId>,
        cfg_name_span: SourceSpan,
        // Is option since if the root option is typedef, it has user boundaries to account for. If
        // the root option is a struct, there are none.
        user_boundaries_opt: Option<TypeBoundaryFlags>,
        sp_opt_name_id: &SpannedContainer<InternedId>,
        array_expr_id: ExprId,
        env: &ResolverEnv,
    ) -> Result<(), PresetErr> {
        // Beep
        let array_expr = &self.compiler.exprs[array_expr_id];
        let val_id = array_expr.val_id;
        // This seems like something that should be an error earlier since an array with unfinished
        // values after full type resolution is just plainly broken as of right now. Need some layer
        // before this that can serve this as a guarantee
        let const_array = self.compiler.values[val_id]
            .const_val
            .as_ref()
            .expect("NOT DONE YET");

        // Maybe just add expects for expressions :(
        let Value::Array(values) = const_array else {
            panic!("NOT DONE");
        };

        // 1. Match kind
        // 2. Check for the identifier of the option to ensure it aligns with something in the schema
        // 3. Check boundaries
        // 4. unwrap()
        match schema_lookup::validate_opt(schema, user_boundaries_opt, sp_opt_name_id.inner, values)
        {
            // DO NOT. USE. THE UNREACHABLE. DO NOT DO IT.
            SchemaResult::Valid => Ok(()),
            res => {
                let opt_name = self.interner.search(sp_opt_name_id.inner);
                let src_diag = match res {
                    SchemaResult::BoundaryMismatch {
                        err_idx,
                        required_boundaries,
                        err_boundaries: _,
                    } => {
                        // Value pos aligns with expr input pos

                        // They're values and not named so..!
                        let core_msg = format!(
                            "Index `{err_idx}` does not satisfy `{required_boundaries}` which is required by option `{opt_name}`",
                        );

                        let err_expr_id = array_expr.inputs[err_idx];
                        let err_span = self.compiler.exprs[err_expr_id].span;
                        SourceDiagnostic::builder(
                            ErrorCode::SchemaOptionErr.code().into(),
                            DiagnosticLevel::Error,
                            core_msg,
                            env.region.path_id,
                        )
                        .add_annotation(
                            err_span,
                            AnnotationKind::Primary,
                            format!("Does not satisfy `{required_boundaries}`").into(),
                        )
                        .add_annotation(
                            sp_opt_name_id.span,
                            AnnotationKind::Secondary,
                            "Required by this option".to_string().into(),
                        )
                    }
                    // Ok.
                    SchemaResult::NoBoundariesInValue {
                        err_idx,
                        required_boundaries,
                    } => {
                        let core_msg = format!(
                            "Index `{err_idx}` does not satisfy `{required_boundaries}` which is required by option `{opt_name}`",
                        );

                        let err_expr_id = array_expr.inputs[err_idx];
                        let err_span = self.compiler.exprs[err_expr_id].span;
                        SourceDiagnostic::builder(
                            ErrorCode::SchemaOptionErr.code().into(),
                            DiagnosticLevel::Error,
                            core_msg,
                            env.region.path_id,
                        )
                        .add_annotation(
                            err_span,
                            AnnotationKind::Primary,
                            format!("Does not satisfy `{required_boundaries}`").into(),
                        )
                        .add_annotation(
                            sp_opt_name_id.span,
                            AnnotationKind::Secondary,
                            "Required by this option".to_string().into(),
                        )
                    }
                    // The current option's constraint requires that the type linked to the actual
                    // config the currnet option is attached to must align with all the values
                    // given.

                    // This is encoding two different error routes, should probably do the internal
                    // split with Some, None return eaoifjeiofjoiaj
                    // This thing above
                    // TODO: Return type too?
                    SchemaResult::SameTypeAsUserMismatch {
                        err_idx,
                        err_boundaries_opt,
                        user_boundaries,
                    } => {
                        let core_msg =
                            format!("Index `{err_idx}` does not have the same type as it's config");

                        let err_expr_id = array_expr.inputs[err_idx];
                        let err_span = self.compiler.exprs[err_expr_id].span;
                        let lowest_bound = user_boundaries.to_fmt_lowest();

                        let next_ann_msg = if let Some(inner) = err_boundaries_opt {
                            let lowest_bound = inner.to_fmt_lowest();
                            format!("Is `{lowest_bound}`")
                        } else {
                            "Has no boundaries".to_string()
                        };

                        let mut builder = SourceDiagnostic::builder(
                            ErrorCode::SchemaOptionErr.code().into(),
                            DiagnosticLevel::Error,
                            core_msg,
                            env.region.path_id,
                        )
                        .add_annotation(
                            err_span,
                            AnnotationKind::Primary,
                            next_ann_msg.into(),
                        );

                        if let Some(ty_name_id) = cfg_ty_name_id {
                            let ty_name = self.interner.search(ty_name_id);
                            builder = builder.add_annotation(
                                cfg_name_span,
                                AnnotationKind::Secondary,
                                format!("Is type `{ty_name}`").into(),
                            )
                        };
                        builder

                        // This is a little odd since it directly points out a boundary, but the
                        // type name of the config points out a concrete type.
                        //
                        // builder = builder.add_annotation(err_span, AnnotationKind::Primary, None);
                    }
                    SchemaResult::CannotSupportBoundaries => {
                        let opt_name = self.interner.search(sp_opt_name_id.inner);
                        let core_msg = format!(
                            "Option `{opt_name}` requires that the config it is attached to has boundaries"
                        );

                        // let err_expr_id = array_expr.inputs[err_idx];
                        // let err_span = self.compiler.exprs[err_expr_id].span;
                        SourceDiagnostic::builder(
                            ErrorCode::SchemaOptionErr.code().into(),
                            DiagnosticLevel::Error,
                            core_msg,
                            env.region.path_id,
                        )
                        .add_annotation(
                            cfg_name_span,
                            AnnotationKind::Primary,
                            "Has no type so cannot hold boundaries".to_string().into(),
                        )
                        .add_annotation(
                            sp_opt_name_id.span,
                            AnnotationKind::Secondary,
                            "Required by this option".to_string().into(),
                        )
                        // .add_annotation(
                        //     err_span,
                        //     AnnotationKind::Secondary,
                        //     format!("Enforces `{}`").into(),
                        // )
                    }
                    SchemaResult::UnknownOptionName => {
                        let opt_name = self.interner.search(sp_opt_name_id.inner);
                        let core_msg = format!(
                            "Option `{opt_name}` does not exist for schema `{}`",
                            schema.kind
                        );
                        SourceDiagnostic::builder(
                            ErrorCode::SchemaOptionErr.code().into(),
                            DiagnosticLevel::Error,
                            core_msg,
                            env.region.path_id,
                        )
                        .add_annotation(sp_opt_name_id.span, AnnotationKind::Primary, None)
                        .add_annotation(
                            cfg_name_span,
                            AnnotationKind::Secondary,
                            format!("Uses schema `{}`", schema.kind).into(),
                        )
                    }
                    SchemaResult::Valid => unreachable!(),
                };

                Err(PresetErr::General(src_diag))
            }
        }
    }

    fn resolve_typedef(&mut self, parent_sym_id: SymbolId, env: &ResolverEnv) {
        let ast_id = self.compiler.symbols[parent_sym_id]
            .ast_id
            .expect("Should be user symbols only");
        let abs_typedef = env.ast_info.get_typedef(ast_id);

        let type_def = self.compiler.get_typedef(parent_sym_id);
        let ty_info = &self.compiler.types[type_def.type_id];

        // Checking if condition is valid for the given type
        // Using the Ast node's condition so that the span information is not lost
        let ty_span = abs_typedef.sp_ty_expr.span;
        for (i, cond_expr) in type_def.conds.iter().enumerate() {
            let ast_span = &abs_typedef.conds[i].span;

            match &ty_info.ty {
                Type::Struct(_) | Type::Enum(_) => {
                    //NOTE: Would be better as a note
                    // The issue with allowing this is if it were not restricted, and p: Person was
                    // typed, that would mean that "other_p: Person" inside the same var-> would
                    // need to align with whatever conditions or arguments given, which would be
                    // problematic. Hence, it just has to be a shallowly applied directive instead.
                    let core_msg = "Cannot give a `var->` defined type a condition when it has a `struct` or `enum` type, define\nthis within `nest->`".to_string();

                    //TODO: Maybe add an error code but this isn't a final decision so not yet
                    let src_diag = SourceDiagnostic::basic(
                        None,
                        DiagnosticLevel::Error,
                        core_msg,
                        env.region.path_id,
                        *ast_span,
                    );
                    // semantic_reporter::report_semantic(
                    //     &mut self.err_vec,
                    //     preset_err,
                    //     env.region,
                    //     self.settings,
                    //     self.interner,
                    // );
                    // semantic_reporter::create_diag_builder_preset(
                    //     &mut self.err_vec,
                    //     preset_err,
                    //     env.region,
                    //     self.settings,
                    //     self.interner,
                    // );
                    self.err_vec.push(src_diag);
                }
                _ => (),
            }

            if let Err(preset_errs) = self.check_cond(type_def.type_id, ty_span, *cond_expr) {
                for err in preset_errs {
                    preset_reporter::report_preset(
                        &mut self.err_vec,
                        err,
                        env.region,
                        self.cfg,
                        self.interner,
                    );
                }
            }
        }

        for sp_directive in &type_def.directives {
            let directive = &self.compiler.directives[sp_directive.inner];
            match &ty_info.ty {
                Type::Struct(_) | Type::Enum(_) => {
                    if directive.has_restrictions() {
                        let preset_err = PresetErr::VagueDirective(SpannedContainer::new(
                            directive.clone(),
                            sp_directive.span,
                        ));

                        preset_reporter::report_preset(
                            &mut self.err_vec,
                            preset_err,
                            env.region,
                            self.cfg,
                            self.interner,
                        );

                        continue;
                    }
                }
                _ => (),
            }

            if let Err(Some(preset_err)) = self.check_directive(
                type_def.type_id,
                abs_typedef.name_span,
                ty_span,
                &SpannedContainerRef::new(directive, sp_directive.span),
                &mut Vec::new(),
                env,
            ) {
                preset_reporter::report_preset(
                    &mut self.err_vec,
                    preset_err,
                    env.region,
                    self.cfg,
                    self.interner,
                );
            }
        }
    }

    // Needs:
    // Check that only known parameters are used in condition expressions.
    //
    // Check if it's removable depending on if it ONLY has args.
    //
    // Check that all args used align with all function constraints
    //
    // Check that only functions that align with the alias's specific type is used if there is a
    // "Default" expression inside, which just means if all params are not of type `Unknown`
    //
    // Need constraints to be added if anything like Range, IsEmpty, etc is used which would mean
    // that the alias must be a number or CharacterMappable

    // Alias should probably be ran first by default
    // Also needs to infer it's own constraints
    fn resolve_alias(&mut self, parent_sym_id: SymbolId, env: &ResolverEnv) {
        let ast_id = self.compiler.symbols[parent_sym_id]
            .ast_id
            .expect("Should be user symbols only");
        let abs_alias = env.ast_info.get_alias(ast_id);

        //TODO: Need to typecheck based off of the conditional expressions found

        // let alias_type_id = self.compiler.get_type_id(sym_id);
        let alias_def = self.compiler.get_alias(parent_sym_id);
        let alias_type_id = self.compiler.extract_type_id(parent_sym_id);

        // TODO: This should now just check instead of infer

        // let mut found_constraints: Vec<Option<TypeBoundaryFlags>> =
        //     vec![None; alias_def.params.len()];
        //
        // for (i, param) in alias_def.params.iter().enumerate() {
        //     let current_constraints_opt = &mut found_constraints[i];
        //
        //     for cond_expr_id in alias_def.conds.iter().copied() {
        //         let param_span = abs_alias.params[i].name_span;
        //
        //         let name_id = self.compiler.symbols[param.sym_id ].name_id;
        //         // New constraints were found which would need to be lowerable
        //         match self.infer_type_constraint_from_expr(cond_expr_id, name_id, param_span) {
        //             Some(new_constraints) => {
        //                 dbg!(new_constraints.to_type_constraint_vec());
        //                 if let Some(current) = current_constraints_opt {
        //                     if new_constraints == *current {
        //                         continue;
        //                     }
        //
        //                     dbg!(&current, new_constraints);
        //                     if current.contains(*&new_constraints) {
        //                         if let Some(lowered) = current.try_lower_to(new_constraints) {
        //                             *current_constraints_opt = Some(lowered);
        //                         } else {
        //                             // let msg = format!("Cannot lower `{}` to `{}`");
        //                             // self.reporter.report_spanned(msg, err_name, spans, metadata);
        //                             todo!("Beep");
        //                         }
        //                     } else {
        //                         let cond_expr = &self.compiler.exprs[cond_expr_id ];
        //
        //                         let preset_err = SemanticError::TypeBoundaryBoundConflict(
        //                             *current,
        //                             new_constraints,
        //                             vec![param_span, cond_expr.span],
        //                         );
        //
        //                         let module = &self.compiler.mods[env.current_mod.id];
        //                         self.reporter.report_semantic(
        //                             preset_err,
        //                             &module
        //                                 .src_metadata
        //                                 .as_ref()
        //                                 .expect("Should be user symbols only"),
        //                         );
        //                     }
        //                     // There is no previous so it is initialized
        //                 } else {
        //                     *current_constraints_opt = Some(new_constraints);
        //                     // dbg!(new_constraints.to_type_constraints());
        //                 }
        //
        //                 // No constraint was stored so this one takes precedence
        //             }
        //             // No new constraints were found
        //             None => (),
        //         }
        //         dbg!("Looped");
        //     }
        // }
        // panic!("Paraming");

        // Filter out duplicates in the type resolver!!
        // let mut ty_constraint: Option<TypeBoundary> = None;
        // for (i, sp_directive) in abs_alias.args.iter().enumerate() {
        //     let param_span = abs_alias.params[i].name_span;
        //     match self.infer_type_constraint_from_arg(sp_directive, param_span) {
        //         Ok(constraint_opt) => {
        //             todo!("Constraining contraint check of cocnsctraint");
        //         }
        //         Err(preset_err) => {
        //             let module = &self.compiler.mods[env.current_mod.id];
        //             self.reporter.report_semantic(
        //                 preset_err,
        //                 &module
        //                     .src_metadata
        //                     .as_ref()
        //                     .expect("Should be user symbols only"),
        //             );
        //         }
        //     }
        // }

        // FIX: Ok so maybe we can keep both systems to where, if it's constrained, check
        // constraints, otherwise, keep the same concrete type checks with builtins

        // Only the type of functions used matter if they depend on self.
        let alias_def = self.compiler.get_alias_mut(parent_sym_id);
        // alias_def.ty_constraints = found_constraints.iter().filter_map(|c| c.is_some());

        // Currently assuming that if we see none here it's fine since technically, you could
        // declare a parameter and have it just not be used and never face any type errors.

        let alias_def = self.compiler.get_alias(parent_sym_id);
        // Need a system where it takes a local variable, looks through each expression, sees if
        // it's used, then if so attempts to assign the constraint to the used argument.

        let module = &self.compiler.mods[env.current_mod];
        // NO
        unimplemented!("Stop using the alias please");

        // NOTE: Small issue here is that when we check an alias, and it has an error, it's
        // emitted. But then if we have something that USES the alias, it also gets that error.
        let sym_span = env.ast_info.get_name_span(ast_id);
        for cond_expr_id in &alias_def.conds {
            if let Err(preset_errs) = self.check_cond(alias_type_id, sym_span, *cond_expr_id) {
                for err in preset_errs {
                    preset_reporter::report_preset(
                        &mut self.err_vec,
                        err,
                        env.region,
                        self.cfg,
                        self.interner,
                    );
                }
            }
        }
    }

    // // Not quite sure what to do with this yet since it's only used for alias, if it were used for
    // // more than alias then a recursive check would be needed. But currently, not muc helse needs
    // // to be done with this since most is unerachable
    // fn infer_type_constraint_from_expr(
    //     &self,
    //     expr_id: ExprId,
    //     // Should this be sym_id?
    //     // Can't really do that right now because expressions aren't symbols, but x usage
    //     // represents the x symbol in the local scope
    //     param_name_id: InternedId,
    //     param_span: SourceSpan,
    // ) -> Option<TypeBoundaryFlags> {
    //     let expr = &self.compiler.exprs[expr_id ];
    //     match &expr.expr_hir {
    //         ExprHir::Val(val_id) => {
    //             panic!("Val id");
    //         }
    //         ExprHir::Var(sym_id) => {
    //             let symbol = &self.compiler.symbols[sym_id ];
    //
    //             match &self.compiler.symbols[sym_id ].kind {
    //                 SymbolKind::Type(type_id) => match &self.compiler.types[type_id ].ty
    //                 {
    //                     Type::BuiltinType(builtin_ty) => {
    //                         // If we go from symbol -> Type, that means the previous symbol can be
    //                         // checked for same identifier/symbol id since we are looking at
    //                         // something that looks like, x: SomeType, rather than Func(x) where
    //                         // the symbol represents the function, not the inner x.
    //                         //
    //                         // Not final in how this works.
    //                         if symbol.name_id != param_name_id {
    //                             return None;
    //                         }
    //
    //                         Some(builtin_ty.kind().type_constraints())
    //                     }
    //                     // rec check
    //                     Type::Struct(struct_def) => todo!(),
    //                     Type::Enum(enum_def) => todo!(),
    //                     Type::Func(func_def) => {
    //                         if func_def.is_callable {
    //                             Some(func_def.type_constraints)
    //                         } else {
    //                             None
    //                         }
    //                     }
    //                     Type::Alias(alias_def) => todo!(),
    //                     Type::TypeDef(type_def) => todo!(),
    //                     Type::Unknown => todo!("Unknown"),
    //                     Type::Constrained(constraint) => {
    //                         if symbol.name_id != param_name_id {
    //                             return None;
    //                         }
    //
    //                         Some(*constraint)
    //                     }
    //                 },
    //                 SymbolKind::Val(_) => {
    //                     // Need a function to get this
    //                     let symbol = &self.compiler.symbols[sym_id ];
    //                     if param_name_id == symbol.name_id {
    //                         let type_id = self.compiler.exprs[expr_id ].type_id;
    //                         return constraints::get_type_constraints(
    //                             self.compiler,
    //                             type_id,
    //                             param_span,
    //                             false,
    //                         );
    //                     }
    //
    //                     None
    //                 }
    //                 SymbolKind::Module(mod_id) => todo!("I'm a module"),
    //                 SymbolKind::Unknown => todo!("Unknown"),
    //             }
    //         }
    //         ExprHir::Default(sym_expr_id, expr_id) => {
    //             todo!()
    //         }
    //         ExprHir::Call(callee_id, arg_ids) => {
    //             let mut has_param_name = false;
    //
    //             for arg_expr_id in arg_ids {
    //                 let expr_hir = &self.compiler.exprs[arg_expr_id ].expr_hir;
    //                 if let ExprHir::Var(sym_id) = expr_hir {
    //                     let sym = &self.compiler.symbols[sym_id ];
    //                     if sym.name_id == param_name_id {
    //                         has_param_name = true;
    //                     }
    //                 }
    //             }
    //
    //             if has_param_name {
    //                 return self.infer_type_constraint_from_expr(
    //                     *callee_id,
    //                     param_name_id,
    //                     param_span,
    //                 );
    //             }
    //
    //             None
    //         }
    //         // Maybe operators also need to carry constraints since that does
    //         //TODO: Not quite sure what this should do yet
    //         ExprHir::Unary { op, operand } => {
    //             self.infer_type_constraint_from_expr(*operand, param_name_id, param_span)
    //         }
    //         ExprHir::BinaryExpr { lhs, op, rhs } => {
    //             let ty = &self.compiler.types[expr.type_id ].ty;
    //             match ty {
    //                 Type::BuiltinType(builtin_ty) => Some(builtin_ty.kind().type_constraints()),
    //                 Type::Struct(struct_def) => todo!(),
    //                 Type::Enum(enum_def) => todo!(),
    //                 Type::Func(func_def) => todo!(),
    //                 Type::Alias(alias_def) => todo!(),
    //                 Type::TypeDef(type_def) => todo!(),
    //                 Type::Constrained(constraint) => Some(*constraint),
    //                 Type::Unknown => todo!(),
    //             }
    //         }
    //     }
    // }
    //
    // fn infer_type_constraint_from_arg(
    //     &self,
    //     sp_directive: &SpannedInnerArgs,
    //     param_span: SourceSpan,
    // ) -> Result<Option<TypeBoundary>, SemanticError> {
    //     todo!()
    // }

    // //NOTE: The reason this would need to look at the struct again would be because it is iterating
    // // through items despite there already being a known struct id, which could be prevented if the
    // // struct id itself was passed, but then the loop would iterate over everything by default
    // // which seems bad if they're just builtins etc.

    // Needs:
    //
    fn resolve_struct(&mut self, parent_sym_id: SymbolId, env: &ResolverEnv) {
        //TODO: global condition and argument setting.
        //field arg and cond settings.
        //same for enums.

        let ast_id = self.compiler.symbols[parent_sym_id]
            .ast_id
            .expect("Should be user symbols only");
        let abs_struct = env.ast_info.get_struct(ast_id);

        let struct_def = self.compiler.get_struct(parent_sym_id);

        // Glob conds
        for (i, member_id) in struct_def.fields.iter().enumerate() {
            let field = self.compiler.get_field(*member_id);
            let ty_span = abs_struct.fields[i].sp_ty_expr.span;

            for cond_expr in &struct_def.glob_conds {
                if let Err(preset_errs) = self.check_cond(field.type_id, ty_span, *cond_expr) {
                    for err in preset_errs {
                        preset_reporter::report_preset(
                            &mut self.err_vec,
                            err,
                            env.region,
                            self.cfg,
                            self.interner,
                        );
                    }
                }
            }
        }

        // Field conds
        for (i, member_id) in struct_def.fields.iter().enumerate() {
            let field = self.compiler.get_field(*member_id);
            let ty_span = abs_struct.fields[i].sp_ty_expr.span;

            for cond_expr in &field.conds {
                if let Err(preset_errs) = self.check_cond(field.type_id, ty_span, *cond_expr) {
                    for err in preset_errs {
                        preset_reporter::report_preset(
                            &mut self.err_vec,
                            err,
                            env.region,
                            self.cfg,
                            self.interner,
                        );
                    }
                }
            }
        }

        // Glob directives
        for (i, member_id) in struct_def.fields.iter().enumerate() {
            let field = self.compiler.get_field(*member_id);
            let ty_span = abs_struct.fields[i].sp_ty_expr.span;

            for sp_directive in &struct_def.glob_directives {
                let directive = &self.compiler.directives[sp_directive.inner];
                if let Err(Some(preset_err)) = self.check_directive(
                    field.type_id,
                    abs_struct.name_span,
                    ty_span,
                    &SpannedContainerRef::new(directive, sp_directive.span),
                    &mut vec![],
                    env,
                ) {
                    preset_reporter::report_preset(
                        &mut self.err_vec,
                        preset_err,
                        env.region,
                        self.cfg,
                        self.interner,
                    );
                }
            }
        }

        // Field directives
        for (i, member_id) in struct_def.fields.iter().enumerate() {
            let field = self.compiler.get_field(*member_id);
            //WARN: Type spanning is not done yet
            let field_ty_span = &abs_struct.fields[i].sp_ty_expr.span;

            for sp_directive in &field.directives {
                let directive = &self.compiler.directives[sp_directive.inner];
                if let Err(Some(preset_err)) = self.check_directive(
                    field.type_id,
                    abs_struct.name_span,
                    *field_ty_span,
                    &SpannedContainerRef::new(directive, sp_directive.span),
                    &mut vec![],
                    env,
                ) {
                    preset_reporter::report_preset(
                        &mut self.err_vec,
                        preset_err,
                        env.region,
                        self.cfg,
                        self.interner,
                    );
                }
            }
        }
    }

    fn resolve_enum(&mut self, parent_sym_id: SymbolId, env: &ResolverEnv) {
        let ast_id = self.compiler.symbols[parent_sym_id]
            .ast_id
            .expect("Should be user symbols only");
        let abs_enum = env.ast_info.get_enum(ast_id);

        let enum_def = &self.compiler.get_enum(parent_sym_id);

        // Glob conds
        for (i, member_id) in enum_def.variants.iter().enumerate() {
            let variant = self.compiler.get_variant(*member_id);
            if let Some(inner_id) = variant.type_id {
                let ty_span = abs_enum.variants[i]
                    .sp_ty_expr
                    .as_ref()
                    .expect("Already checked")
                    .span;

                for cond_expr in &enum_def.glob_conds {
                    if let Err(preset_errs) = self.check_cond(inner_id, ty_span, *cond_expr) {
                        for err in preset_errs {
                            preset_reporter::report_preset(
                                &mut self.err_vec,
                                err,
                                env.region,
                                self.cfg,
                                self.interner,
                            );
                        }
                    }
                }
            }
        }

        // Variant conds
        for (i, member_id) in enum_def.variants.iter().enumerate() {
            let variant = self.compiler.get_variant(*member_id);
            if let Some(inner_id) = variant.type_id {
                let ty_span = abs_enum.variants[i]
                    .sp_ty_expr
                    .as_ref()
                    .expect("Already checked")
                    .span;

                for cond_expr in &variant.conds {
                    if let Err(preset_errs) = self.check_cond(inner_id, ty_span, *cond_expr) {
                        for err in preset_errs {
                            preset_reporter::report_preset(
                                &mut self.err_vec,
                                err,
                                env.region,
                                self.cfg,
                                self.interner,
                            );
                        }
                    }
                }
            }
        }

        // Glob args
        for (i, member_id) in enum_def.variants.iter().enumerate() {
            let variant = self.compiler.get_variant(*member_id);
            if let Some(inner_id) = variant.type_id {
                let ty_span = abs_enum.variants[i]
                    .sp_ty_expr
                    .as_ref()
                    .expect("Just checked")
                    .span;

                for sp_directive in &enum_def.glob_directives {
                    let directive = &self.compiler.directives[sp_directive.inner];
                    if let Err(Some(preset_err)) = self.check_directive(
                        inner_id,
                        abs_enum.name_span,
                        ty_span,
                        &SpannedContainerRef::new(directive, sp_directive.span),
                        &mut vec![],
                        env,
                    ) {
                        preset_reporter::report_preset(
                            &mut self.err_vec,
                            preset_err,
                            env.region,
                            self.cfg,
                            self.interner,
                        );
                    }
                }
            }
        }

        // Variant args
        for (i, member_id) in enum_def.variants.iter().enumerate() {
            let variant = self.compiler.get_variant(*member_id);
            if let Some(inner_id) = variant.type_id {
                let abs_variant = &abs_enum.variants[i];
                let variant_ty_span = abs_variant.sp_ty_expr.as_ref().expect("Just checked").span;

                for sp_directive in &variant.directives {
                    let directive = &self.compiler.directives[sp_directive.inner];
                    if let Err(Some(preset_err)) = self.check_directive(
                        inner_id,
                        abs_enum.name_span,
                        variant_ty_span,
                        &SpannedContainerRef::new(directive, sp_directive.span),
                        &mut vec![],
                        env,
                    ) {
                        preset_reporter::report_preset(
                            &mut self.err_vec,
                            preset_err,
                            env.region,
                            self.cfg,
                            self.interner,
                        );
                    }
                }
            }
        }
    }

    // TODO: Type alignment with the used function
    fn check_cond(
        &self,
        parent_ty_id: TypeId,
        parent_span: SourceSpan,
        cond_expr_id: ExprId,
    ) -> Result<(), Vec<PresetErr>> {
        // let cond_expr = &self.compiler.exprs[cond_expr_id ];
        //
        // // if visited.contains(&field.type_id) {
        // //     if spanned_directive.arg.has_restrictions() {
        // //         let name = self.interner.search(symbol.name_id );
        // //
        // //         let msg = format!(
        // //             "The type `{name}` cannot have `#{}` applied due to recursively relying on itself satisfying the argument",
        // //             spanned_directive.arg
        // //         );
        // //
        // //         return Err(SemanticError::General(
        // //             msg,
        // //             vec![spanned_directive.span, active_span],
        // //         ));
        // //     }
        // match &cond_expr.expr_hir {
        //     ExprHir::Call(callee_expr_id, arg_expr_ids) => {
        //         let callee = &self.compiler.exprs[callee_expr_id ];
        //         let ty = &self.compiler.types[callee.type_id ].ty;
        //
        //         match ty {
        //             Type::Func(func_def) => {
        //                 if !func_def.is_callable {
        //                     let msg = "Predicate keywords cannot use parameters".to_string();
        //                     return Err(vec![SemanticError::General(msg, vec![cond_expr.span])]);
        //                 }
        //
        //                 // Top level functions or predicates used within type constraints must
        //                 // evaluate to a boolean
        //                 let ret_type = &self.compiler.types[func_def.ret_type ].ty;
        //
        //                 if let Type::BuiltinType(BuiltinType::Bool) = ret_type {
        //                     // Maybbe tturrnrn in tot a fucntinson
        //                     if let Err(preset_err) = self.check_arg_constraints(
        //                         parent_ty_id,
        //                         parent_span,
        //                         cond_expr_id,
        //                         arg_expr_ids,
        //                         &func_def.arg_constraints,
        //                     ) {
        //                         return Err(preset_err);
        //                     };
        //
        //                     match constraints::check_type_constraint(
        //                         self.compiler,
        //                         parent_ty_id,
        //                         parent_span,
        //                         cond_expr.span,
        //                         &mut Vec::new(),
        //                         func_def.type_constraints,
        //                     ) {
        //                         Ok(_) => Ok(()),
        //                         Err(preset_err) => return Err(vec![preset_err]),
        //                     }
        //                 } else {
        //                     let msg = "Top level functions or predicates used within type constraint blocks must evaluate to a boolean"
        //                         .to_string();
        //                     Err(vec![SemanticError::General(msg, vec![cond_expr.span])])
        //                 }
        //             }
        //             Type::Alias(alias_def) => {
        //                 let mut preset_errs: Vec<SemanticError> = Vec::new();
        //
        //                 // Checking the arguments given in the call against the arg constraints of
        //                 // the alias
        //                 if let Err(preset_err) = self.check_arg_constraints(
        //                     parent_ty_id,
        //                     parent_span,
        //                     cond_expr_id,
        //                     arg_expr_ids,
        //                     &alias_def.arg_constraints,
        //                 ) {
        //                     return Err(preset_err);
        //                 };
        //
        //                 // Checking if say, ch: char, aligns with each condition given. Where, is
        //                 // IsEmpty was used it would not be a `Collection` type, but if
        //                 // `IsWhitespace` was used it would be fine
        //                 // for inner_cond_expr_id in &alias_def.conds {
        //
        //                 //WARN: I think this is wrong
        //                 for inner_cond_expr_id in &alias_def.conds {
        //                     if let Err(mut preset_err) =
        //                         self.check_cond(parent_ty_id, parent_span, *inner_cond_expr_id)
        //                     {
        //                         preset_errs.append(&mut preset_err);
        //                     }
        //                 }
        //
        //                 // Checking the parameter's type constraints, if present, against the
        //                 // corresponding argument
        //                 for (i, param) in alias_def.params.iter().enumerate() {
        //                     let constraint_flags =
        //                         match self.compiler.types[param.type_id ].ty {
        //                             Type::Constrained(constraint) => constraint,
        //                             // Type::BuiltinType(builtin_type) => todo!(),
        //                             // Type::Struct(struct_def) => todo!(),
        //                             // Type::Enum(enum_def) => todo!(),
        //                             // Type::Func(func_def) => todo!(),
        //                             // Type::Alias(alias_def) => todo!(),
        //                             // Type::TypeDef(type_def) => todo!(),
        //                             // Type::Unknown => todo!(),
        //                             _ => unimplemented!("not yet"),
        //                         };
        //
        //                     let arg_expr_id = arg_expr_ids[i];
        //                     let arg_ty_id = &self.compiler.exprs[arg_expr_id ].type_id;
        //
        //                     dbg!(&self.compiler.types[arg_ty_id ]);
        //                     panic!();
        //
        //                     if let Err(preset_err) = constraints::check_type_constraint(
        //                         self.compiler,
        //                         *arg_ty_id,
        //                         parent_span,
        //                         cond_expr.span,
        //                         &mut Vec::new(),
        //                         constraint_flags,
        //                     ) {
        //                         preset_errs.push(preset_err);
        //                     }
        //                 }
        //
        //                 if !preset_errs.is_empty() {
        //                     return Err(preset_errs);
        //                 }
        //
        //                 Ok(())
        //             }
        //             Type::BuiltinType(builtin_type) => todo!(),
        //             Type::Struct(struct_def) => todo!(),
        //             Type::Enum(enum_def) => todo!(),
        //             Type::TypeDef(type_def) => todo!(),
        //             Type::Unknown => todo!(),
        //             Type::Constrained(type_constraint) => todo!(),
        //         }
        //     }
        //     // Ok
        //     ExprHir::Var(sym_id) => {
        //         let sym = &self.compiler.symbols[sym_id ];
        //         match sym.kind {
        //             SymbolKind::Type(type_id) => match &self.compiler.types[type_id ].ty
        //             {
        //                 Type::Func(func_def) => {
        //                     // Anything used in a condition must return a boolean
        //                     let ret_type = &self.compiler.types[func_def.ret_type ].ty;
        //
        //                     if let Type::BuiltinType(BuiltinType::Bool) = ret_type {
        //                         // self.check_arg_constraints(
        //                         //     cond_expr_id,
        //                         //     &[],
        //                         //     &func_def.arg_constraints,
        //                         //     func_def.kind,
        //                         // )?;
        //
        //                         match constraints::check_type_constraint(
        //                             self.compiler,
        //                             parent_ty_id,
        //                             parent_span,
        //                             cond_expr.span,
        //                             &mut Vec::new(),
        //                             func_def.type_constraints,
        //                         ) {
        //                             Ok(_) => Ok(()),
        //                             Err(preset_err) => Err(vec![preset_err]),
        //                         }
        //                     } else {
        //                         let msg =
        //                             "Every value within a condition must be a boolean".to_string();
        //                         Err(vec![SemanticError::General(msg, vec![cond_expr.span])])
        //                     }
        //
        //                     // We need to know if it matches the type given, but only if we are
        //                     // matching against something that isn't an alias or another function
        //                     // since that of course wouldn't match.
        //                 }
        //                 Type::BuiltinType(builtin_type) => todo!(),
        //                 Type::Struct(struct_def) => todo!(),
        //                 Type::Unknown => todo!(),
        //                 Type::Enum(enum_def) => todo!(),
        //                 Type::Alias(alias_def) => todo!(),
        //                 Type::TypeDef(type_def) => unreachable!("Not syntactically possible"),
        //                 Type::Constrained(type_constraint) => todo!(),
        //             },
        //             SymbolKind::Val(_) | SymbolKind::Unknown => {
        //                 let type_id = &self.compiler.values[cond_expr.val_id ].type_id;
        //                 let ty = &self.compiler.types[type_id ].ty;
        //
        //                 if let Type::BuiltinType(BuiltinType::Bool) = ty {
        //                     Ok(())
        //                 } else {
        //                     // Confusing?
        //                     let msg = "Top level values used within type constraint blocks must evaluate to a boolean"
        //                         .to_string();
        //                     Err(vec![SemanticError::General(msg, vec![cond_expr.span])])
        //                 }
        //             }
        //             SymbolKind::Module(module_id) => {}
        //         }
        //     }
        //     // Only `BinaryExpr` can actually evaluate to a boolean here, just re-using the logic
        //     ExprHir::BinaryExpr { .. } | ExprHir::Unary { .. } | ExprHir::Default(..) => {
        //         let type_id = &self.compiler.values[cond_expr.val_id ].type_id;
        //         let ty = &self.compiler.types[type_id ].ty;
        //
        //         if let Type::BuiltinType(BuiltinType::Bool) = ty {
        //             Ok(())
        //         } else {
        //             Err(vec![SemanticError::General(
        //                 "Top level expressions used within type constraint blocks must evaluate to a boolean".to_string(),
        //                 vec![cond_expr.span],
        //             )])
        //         }
        //     }
        //     ExprHir::Val(_) => {
        //         let type_id = &self.compiler.values[cond_expr.val_id ].type_id;
        //         let ty = &self.compiler.types[type_id ].ty;
        //
        //         if let Type::BuiltinType(BuiltinType::Bool) = ty {
        //             Ok(())
        //         } else {
        //             let msg =
        //                 "Top level values used within type constraint blocks must evaluate to a boolean".to_string();
        //             Err(vec![SemanticError::General(msg, vec![cond_expr.span])])
        //         }
        //     }
        // }
        todo!()
    }

    // This should really send signals
    fn check_directive(
        &self,
        type_id: TypeId,
        parent_span: SourceSpan,
        active_span: SourceSpan,
        spanned_directive: &SpannedContainerRef<Directive>,
        visited: &mut Vec<TypeId>,
        env: &ResolverEnv,
        // Making this vec makes error messages painful depending on which message failed, so it
        // needs some signal to say to stop going.
    ) -> Result<(), Option<PresetErr>> {
        match &self.compiler.types[type_id].ty {
            Type::Struct(struct_def) => {
                visited.push(type_id);

                // No cross module reporting so all messages are shallow in spanning
                for member_id in &struct_def.fields {
                    let field = self.compiler.get_field(*member_id);
                    // Checking if one of it's variants are self referencing, or if the type from
                    // the last call stack, possibly a tuple, is self referencing the current
                    // struct.
                    if visited.contains(&field.type_id) {
                        if spanned_directive.inner.has_restrictions() {
                            return Err(Some(PresetErr::CircularDirective {
                                sp_fmtted_parent: SpannedContainer::new(
                                    Formatted::Struct,
                                    struct_def.name_span,
                                ),
                                sp_directive: spanned_directive.into_owned(),
                                err_ty_span: field.name_span,
                            }));
                        }

                        continue;
                    }

                    let ty = &self.compiler.types[field.type_id].ty;
                    if !matches!(ty, Type::BuiltinTypeInfo(_)) {
                        visited.push(field.type_id);
                    }

                    //TODO: Needs to separate path and errors depending on fjailfjialfn path
                    self.check_directive(
                        field.type_id,
                        struct_def.name_span,
                        field.name_span,
                        spanned_directive,
                        visited,
                        env,
                    )?;
                }

                Ok(())
            }
            Type::Enum(enum_def) => {
                visited.push(type_id);

                for member_id in &enum_def.variants {
                    let variant = self.compiler.get_variant(*member_id);
                    if let Some(inner) = variant.type_id {
                        // Checking if one of it's variants are self referencing, or if the type we
                        // just came from, possibly a tuple, is referring to itself from a
                        // different context.
                        if visited.contains(&inner) {
                            if spanned_directive.inner.has_restrictions() {
                                return Err(Some(PresetErr::CircularDirective {
                                    sp_fmtted_parent: SpannedContainer::new(
                                        Formatted::Enum,
                                        enum_def.name_span,
                                    ),
                                    sp_directive: spanned_directive.into_owned(),
                                    err_ty_span: variant.name_span,
                                }));
                            }

                            continue;
                        }

                        let ty = &self.compiler.types[inner].ty;
                        if !matches!(ty, Type::BuiltinTypeInfo(_)) {
                            visited.push(inner);
                        }

                        self.check_directive(
                            inner,
                            enum_def.name_span,
                            variant.name_span,
                            spanned_directive,
                            visited,
                            env,
                        )?;
                    }
                }

                Ok(())
            }
            Type::BuiltinTypeInfo(builtin_info) => {
                match &builtin_info.ty {
                    BuiltinType::List(type_id) | BuiltinType::Set(type_id) => self.check_directive(
                        *type_id,
                        parent_span,
                        active_span,
                        spanned_directive,
                        visited,
                        env,
                    ),
                    BuiltinType::Map(key_id, val_id) => {
                        // This looks weird...
                        self.check_directive(
                            *key_id,
                            parent_span,
                            active_span,
                            spanned_directive,
                            visited,
                            env,
                        )?;
                        self.check_directive(
                            *val_id,
                            parent_span,
                            active_span,
                            spanned_directive,
                            visited,
                            env,
                        )
                    }
                    BuiltinType::Tuple(elements) => {
                        visited.push(type_id);
                        for element in elements {
                            if visited.contains(&*element) {
                                if spanned_directive.inner.has_restrictions() {
                                    return Err(Some(PresetErr::CircularDirective {
                                        sp_fmtted_parent: SpannedContainer::new(
                                            Formatted::Tuple,
                                            parent_span,
                                        ),
                                        sp_directive: spanned_directive.into_owned(),
                                        err_ty_span: active_span,
                                    }));
                                }
                            }

                            let ty = &self.compiler.types[*element].ty;
                            match ty {
                                Type::BuiltinTypeInfo(_) => (),
                                _ => visited.push(*element),
                            }

                            self.check_directive(
                                *element,
                                active_span,
                                parent_span,
                                spanned_directive,
                                visited,
                                env,
                            )?;
                        }

                        Ok(())
                    }
                    // Need a function where it obtains type constraints given the recursive types
                    // since shallow checks accept more than proven
                    builtin_ty => {
                        let ty_boundaries = builtin_ty.kind().boundaries();
                        let directive_boundaries = spanned_directive.inner.boundaries();

                        if !directive_boundaries.overlaps(ty_boundaries) {
                            return Err(Some(PresetErr::UnsupportedDirective {
                                sp_directive: spanned_directive.into_owned(),
                                sym_span: active_span,
                            }));
                        }

                        Ok(())
                    }
                }
            }
            Type::Alias(alias_def) => {
                let alias_constraints = alias_def.ty_constraints;
                let arg_constraints = spanned_directive.inner.boundaries();

                if !arg_constraints.overlaps(alias_constraints) {
                    return Err(Some(PresetErr::TypeBoundaryBoundConflict {
                        inferred: alias_constraints,
                        conflicting: arg_constraints,
                        spans: vec![spanned_directive.span, active_span],
                    }));
                }

                Ok(())
            }
            // I kinda would rather it just did nothing rather than report
            Type::Unknown => Err(None),
            Type::Func(_) => {
                let core_msg = "Functions can only be placed within condition blocks".to_string();

                //NOTE: I don't THINK this warrants a code?
                let src_diag = SourceDiagnostic::basic_builder(
                    None,
                    DiagnosticLevel::Error,
                    core_msg,
                    env.region.path_id,
                    active_span,
                );
                Err(Some(PresetErr::General(src_diag)))
            }
            // Function.
            Type::TypeDef(_) => {
                unreachable!("Not syntactically possible")
            }
            Type::Deferred(deferred_ty_id) => self.check_directive(
                *deferred_ty_id,
                parent_span,
                active_span,
                spanned_directive,
                visited,
                env,
            ),
            Type::Boundaries(current_boundaries) => {
                let directive_boundaries = spanned_directive.inner.boundaries();

                if !directive_boundaries.overlaps(*current_boundaries) {
                    return Err(Some(PresetErr::TypeBoundaryBoundConflict {
                        inferred: *current_boundaries,
                        conflicting: directive_boundaries,
                        spans: vec![spanned_directive.span, active_span],
                    }));
                }
                panic!("Hi");

                Ok(())
            }
        }
    }
    // Maybe alias specific method not needed since alias is just a wrapper for calling multiple
    // functions
    // Maybe we should have continue on success so that the cconstraint can immediately be reported
    // rather than inlined same code

    /// Returns a tuple with the collected errors, and a boolean to decide error reporting
    /// continuation on `Err`
    // TODO: Can be simplified eventually
    // Need to redo this so that it accounts for the constraints, not just builtin types
    fn check_arg_constraints(
        &self,
        parent_ty_id: TypeId,
        parent_span: SourceSpan,
        cond_expr_id: ExprId,
        expr_id_args: &[ExprId],
        arg_constraints: &[ArgConstraint],
        // Maybe a more explicit state of Recoverabilitiy as an enum of some sort would be better
        // eventually or at least a wrapper
    ) -> Result<(), Vec<PresetErr>> {
        let mut preset_errs: Vec<PresetErr> = Vec::new();
        todo!();
        // for constraint in arg_constraints {
        //     match constraint {
        //         ArgConstraint::ArgCount(arg_count_constraint) => {
        //             let found_arg_count = expr_id_args.len() as u32;
        //
        //             if found_arg_count != *arg_count_constraint {
        //                 let mut spans: Vec<SourceSpan> = expr_id_args
        //                     .iter()
        //                     .map(|ex_id| self.compiler.exprs[ex_id ].span)
        //                     .collect();
        //
        //                 if spans.is_empty() {
        //                     let cond_span = &self.compiler.exprs[cond_expr_id ].span;
        //                     spans.push(*cond_span);
        //                 }
        //
        //                 preset_errs.push(SemanticError::ArgCountMismatch(
        //                     *constraint,
        //                     found_arg_count,
        //                     spans,
        //                 ));
        //
        //                 // Going further would likely lead to misleading errors
        //                 return Err(preset_errs);
        //             }
        //         }
        //         ArgConstraint::MatchingArgumentTypes => {
        //             // If no arguments then it innately succeeds
        //             let req_expr_id = match expr_id_args.first() {
        //                 Some(id) => id,
        //                 None => continue,
        //             };
        //
        //             let req_type_id = self.compiler.exprs[req_expr_id ].type_id;
        //
        //             for expr_id in expr_id_args.iter().skip(1) {
        //                 let other_type_id = self.compiler.exprs[expr_id ].type_id;
        //
        //                 if req_type_id != other_type_id {
        //                     let req_span = self.compiler.exprs[req_expr_id ].span;
        //                     let other_span = self.compiler.exprs[expr_id ].span;
        //
        //                     let ty = &self.compiler.types[req_type_id ].ty;
        //
        //                     preset_errs.push(SemanticError::FuncConstraintMismatch(
        //                         *constraint,
        //                         ty.to_fmt(),
        //                         vec![req_span, other_span],
        //                     ));
        //                 }
        //             }
        //         }
        //         ArgConstraint::Numeric => {
        //             for expr_id in expr_id_args {
        //                 let type_id = &self.compiler.exprs[expr_id ].type_id;
        //                 let ty = &self.compiler.types[type_id ].ty;
        //
        //                 if let Type::BuiltinType(builtin_ty) = ty {
        //                     if !builtin_ty.kind().is_numeric() {
        //                         let span = self.compiler.exprs[expr_id ].span;
        //
        //                         preset_errs.push(SemanticError::FuncConstraintMismatch(
        //                             *constraint,
        //                             ty.to_fmt(),
        //                             vec![span],
        //                         ));
        //                     }
        //                 }
        //             }
        //         }
        //         ArgConstraint::Integer => {
        //             for expr_id in expr_id_args {
        //                 let type_id = &self.compiler.exprs[expr_id ].type_id;
        //                 let ty = &self.compiler.types[type_id ].ty;
        //
        //                 if let Type::BuiltinType(builtin_ty) = ty {
        //                     if !builtin_ty.kind().is_integer() {
        //                         let span = self.compiler.exprs[expr_id ].span;
        //
        //                         preset_errs.push(SemanticError::FuncConstraintMismatch(
        //                             *constraint,
        //                             ty.to_fmt(),
        //                             vec![span],
        //                         ));
        //                     }
        //                 }
        //             }
        //         }
        //         ArgConstraint::Float => {
        //             for expr_id in expr_id_args {
        //                 let type_id = &self.compiler.exprs[expr_id ].type_id;
        //                 let ty = &self.compiler.types[type_id ].ty;
        //
        //                 if let Type::BuiltinType(builtin_ty) = ty {
        //                     if !builtin_ty.kind().is_float() {
        //                         let span = self.compiler.exprs[expr_id ].span;
        //
        //                         preset_errs.push(SemanticError::FuncConstraintMismatch(
        //                             *constraint,
        //                             ty.to_fmt(),
        //                             vec![span],
        //                         ));
        //                     }
        //                 }
        //             }
        //         }
        //         ArgConstraint::Str => {
        //             for expr_id in expr_id_args {
        //                 let type_id = &self.compiler.exprs[expr_id ].type_id;
        //                 let ty = &self.compiler.types[type_id ].ty;
        //
        //                 if let Type::BuiltinType(builtin_ty) = ty {
        //                     if builtin_ty.kind() != BuiltinTypeKind::Str {
        //                         let span = self.compiler.exprs[expr_id ].span;
        //
        //                         preset_errs.push(SemanticError::FuncConstraintMismatch(
        //                             *constraint,
        //                             ty.to_fmt(),
        //                             vec![span],
        //                         ));
        //                     }
        //                 }
        //             }
        //         }
        //         ArgConstraint::CharacterMappable => {
        //             for expr_id in expr_id_args {
        //                 let type_id = &self.compiler.exprs[expr_id ].type_id;
        //                 let ty = &self.compiler.types[type_id ].ty;
        //
        //                 if let Type::BuiltinType(builtin_ty) = ty {
        //                     if !builtin_ty.kind().is_character_mappable() {
        //                         let span = self.compiler.exprs[expr_id ].span;
        //
        //                         preset_errs.push(SemanticError::FuncConstraintMismatch(
        //                             *constraint,
        //                             ty.to_fmt(),
        //                             vec![span],
        //                         ));
        //                     }
        //                 }
        //             }
        //         }
        //         ArgConstraint::Bool => {
        //             for expr_id in expr_id_args {
        //                 let type_id = &self.compiler.exprs[expr_id ].type_id;
        //                 let ty = &self.compiler.types[type_id ].ty;
        //
        //                 if let Type::BuiltinType(builtin_ty) = ty {
        //                     if builtin_ty.kind() != BuiltinTypeKind::Bool {
        //                         let span = self.compiler.exprs[expr_id ].span;
        //
        //                         preset_errs.push(SemanticError::FuncConstraintMismatch(
        //                             *constraint,
        //                             ty.to_fmt(),
        //                             vec![span],
        //                         ));
        //                     }
        //                 }
        //             }
        //         }
        //         ArgConstraint::Variadic | ArgConstraint::DynType => (),
        //         ArgConstraint::Comparable => {
        //             for expr_id in expr_id_args {
        //                 let type_id = &self.compiler.exprs[expr_id ].type_id;
        //                 // let ty = &self.compiler.types[type_id ].ty;
        //
        //                 let expr_span = self.compiler.exprs[expr_id ].span;
        //                 let cond_span = self.compiler.exprs[cond_expr_id ].span;
        //
        //                 if let Err(preset_err) = constraints::check_type_constraint(
        //                     self.compiler,
        //                     *type_id,
        //                     expr_span,
        //                     cond_span,
        //                     &mut Vec::new(),
        //                     TypeBoundaryFlags::new(TypeBoundary::Comparable.to_u64()),
        //                 ) {
        //                     preset_errs.push(preset_err);
        //                 };
        //                 todo!("TOdol");
        //
        //                 // dbg!(ty);
        //                 // match ty {
        //                 //     Type::BuiltinType(builtin_ty) => {
        //                 //         if !builtin_ty.kind().is_comparable() {
        //                 //             let span = self.compiler.exprs[expr_id ].span;
        //                 //
        //                 //             preset_errs.push(SemanticError::FuncConstraintMismatch(
        //                 //                 *constraint,
        //                 //                 ty.to_fmt(),
        //                 //                 vec![span],
        //                 //             ));
        //                 //         }
        //                 //     }
        //                 //     Type::Struct(struct_def) => todo!(),
        //                 //     Type::Enum(enum_def) => todo!(),
        //                 //     Type::Func(func_def) => todo!(),
        //                 //     Type::Alias(alias_def) => todo!(),
        //                 //     Type::TypeDef(type_def) => todo!(),
        //                 //     Type::Constrained(type_constraint_flags) => todo!(),
        //                 //     Type::Unknown => todo!(),
        //                 // }
        //             }
        //         }
        //         // Should be more constrsaint based
        //         ArgConstraint::SameTypeAsSelf => {
        //             let parent_ty = &self.compiler.types[parent_ty_id ];
        //             for expr_id in expr_id_args.iter().skip(1) {
        //                 let other_ty_id = self.compiler.exprs[expr_id ].type_id;
        //                 let types = &self.compiler.types;
        //                 let ty = &types[parent_ty_id ];
        //                 dbg!(ty);
        //
        //                 panic!();
        //                 if parent_ty_id != other_ty_id {
        //                     let other_span = self.compiler.exprs[expr_id ].span;
        //                     let msg = "Must be the same type as `self`".to_string();
        //
        //                     preset_errs
        //                         .push(SemanticError::General(msg, vec![parent_span, other_span]));
        //                 }
        //             }
        //         }
        //     }
        // }
        //
        // if !preset_errs.is_empty() {
        //     return Err(preset_errs);
        // }
        //
        // Ok(())
    }
}
