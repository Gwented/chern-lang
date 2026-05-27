use chrn_utils::{
    id_types::{AstId, ExprId, InternedId, ModuleId, SymbolId, TypeId, ValueId},
    inner_args::{InnerArgs, SpannedInnerArgs},
    intern::Intern,
    types::{
        builtins::{BuiltinType, BuiltinTypeKind},
        type_constraints::{TypeConstraint, TypeConstraintFlags},
    },
};
use common::{
    chrn_settings::ChrnSettings,
    fmter::{Formattable, Formatted},
    reporter::diagnostic::Diagnostic,
    span::Span,
};

use crate::{
    modules::Module,
    parser::ast::{
        AbstractAlias, AbstractEnum, AbstractStruct, AbstractTypeDef, AbstractVar, AstInfo, Expr,
        Item, SpannedExpr, UnaryOp,
    },
    script_compiler::ScriptCompiler,
    semantic::{
        constraints::{self, ArgConstraint},
        error::{MathError, SemanticError},
        evaluator,
        representation::{
            AliasDef, ExprHir, FuncDef, FuncKind, PossibleMember, ResolvedExpr, Symbol, SymbolKind,
            Type,
        },
        scopes::ScopeType,
        semantic_reporter::SemanticReporter,
    },
};

pub struct ConstraintResolver<'a> {
    ast_info: &'a AstInfo,
    interner: &'a Intern,
    compiler: &'a mut ScriptCompiler,
    // We reward hack here
    /// If module and ast ids are not the same, this will break. (Will change(Right?))
    current_mod: ModuleId,
    reporter: SemanticReporter<'a>,
}

impl<'a> ConstraintResolver<'a> {
    pub fn new(
        settings: &'a ChrnSettings,
        ast_info: &'a AstInfo,
        interner: &'a Intern,
        current_mod: ModuleId,
        compiler: &'a mut ScriptCompiler,
    ) -> ConstraintResolver<'a> {
        ConstraintResolver {
            ast_info,
            interner,
            current_mod,
            compiler,
            reporter: SemanticReporter::new(settings, interner),
        }
    }

    pub fn resolve(&mut self) -> Result<(), Vec<Diagnostic>> {
        // I don't think dependency tracking can be avoided here
        for (id, item) in self.ast_info.items.iter().enumerate() {
            let ast_id = AstId::new(id as u32);
            // Maybe alias is solved first?

            match item {
                Item::TypeDef(abs_typedef) => {
                    _ = self.resolve_typedef(abs_typedef, ast_id);
                    // for err in &self.reporter.err_vec {
                    //     println!("{}", err.fmtted_diag);
                    // }
                }
                Item::Struct(abs_struct) => {
                    _ = self.resolve_struct(abs_struct, ast_id);
                    // for err in &self.reporter.err_vec {
                    //     println!("{}", err.fmtted_diag);
                    // }
                }
                Item::Enum(abs_enum) => {
                    _ = self.resolve_enum(abs_enum, ast_id);
                    // for err in &self.reporter.err_vec {
                    //     println!("{}", err.fmtted_diag);
                    // }
                }
                Item::Alias(abs_alias) => {
                    _ = self.resolve_alias(abs_alias, ast_id);
                    // for err in &self.reporter.err_vec {
                    //     println!("{}", err.fmtted_diag);
                    // }
                    // todo!("Todol");
                }
                Item::Var(abs_var) => {
                    _ = self.resolve_var(abs_var, ast_id);
                    // for err in &self.reporter.err_vec {
                    //     println!("{}", err.fmtted_diag);
                    // }
                    // todo!("Todol");
                }
                Item::Config(abs_cfg) => todo!(),
            }
        }

        if !self.reporter.err_vec.is_empty() {
            let mut diags = Vec::new();
            diags.append(&mut self.reporter.err_vec);

            return Err(diags);
        }

        Ok(())
    }

    // Needs:
    //
    // Maybe we can privacy check here so semantic information is still present, and the error is
    // also present
    fn resolve_var(&mut self, abs_var: &AbstractVar, ast_id: AstId) -> Result<(), ()> {
        // Not sure what this might need checked yet other than privacy
        let scope_id = self
            .compiler
            .extract_scope_id(ScopeType::Neutral, self.current_mod);
        let table = &self.compiler.get_scope(scope_id).scope.table;
        let sym_id = table.ast_to_sym[&ast_id];

        let symbol = &self.compiler.symbols[sym_id.id as usize];

        let val_info = self.compiler.get_var(sym_id);
        let ty = &self.compiler.types[val_info.type_id.id as usize].ty;

        // Not currently syntactically possible to make runtime expressions
        if let Type::Unknown = ty {
            todo!("Unknowned");
        }

        Ok(())
    }

    fn resolve_typedef(&mut self, abs_typedef: &AbstractTypeDef, ast_id: AstId) -> Result<(), ()> {
        let scope_id = self
            .compiler
            .extract_scope_id(ScopeType::Var, self.current_mod);
        let table = &self.compiler.get_scope(scope_id).scope.table;
        let sym_id = table.ast_to_sym[&ast_id];

        let module = &self.compiler.mods[self.current_mod.id];
        let type_def = self.compiler.get_typedef(sym_id);
        let ty_info = &self.compiler.types[type_def.type_id.id as usize];

        // Checking if condition is valid for the given type
        // Using the Ast node's condition so that the span information is not lost
        let ty_span = abs_typedef.spanned_ty_expr.span;
        for (i, cond_expr) in type_def.conds.iter().enumerate() {
            let ast_span = &abs_typedef.conds[i].span;

            match &ty_info.ty {
                Type::Struct(_) | Type::Enum(_) => {
                    //NOTE: Would be better as a note
                    // The issue with allowing this is if it were not restricted, and p: Person was
                    // typed, that would mean that "other_p: Person" inside the same var-> would
                    // need to align with whatever conditions or arguments given, which would be
                    // problematic. Hence, it just has to be a shallowly applied argument instead.
                    let msg = "Cannot give a `var->` defined type a condition when it has a `struct` or `enum` type, define\nthis within `nest->`";

                    self.reporter.report_spanned(
                        msg,
                        None,
                        &[ast_span.clone()],
                        &self.compiler.mods[self.current_mod.id as usize]
                            .src_metadata
                            .as_ref()
                            .expect("core should not be resolved"),
                    );
                }
                _ => (),
            }

            if let Err(sem_errs) = self.check_cond(type_def.type_id, ty_span, *cond_expr) {
                for err in sem_errs {
                    self.reporter.report_semantic(
                        err,
                        module
                            .src_metadata
                            .as_ref()
                            .expect("core should not be resolved"),
                    );
                }
            }
        }

        for spanned_arg in &abs_typedef.args {
            match &ty_info.ty {
                Type::Struct(_) | Type::Enum(_) => {
                    if spanned_arg.arg.has_restrictions() {
                        let sem_err =
                            SemanticError::VagueArg(spanned_arg.arg, vec![spanned_arg.span]);

                        self.reporter.report_semantic(
                            sem_err,
                            &self.compiler.mods[self.current_mod.id as usize]
                                .src_metadata
                                .as_ref()
                                .expect("core should not be resolved"),
                        );
                    }
                }
                _ => (),
            }

            if let Err(sem_err) = self.check_type_arg(
                type_def.type_id,
                ty_span,
                module,
                &spanned_arg,
                &mut Vec::new(),
            ) {
                self.reporter.report_semantic(
                    sem_err,
                    module
                        .src_metadata
                        .as_ref()
                        .expect("core should not be resolved"),
                );
            }
        }

        Ok(())
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
    fn resolve_alias(&mut self, abs_alias: &AbstractAlias, ast_id: AstId) -> Result<(), ()> {
        let scope_id = self
            .compiler
            .extract_scope_id(ScopeType::Neutral, self.current_mod);
        let table = &self.compiler.get_scope(scope_id).scope.table;
        let sym_id = table.ast_to_sym[&ast_id];

        //TODO: Need to typecheck based off of the conditional expressions found

        // let alias_type_id = self.compiler.get_type_id(sym_id);
        let alias_def = self.compiler.get_alias(sym_id);
        let alias_type_id = self.compiler.get_type_id(sym_id);

        // TODO: This should now just check instead of infer

        // let mut found_constraints: Vec<Option<TypeConstraintFlags>> =
        //     vec![None; alias_def.params.len()];
        //
        // for (i, param) in alias_def.params.iter().enumerate() {
        //     let current_constraints_opt = &mut found_constraints[i];
        //
        //     for cond_expr_id in alias_def.conds.iter().copied() {
        //         let param_span = abs_alias.params[i].name_span;
        //
        //         let name_id = self.compiler.symbols[param.sym_id.id as usize].name_id;
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
        //                         let cond_expr = &self.compiler.exprs[cond_expr_id.id as usize];
        //
        //                         let sem_err = SemanticError::TypeConstraintBoundConflict(
        //                             *current,
        //                             new_constraints,
        //                             vec![param_span, cond_expr.span],
        //                         );
        //
        //                         let module = &self.compiler.mods[self.current_mod.id];
        //                         self.reporter.report_semantic(
        //                             sem_err,
        //                             &module
        //                                 .src_metadata
        //                                 .as_ref()
        //                                 .expect("core should not be resolved"),
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
        // let mut ty_constraint: Option<TypeConstraint> = None;
        // for (i, sp_arg) in abs_alias.args.iter().enumerate() {
        //     let param_span = abs_alias.params[i].name_span;
        //     match self.infer_type_constraint_from_arg(sp_arg, param_span) {
        //         Ok(constraint_opt) => {
        //             todo!("Constraining contraint check of cocnsctraint");
        //         }
        //         Err(sem_err) => {
        //             let module = &self.compiler.mods[self.current_mod.id];
        //             self.reporter.report_semantic(
        //                 sem_err,
        //                 &module
        //                     .src_metadata
        //                     .as_ref()
        //                     .expect("core should not be resolved"),
        //             );
        //         }
        //     }
        // }

        // FIX: Ok so maybe we can keep both systems to where, if it's constrained, check
        // constraints, otherwise, keep the same concrete type checks with builtins

        // Only the type of functions used matter if they depend on self.
        let alias_def = self.compiler.get_alias_mut(sym_id);
        // alias_def.ty_constraints = found_constraints.iter().filter_map(|c| c.is_some());

        // Currently assuming that if we see none here it's fine since technically, you could
        // declare a parameter and have it just not be used and never face any type errors.

        let alias_def = self.compiler.get_alias(sym_id);
        // Need a system where it takes a local variable, looks through each expression, sees if
        // it's used, then if so attempts to assign the constraint to the used argument.

        let module = &self.compiler.mods[self.current_mod.id];
        // NO
        unimplemented!("Stop using the alias please");

        // NOTE: Small issue here is that when we check an alias, and it has an error, it's
        // emitted. But then if we have something that USES the alias, it also gets that error.
        let sym_span = self.ast_info.get_sym_span(ast_id);
        for cond_expr_id in &alias_def.conds {
            if let Err(sem_errs) = self.check_cond(alias_type_id, sym_span, *cond_expr_id) {
                for err in sem_errs {
                    self.reporter.report_semantic(
                        err,
                        module
                            .src_metadata
                            .as_ref()
                            .expect("core should not be resolved"),
                    );
                }
            }
        }

        // todo!("End");
        Ok(())
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
    //     param_span: Span,
    // ) -> Option<TypeConstraintFlags> {
    //     let expr = &self.compiler.exprs[expr_id.id as usize];
    //     match &expr.expr_hir {
    //         ExprHir::Val(val_id) => {
    //             panic!("Val id");
    //         }
    //         ExprHir::Var(sym_id) => {
    //             let symbol = &self.compiler.symbols[sym_id.id as usize];
    //
    //             match &self.compiler.symbols[sym_id.id as usize].kind {
    //                 SymbolKind::Type(type_id) => match &self.compiler.types[type_id.id as usize].ty
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
    //                     let symbol = &self.compiler.symbols[sym_id.id as usize];
    //                     if param_name_id == symbol.name_id {
    //                         let type_id = self.compiler.exprs[expr_id.id as usize].type_id;
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
    //                 let expr_hir = &self.compiler.exprs[arg_expr_id.id as usize].expr_hir;
    //                 if let ExprHir::Var(sym_id) = expr_hir {
    //                     let sym = &self.compiler.symbols[sym_id.id as usize];
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
    //             let ty = &self.compiler.types[expr.type_id.id as usize].ty;
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
    //     sp_arg: &SpannedInnerArgs,
    //     param_span: Span,
    // ) -> Result<Option<TypeConstraint>, SemanticError> {
    //     todo!()
    // }

    // //NOTE: The reason this would need to look at the struct again would be because it is iterating
    // // through items despite there already being a known struct id, which could be prevented if the
    // // struct id itself was passed, but then the loop would iterate over everything by default
    // // which seems bad if they're just builtins etc.

    // Needs:
    //
    fn resolve_struct(&mut self, abs_struct: &AbstractStruct, ast_id: AstId) -> Result<(), ()> {
        let scope_id = self
            .compiler
            .extract_scope_id(ScopeType::Nest, self.current_mod);
        let table = &self.compiler.get_scope(scope_id).scope.table;

        //TODO: global condition and argument setting.
        //field arg and cond settings.
        //same for enums.

        let sym_id = table.ast_to_sym[&ast_id];
        let struct_def = self.compiler.get_struct(sym_id);

        let module = &self.compiler.mods[self.current_mod.id];

        // Glob conds
        for (i, field) in struct_def.fields.iter().enumerate() {
            let ty_span = abs_struct.fields[i].spanned_ty_expr.span;
            for cond_expr in &struct_def.glob_conds {
                if let Err(sem_errs) = self.check_cond(field.type_id, ty_span, *cond_expr) {
                    for err in sem_errs {
                        self.reporter.report_semantic(
                            err,
                            module
                                .src_metadata
                                .as_ref()
                                .expect("core should not be resolved"),
                        );
                    }
                }
            }
        }

        // Field conds
        for (i, field) in struct_def.fields.iter().enumerate() {
            let ty_span = abs_struct.fields[i].spanned_ty_expr.span;
            for cond_expr in &field.conds {
                if let Err(sem_errs) = self.check_cond(field.type_id, ty_span, *cond_expr) {
                    for err in sem_errs {
                        self.reporter.report_semantic(
                            err,
                            module
                                .src_metadata
                                .as_ref()
                                .expect("core should not be resolved"),
                        );
                    }
                }
            }
        }

        // Glob args
        for (i, field) in struct_def.fields.iter().enumerate() {
            let ty_span = abs_struct.fields[i].spanned_ty_expr.span;
            for spanned_arg in &abs_struct.glob_args {
                if let Err(sem_err) =
                    self.check_type_arg(field.type_id, ty_span, module, spanned_arg, &mut vec![])
                {
                    self.reporter.report_semantic(
                        sem_err,
                        module
                            .src_metadata
                            .as_ref()
                            .expect("core should not be resolved"),
                    );
                }
            }
        }

        // Field args
        for (i, field) in struct_def.fields.iter().enumerate() {
            let abs_field = &abs_struct.fields[i];
            let ty_span = abs_field.spanned_ty_expr.span;
            for spanned_arg in &abs_field.args {
                if let Err(sem_err) =
                    self.check_type_arg(field.type_id, ty_span, module, spanned_arg, &mut vec![])
                {
                    self.reporter.report_semantic(
                        sem_err,
                        module
                            .src_metadata
                            .as_ref()
                            .expect("core should not be resolved"),
                    );
                }
            }
        }

        Ok(())
    }

    fn resolve_enum(&mut self, abs_enum: &AbstractEnum, ast_id: AstId) -> Result<(), ()> {
        let scope_id = self
            .compiler
            .extract_scope_id(ScopeType::Nest, self.current_mod);
        let table = &self.compiler.get_scope(scope_id).scope.table;

        let sym_id = table.ast_to_sym[&ast_id];

        let enum_def = &self.compiler.get_enum(sym_id);
        let module = &self.compiler.mods[self.current_mod.id];

        // Glob conds
        for (i, variant) in enum_def.variants.iter().enumerate() {
            if let Some(inner_id) = variant.type_id {
                let ty_span = abs_enum.variants[i]
                    .ty_expr
                    .as_ref()
                    .expect("Already checked")
                    .span;

                for cond_expr in &enum_def.glob_conds {
                    if let Err(sem_errs) = self.check_cond(inner_id, ty_span, *cond_expr) {
                        for err in sem_errs {
                            self.reporter.report_semantic(
                                err,
                                module
                                    .src_metadata
                                    .as_ref()
                                    .expect("core should not be resolved"),
                            );
                        }
                    }
                }
            }
        }

        // Variant conds
        for (i, variant) in enum_def.variants.iter().enumerate() {
            if let Some(inner_id) = variant.type_id {
                let ty_span = abs_enum.variants[i]
                    .ty_expr
                    .as_ref()
                    .expect("Already checked")
                    .span;

                for cond_expr in &variant.conds {
                    if let Err(sem_errs) = self.check_cond(inner_id, ty_span, *cond_expr) {
                        for err in sem_errs {
                            self.reporter.report_semantic(
                                err,
                                module
                                    .src_metadata
                                    .as_ref()
                                    .expect("core should not be resolved"),
                            );
                        }
                    }
                }
            }
        }

        // Glob args
        for (i, variant) in enum_def.variants.iter().enumerate() {
            if let Some(inner_id) = variant.type_id {
                let ty_span = abs_enum.variants[i]
                    .ty_expr
                    .as_ref()
                    .expect("Just checked")
                    .span;

                for spanned_arg in &abs_enum.glob_args {
                    if let Err(sem_err) =
                        self.check_type_arg(inner_id, ty_span, module, spanned_arg, &mut vec![])
                    {
                        self.reporter.report_semantic(
                            sem_err,
                            module
                                .src_metadata
                                .as_ref()
                                .expect("core should not be resolved"),
                        );
                    }
                }
            }
        }

        // Variant args
        for (i, variant) in enum_def.variants.iter().enumerate() {
            if let Some(inner_id) = variant.type_id {
                let abs_variant = &abs_enum.variants[i];
                let ty_span = abs_variant.ty_expr.as_ref().expect("Just checked").span;

                for spanned_arg in &abs_variant.args {
                    if let Err(sem_err) =
                        self.check_type_arg(inner_id, ty_span, module, spanned_arg, &mut vec![])
                    {
                        self.reporter.report_semantic(
                            sem_err,
                            module
                                .src_metadata
                                .as_ref()
                                .expect("core should not be resolved"),
                        );
                    }
                }
            }
        }

        Ok(())
    }

    // TODO: Type alignment with the used function
    fn check_cond(
        &self,
        parent_ty_id: TypeId,
        parent_span: Span,
        cond_expr_id: ExprId,
    ) -> Result<(), Vec<SemanticError>> {
        // let cond_expr = &self.compiler.exprs[cond_expr_id.id as usize];
        //
        // // if visited.contains(&field.type_id) {
        // //     if spanned_arg.arg.has_restrictions() {
        // //         let name = self.interner.search(symbol.name_id.id as usize);
        // //
        // //         let msg = format!(
        // //             "The type `{name}` cannot have `#{}` applied due to recursively relying on itself satisfying the argument",
        // //             spanned_arg.arg
        // //         );
        // //
        // //         return Err(SemanticError::General(
        // //             msg,
        // //             vec![spanned_arg.span, active_span],
        // //         ));
        // //     }
        // match &cond_expr.expr_hir {
        //     ExprHir::Call(callee_expr_id, arg_expr_ids) => {
        //         let callee = &self.compiler.exprs[callee_expr_id.id as usize];
        //         let ty = &self.compiler.types[callee.type_id.id as usize].ty;
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
        //                 let ret_type = &self.compiler.types[func_def.ret_type.id as usize].ty;
        //
        //                 if let Type::BuiltinType(BuiltinType::Bool) = ret_type {
        //                     // Maybbe tturrnrn in tot a fucntinson
        //                     if let Err(sem_err) = self.check_arg_constraints(
        //                         parent_ty_id,
        //                         parent_span,
        //                         cond_expr_id,
        //                         arg_expr_ids,
        //                         &func_def.arg_constraints,
        //                     ) {
        //                         return Err(sem_err);
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
        //                         Err(sem_err) => return Err(vec![sem_err]),
        //                     }
        //                 } else {
        //                     let msg = "Top level functions or predicates used within type constraint blocks must evaluate to a boolean"
        //                         .to_string();
        //                     Err(vec![SemanticError::General(msg, vec![cond_expr.span])])
        //                 }
        //             }
        //             Type::Alias(alias_def) => {
        //                 let mut sem_errs: Vec<SemanticError> = Vec::new();
        //
        //                 // Checking the arguments given in the call against the arg constraints of
        //                 // the alias
        //                 if let Err(sem_err) = self.check_arg_constraints(
        //                     parent_ty_id,
        //                     parent_span,
        //                     cond_expr_id,
        //                     arg_expr_ids,
        //                     &alias_def.arg_constraints,
        //                 ) {
        //                     return Err(sem_err);
        //                 };
        //
        //                 // Checking if say, ch: char, aligns with each condition given. Where, is
        //                 // IsEmpty was used it would not be a `Collection` type, but if
        //                 // `IsWhitespace` was used it would be fine
        //                 // for inner_cond_expr_id in &alias_def.conds {
        //
        //                 //WARN: I think this is wrong
        //                 for inner_cond_expr_id in &alias_def.conds {
        //                     if let Err(mut sem_err) =
        //                         self.check_cond(parent_ty_id, parent_span, *inner_cond_expr_id)
        //                     {
        //                         sem_errs.append(&mut sem_err);
        //                     }
        //                 }
        //
        //                 // Checking the parameter's type constraints, if present, against the
        //                 // corresponding argument
        //                 for (i, param) in alias_def.params.iter().enumerate() {
        //                     let constraint_flags =
        //                         match self.compiler.types[param.type_id.id as usize].ty {
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
        //                     let arg_ty_id = &self.compiler.exprs[arg_expr_id.id as usize].type_id;
        //
        //                     dbg!(&self.compiler.types[arg_ty_id.id as usize]);
        //                     panic!();
        //
        //                     if let Err(sem_err) = constraints::check_type_constraint(
        //                         self.compiler,
        //                         *arg_ty_id,
        //                         parent_span,
        //                         cond_expr.span,
        //                         &mut Vec::new(),
        //                         constraint_flags,
        //                     ) {
        //                         sem_errs.push(sem_err);
        //                     }
        //                 }
        //
        //                 if !sem_errs.is_empty() {
        //                     return Err(sem_errs);
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
        //         let sym = &self.compiler.symbols[sym_id.id as usize];
        //         match sym.kind {
        //             SymbolKind::Type(type_id) => match &self.compiler.types[type_id.id as usize].ty
        //             {
        //                 Type::Func(func_def) => {
        //                     // Anything used in a condition must return a boolean
        //                     let ret_type = &self.compiler.types[func_def.ret_type.id as usize].ty;
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
        //                             Err(sem_err) => Err(vec![sem_err]),
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
        //                 let type_id = &self.compiler.values[cond_expr.val_id.id as usize].type_id;
        //                 let ty = &self.compiler.types[type_id.id as usize].ty;
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
        //         let type_id = &self.compiler.values[cond_expr.val_id.id as usize].type_id;
        //         let ty = &self.compiler.types[type_id.id as usize].ty;
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
        //         let type_id = &self.compiler.values[cond_expr.val_id.id as usize].type_id;
        //         let ty = &self.compiler.types[type_id.id as usize].ty;
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
    fn check_type_arg(
        &self,
        type_id: TypeId,
        active_span: Span,
        module: &Module,
        spanned_arg: &SpannedInnerArgs,
        visited: &mut Vec<TypeId>,
        // Making this vec makes error messages painful depending on which message failed, so it
        // needs some signal to say to stop going.
    ) -> Result<(), SemanticError> {
        match &self.compiler.types[type_id.id as usize].ty {
            Type::Struct(struct_def) => {
                let symbol = &self.compiler.symbols[struct_def.sym_id.id as usize];
                // let ast_id = symbol.ast_id.expect("Core should not be resolved");
                // let abs_struct = &self.ast_info.get_struct(ast_id);
                visited.push(type_id);

                // No cross module reporting so all messages are shallow in spanning
                for (i, field) in struct_def.fields.iter().enumerate() {
                    // let ast_span = abs_struct.fields[i].spanned_ty_expr.span;
                    // Checking if one of it's variants are self referencing, or if the type from
                    // the last call stack, possibly a tuple, is self referencing the current
                    // struct.
                    if visited.contains(&field.type_id) {
                        if spanned_arg.arg.has_restrictions() {
                            let name = self.interner.search(symbol.name_id.id as usize);

                            let msg = format!(
                                "The type `{name}` cannot have `#{}` applied due to recursively relying on itself satisfying the argument",
                                spanned_arg.arg
                            );

                            return Err(SemanticError::General(
                                msg,
                                vec![spanned_arg.span, active_span],
                            ));
                        }

                        continue;
                    }

                    visited.push(field.type_id);

                    self.check_type_arg(field.type_id, active_span, module, spanned_arg, visited)?;
                }

                Ok(())
            }
            Type::Enum(enum_def) => {
                let symbol = &self.compiler.symbols[enum_def.sym_id.id as usize];
                // let ast_id = symbol.ast_id.expect("Core should not be resolved");
                // let abs_struct = &self.ast_info.get_enum(ast_id);
                visited.push(type_id);

                for variant in &enum_def.variants {
                    if let Some(inner) = variant.type_id {
                        visited.push(inner);

                        // Checking if one of it's variants are self referencing, or if the type we
                        // just came from, possibly a tuple, is referring to itself from a
                        // different context.
                        if visited.contains(&inner) {
                            if spanned_arg.arg.has_restrictions() {
                                let name = self.interner.search(symbol.name_id.id as usize);

                                let msg = format!(
                                    "The type `{name}` cannot have `#{}` applied due to recursively relying on itself satisfying the argument",
                                    spanned_arg.arg
                                );

                                return Err(SemanticError::General(
                                    msg,
                                    vec![spanned_arg.span, active_span],
                                ));
                            }

                            continue;
                        }

                        self.check_type_arg(inner, active_span, module, spanned_arg, visited)?;
                    }
                }

                Ok(())
            }
            Type::BuiltinType(builtin_type) => {
                match builtin_type {
                    BuiltinType::List(type_id) | BuiltinType::Set(type_id) => {
                        self.check_type_arg(*type_id, active_span, module, spanned_arg, visited)
                    }
                    BuiltinType::Map(key_id, val_id) => {
                        // This looks weird...
                        self.check_type_arg(*key_id, active_span, module, spanned_arg, visited)?;
                        self.check_type_arg(*val_id, active_span, module, spanned_arg, visited)
                    }
                    BuiltinType::Tuple(elements) => {
                        visited.push(type_id);
                        for element in elements {
                            if visited.contains(&*element) {
                                if spanned_arg.arg.has_restrictions() {
                                    return Err(SemanticError::CircularArg(
                                        spanned_arg.arg,
                                        Formatted::Tuple,
                                        vec![spanned_arg.span, active_span],
                                    ));
                                }
                            }

                            let ty = &self.compiler.types[element.id as usize].ty;
                            match ty {
                                Type::BuiltinType(_) => (),
                                _ => visited.push(*element),
                            }

                            self.check_type_arg(
                                *element,
                                active_span,
                                module,
                                spanned_arg,
                                visited,
                            )?;
                        }

                        Ok(())
                    }
                    // Need a function where it obtains type constraints given the recursive types
                    // since shallow checks accept more than proven
                    builtin_ty => {
                        let constraints = builtin_ty.kind().type_constraints();
                        let arg_constraints = spanned_arg.arg.type_constraints();

                        if !arg_constraints.contains(constraints) {
                            return Err(SemanticError::UnsupportedArg(
                                spanned_arg.arg,
                                vec![spanned_arg.span, active_span],
                            ));
                        }

                        Ok(())
                    }
                }
            }
            Type::Alias(alias_def) => {
                let alias_constraints = alias_def.ty_constraints;
                let arg_constraints = spanned_arg.arg.type_constraints();

                if !arg_constraints.contains(alias_constraints) {
                    return Err(SemanticError::TypeConstraintBoundConflict(
                        alias_constraints,
                        arg_constraints,
                        vec![spanned_arg.span, active_span],
                    ));
                }

                Ok(())
            }
            // Uhhhhhhh.
            Type::Unknown => todo!("Unknowned"),
            Type::Func(_) => {
                // VAGUE
                let msg = "Functions can only be placed within type constraint blocks".to_string();
                Err(SemanticError::General(msg, vec![active_span]))
            }
            // Function.
            Type::TypeDef(_) => {
                unreachable!("Not syntactically possible")
            }
            Type::Deferred(deferred_ty_id) => {
                self.check_type_arg(*deferred_ty_id, active_span, module, spanned_arg, visited)
            }
            Type::Constrained(current_constraints) => {
                let arg_constraints = spanned_arg.arg.type_constraints();

                if !arg_constraints.contains(*current_constraints) {
                    return Err(SemanticError::TypeConstraintBoundConflict(
                        *current_constraints,
                        arg_constraints,
                        vec![spanned_arg.span, active_span],
                    ));
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
        parent_span: Span,
        cond_expr_id: ExprId,
        expr_id_args: &[ExprId],
        arg_constraints: &[ArgConstraint],
        // Maybe a more explicit state of Recoverabilitiy as an enum of some sort would be better
        // eventually or at least a wrapper
    ) -> Result<(), Vec<SemanticError>> {
        let mut sem_errs: Vec<SemanticError> = Vec::new();
        for constraint in arg_constraints {
            match constraint {
                ArgConstraint::ArgCount(arg_count_constraint) => {
                    let found_arg_count = expr_id_args.len() as u32;

                    if found_arg_count != *arg_count_constraint {
                        let mut spans: Vec<Span> = expr_id_args
                            .iter()
                            .map(|ex_id| self.compiler.exprs[ex_id.id as usize].span)
                            .collect();

                        if spans.is_empty() {
                            let cond_span = &self.compiler.exprs[cond_expr_id.id as usize].span;
                            spans.push(*cond_span);
                        }

                        sem_errs.push(SemanticError::ArgCountMismatch(
                            *constraint,
                            found_arg_count,
                            spans,
                        ));

                        // Going further would likely lead to misleading errors
                        return Err(sem_errs);
                    }
                }
                ArgConstraint::MatchingArgumentTypes => {
                    // If no arguments then it innately succeeds
                    let req_expr_id = match expr_id_args.first() {
                        Some(id) => id,
                        None => continue,
                    };

                    let req_type_id = self.compiler.exprs[req_expr_id.id as usize].type_id;

                    for expr_id in expr_id_args.iter().skip(1) {
                        let other_type_id = self.compiler.exprs[expr_id.id as usize].type_id;

                        if req_type_id != other_type_id {
                            let req_span = self.compiler.exprs[req_expr_id.id as usize].span;
                            let other_span = self.compiler.exprs[expr_id.id as usize].span;

                            let ty = &self.compiler.types[req_type_id.id as usize].ty;

                            sem_errs.push(SemanticError::FuncConstraintMismatch(
                                *constraint,
                                ty.to_fmt(),
                                vec![req_span, other_span],
                            ));
                        }
                    }
                }
                ArgConstraint::Numeric => {
                    for expr_id in expr_id_args {
                        let type_id = &self.compiler.exprs[expr_id.id as usize].type_id;
                        let ty = &self.compiler.types[type_id.id as usize].ty;

                        if let Type::BuiltinType(builtin_ty) = ty {
                            if !builtin_ty.kind().is_numeric() {
                                let span = self.compiler.exprs[expr_id.id as usize].span;

                                sem_errs.push(SemanticError::FuncConstraintMismatch(
                                    *constraint,
                                    ty.to_fmt(),
                                    vec![span],
                                ));
                            }
                        }
                    }
                }
                ArgConstraint::Integer => {
                    for expr_id in expr_id_args {
                        let type_id = &self.compiler.exprs[expr_id.id as usize].type_id;
                        let ty = &self.compiler.types[type_id.id as usize].ty;

                        if let Type::BuiltinType(builtin_ty) = ty {
                            if !builtin_ty.kind().is_integer() {
                                let span = self.compiler.exprs[expr_id.id as usize].span;

                                sem_errs.push(SemanticError::FuncConstraintMismatch(
                                    *constraint,
                                    ty.to_fmt(),
                                    vec![span],
                                ));
                            }
                        }
                    }
                }
                ArgConstraint::Float => {
                    for expr_id in expr_id_args {
                        let type_id = &self.compiler.exprs[expr_id.id as usize].type_id;
                        let ty = &self.compiler.types[type_id.id as usize].ty;

                        if let Type::BuiltinType(builtin_ty) = ty {
                            if !builtin_ty.kind().is_float() {
                                let span = self.compiler.exprs[expr_id.id as usize].span;

                                sem_errs.push(SemanticError::FuncConstraintMismatch(
                                    *constraint,
                                    ty.to_fmt(),
                                    vec![span],
                                ));
                            }
                        }
                    }
                }
                ArgConstraint::Str => {
                    for expr_id in expr_id_args {
                        let type_id = &self.compiler.exprs[expr_id.id as usize].type_id;
                        let ty = &self.compiler.types[type_id.id as usize].ty;

                        if let Type::BuiltinType(builtin_ty) = ty {
                            if builtin_ty.kind() != BuiltinTypeKind::Str {
                                let span = self.compiler.exprs[expr_id.id as usize].span;

                                sem_errs.push(SemanticError::FuncConstraintMismatch(
                                    *constraint,
                                    ty.to_fmt(),
                                    vec![span],
                                ));
                            }
                        }
                    }
                }
                ArgConstraint::CharacterMappable => {
                    for expr_id in expr_id_args {
                        let type_id = &self.compiler.exprs[expr_id.id as usize].type_id;
                        let ty = &self.compiler.types[type_id.id as usize].ty;

                        if let Type::BuiltinType(builtin_ty) = ty {
                            if !builtin_ty.kind().is_character_mappable() {
                                let span = self.compiler.exprs[expr_id.id as usize].span;

                                sem_errs.push(SemanticError::FuncConstraintMismatch(
                                    *constraint,
                                    ty.to_fmt(),
                                    vec![span],
                                ));
                            }
                        }
                    }
                }
                ArgConstraint::Bool => {
                    for expr_id in expr_id_args {
                        let type_id = &self.compiler.exprs[expr_id.id as usize].type_id;
                        let ty = &self.compiler.types[type_id.id as usize].ty;

                        if let Type::BuiltinType(builtin_ty) = ty {
                            if builtin_ty.kind() != BuiltinTypeKind::Bool {
                                let span = self.compiler.exprs[expr_id.id as usize].span;

                                sem_errs.push(SemanticError::FuncConstraintMismatch(
                                    *constraint,
                                    ty.to_fmt(),
                                    vec![span],
                                ));
                            }
                        }
                    }
                }
                ArgConstraint::Variadic | ArgConstraint::DynType => (),
                ArgConstraint::Comparable => {
                    for expr_id in expr_id_args {
                        let type_id = &self.compiler.exprs[expr_id.id as usize].type_id;
                        // let ty = &self.compiler.types[type_id.id as usize].ty;

                        let expr_span = self.compiler.exprs[expr_id.id as usize].span;
                        let cond_span = self.compiler.exprs[cond_expr_id.id as usize].span;

                        if let Err(sem_err) = constraints::check_type_constraint(
                            self.compiler,
                            *type_id,
                            expr_span,
                            cond_span,
                            &mut Vec::new(),
                            TypeConstraintFlags::new(TypeConstraint::Comparable.to_u64()),
                        ) {
                            sem_errs.push(sem_err);
                        };
                        todo!("TOdol");

                        // dbg!(ty);
                        // match ty {
                        //     Type::BuiltinType(builtin_ty) => {
                        //         if !builtin_ty.kind().is_comparable() {
                        //             let span = self.compiler.exprs[expr_id.id as usize].span;
                        //
                        //             sem_errs.push(SemanticError::FuncConstraintMismatch(
                        //                 *constraint,
                        //                 ty.to_fmt(),
                        //                 vec![span],
                        //             ));
                        //         }
                        //     }
                        //     Type::Struct(struct_def) => todo!(),
                        //     Type::Enum(enum_def) => todo!(),
                        //     Type::Func(func_def) => todo!(),
                        //     Type::Alias(alias_def) => todo!(),
                        //     Type::TypeDef(type_def) => todo!(),
                        //     Type::Constrained(type_constraint_flags) => todo!(),
                        //     Type::Unknown => todo!(),
                        // }
                    }
                }
                // Should be more constrsaint based
                ArgConstraint::SameTypeAsSelf => {
                    let parent_ty = &self.compiler.types[parent_ty_id.id as usize];
                    for expr_id in expr_id_args.iter().skip(1) {
                        let other_ty_id = self.compiler.exprs[expr_id.id as usize].type_id;
                        let types = &self.compiler.types;
                        let ty = &types[parent_ty_id.id as usize];
                        dbg!(ty);

                        panic!();
                        if parent_ty_id != other_ty_id {
                            let other_span = self.compiler.exprs[expr_id.id as usize].span;
                            let msg = "Must be the same type as `self`".to_string();

                            sem_errs
                                .push(SemanticError::General(msg, vec![parent_span, other_span]));
                        }
                    }
                }
            }
        }

        if !sem_errs.is_empty() {
            return Err(sem_errs);
        }

        Ok(())
    }
}
