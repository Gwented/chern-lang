pub mod value_context;

use chrn_utils::{
    builtins::{BuiltinType, BuiltinTypeKind},
    id_types::{AstId, ExprId, InternedId, ModuleId, SymbolId, TypeId, ValueId},
    inner_args::{InnerArgs, SpannedInnerArgs},
    intern::Intern,
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
        constraint_resolver::value_context::{Job, JobStatus, ValueContext},
        constraints::{self, ArgConstraint, TypeConstraint},
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
    val_ctx: &'a mut ValueContext,
    reporter: SemanticReporter<'a>,
}

impl<'a> ConstraintResolver<'a> {
    pub fn new(
        settings: &'a ChrnSettings,
        ast_info: &'a AstInfo,
        interner: &'a Intern,
        current_mod: ModuleId,
        val_ctx: &'a mut ValueContext,
        compiler: &'a mut ScriptCompiler,
    ) -> ConstraintResolver<'a> {
        ConstraintResolver {
            ast_info,
            interner,
            current_mod,
            compiler,
            val_ctx,
            reporter: SemanticReporter::new(settings, interner),
        }
    }

    pub fn resolve(&mut self) -> Result<(), Vec<Diagnostic>> {
        // The current module and current ast align but that's a bit too arbitrary so will likely
        // stoer that information more explicitly
        // stoer
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
            }
        }
        //
        // //NOTE: Subject to change
        //
        // // Starts jobs upon resolving everything from all modules
        // if self.current_mod == self.compiler.mods[self.compiler.mods.len() - 1].mod_id {
        //     match self.resolve_leftover_jobs() {
        //         Ok(_) => (),
        //         Err(_) => {
        //             let mut jobs: VecDeque<Job> = VecDeque::new();
        //             jobs.append(&mut self.val_ctx.jobs);
        //             for job in jobs {
        //                 self.report_job(job);
        //             }
        //         }
        //     };
        // }
        //
        if !self.reporter.err_vec.is_empty() {
            let mut diags = Vec::new();
            diags.append(&mut self.reporter.err_vec);

            return Err(diags);
        }

        Ok(())
    }

    // fn resolve_expr(
    //     &mut self,
    //     spanned_expr: &SpannedExpr,
    //     scope_type: ScopeType,
    // ) -> Result<ValueResult, SemanticError> {
    //     todo!();
    // }

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

            if let Err(sem_err) =
                self.check_arg(type_def.type_id, ty_span, module, &spanned_arg, &mut vec![])
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

        Ok(())
    }

    fn typecheck_fn(&self) -> Result<(), SemanticError> {
        todo!()
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
    fn resolve_alias(&mut self, abs_alias: &AbstractAlias, ast_id: AstId) -> Result<(), ()> {
        let scope_id = self
            .compiler
            .extract_scope_id(ScopeType::Neutral, self.current_mod);
        let table = &self.compiler.get_scope(scope_id).scope.table;
        let sym_id = table.ast_to_sym[&ast_id];

        //TODO: Need to typecheck based off of the conditional expressions found
        let alias_def = self.compiler.get_alias(sym_id);
        let type_id = self.compiler.get_type_id(sym_id);

        let mut local_vars: Vec<InternedId> = Vec::new();

        for param in &alias_def.params {
            local_vars.push(param.name_id);
        }

        let module = &self.compiler.mods[self.current_mod.id];

        // NOTE: Small issue here is that when we check an alias, and it has an error, it's
        // emitted. But then if we have something that USES the alias, it also gets that error.
        let sym_span = self.ast_info.get_sym_span(ast_id);
        for cond_expr_id in &alias_def.conds {
            if let Err(sem_errs) = self.check_cond(type_id, sym_span, *cond_expr_id) {
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

        // dbg!(local_vars);
        //
        // dbg!(alias_def);
        Ok(())
    }

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
                    self.check_arg(field.type_id, ty_span, module, spanned_arg, &mut vec![])
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
                    self.check_arg(field.type_id, ty_span, module, spanned_arg, &mut vec![])
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
                        self.check_arg(inner_id, ty_span, module, spanned_arg, &mut vec![])
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
                        self.check_arg(inner_id, ty_span, module, spanned_arg, &mut vec![])
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
        ty_span: Span,
        cond_expr_id: ExprId,
    ) -> Result<(), Vec<SemanticError>> {
        let cond_expr = &self.compiler.exprs[cond_expr_id.id as usize];

        match &cond_expr.expr_hir {
            ExprHir::Call(callee_expr_id, arg_expr_ids) => {
                let callee = &self.compiler.exprs[callee_expr_id.id as usize];
                let ty = &self.compiler.types[callee.type_id.id as usize].ty;

                match ty {
                    Type::Func(func_def) => {
                        if !func_def.is_callable {
                            let msg = "Predicate keywords cannot use parameters".to_string();
                            return Err(vec![SemanticError::General(msg, vec![cond_expr.span])]);
                        }

                        // Anything used in a condition must return a boolean
                        let ret_type = &self.compiler.types[func_def.ret_type.id as usize].ty;

                        if let Type::BuiltinType(BuiltinType::Bool) = ret_type {
                            // Maybbe tturrnrn in tot a fucntinson
                            if let Err(sem_err) = self.check_arg_constraints(
                                cond_expr_id,
                                arg_expr_ids,
                                &func_def.arg_constraints,
                                func_def.kind,
                            ) {
                                return Err(sem_err);
                            };

                            match constraints::check_type_constraint(
                                self.compiler,
                                parent_ty_id,
                                ty_span,
                                cond_expr.span,
                                &mut Vec::new(),
                                &func_def.type_constraint,
                            ) {
                                Ok(_) => Ok(()),
                                Err(sem_err) => return Err(vec![sem_err]),
                            }
                        } else {
                            let msg = "Every value within a condition mut evaluate to a boolean"
                                .to_string();
                            Err(vec![SemanticError::General(msg, vec![cond_expr.span])])
                        }
                    }
                    Type::Alias(alias_def) => {
                        let mut sem_errs: Vec<SemanticError> = Vec::new();

                        for inner_cond_expr_id in &alias_def.conds {
                            if let Err(mut sem_err) =
                                self.check_cond(parent_ty_id, ty_span, *inner_cond_expr_id)
                            {
                                sem_errs.append(&mut sem_err);
                            }
                        }

                        if !sem_errs.is_empty() {
                            return Err(sem_errs);
                        }

                        if let Some(constraint) = &alias_def.ty_constraint {
                            todo!("Todol");
                            // constraints::check_type_constraint(
                            //     self.compiler,
                            //     parent_ty_id,
                            //     ty_span,
                            //     cond_expr.span,
                            //     &mut Vec::new(),
                            //     constraint,
                            // )?;
                        }

                        Ok(())
                    }
                    Type::BuiltinType(builtin_type) => todo!(),
                    Type::Struct(struct_def) => todo!(),
                    Type::Enum(enum_def) => todo!(),
                    Type::TypeDef(type_def) => todo!(),
                    Type::Unknown => todo!(),
                }
            }
            // Ok
            ExprHir::Var(sym_id) => {
                let sym = &self.compiler.symbols[sym_id.id as usize];
                match sym.kind {
                    SymbolKind::Type(type_id) => match &self.compiler.types[type_id.id as usize].ty
                    {
                        // Case of just finding a single symbol that expands to a function, like
                        // IsEmpty
                        // Need to check if the function used is usable for the type given
                        // All symbols without a call are predicates so this may be redundant
                        Type::Func(func_def) => {
                            // Anything used in a condition must return a boolean
                            let ret_type = &self.compiler.types[func_def.ret_type.id as usize].ty;

                            if let Type::BuiltinType(BuiltinType::Bool) = ret_type {
                                // self.check_arg_constraints(
                                //     cond_expr_id,
                                //     &[],
                                //     &func_def.arg_constraints,
                                //     func_def.kind,
                                // )?;

                                match constraints::check_type_constraint(
                                    self.compiler,
                                    parent_ty_id,
                                    ty_span,
                                    cond_expr.span,
                                    &mut Vec::new(),
                                    &func_def.type_constraint,
                                ) {
                                    Ok(_) => Ok(()),
                                    Err(sem_err) => Err(vec![sem_err]),
                                }
                            } else {
                                let msg =
                                    "Every value within a condition must be a boolean".to_string();
                                Err(vec![SemanticError::General(msg, vec![cond_expr.span])])
                            }

                            // We need to know if it matches the type given, but only if we are
                            // matching against something that isn't an alias or another function
                            // since that of course wouldn't match.
                        }
                        Type::BuiltinType(builtin_type) => todo!(),
                        Type::Struct(struct_def) => todo!(),
                        Type::Unknown => todo!(),
                        Type::Enum(enum_def) => todo!(),
                        // Should alias build constraints?
                        Type::Alias(alias_def) => todo!(),
                        Type::TypeDef(type_def) => unreachable!("Not syntactically possible"),
                    },
                    // I do not believe unknown is reachable here
                    SymbolKind::Val(_) | SymbolKind::Unknown => {
                        let type_id = &self.compiler.values[cond_expr.val_id.id as usize].type_id;
                        let ty = &self.compiler.types[type_id.id as usize].ty;

                        if let Type::BuiltinType(BuiltinType::Bool) = ty {
                            Ok(())
                        } else {
                            let msg = "Expressions within constraints must evaluate to a boolean"
                                .to_string();
                            Err(vec![SemanticError::General(msg, vec![cond_expr.span])])
                        }
                    }
                }
            }
            // Only `BinaryExpr` can actually evaluate to a boolean here, just re-using the logic
            ExprHir::BinaryExpr { .. } | ExprHir::Unary { .. } | ExprHir::Default(..) => {
                let type_id = &self.compiler.values[cond_expr.val_id.id as usize].type_id;
                let ty = &self.compiler.types[type_id.id as usize].ty;

                if let Type::BuiltinType(BuiltinType::Bool) = ty {
                    Ok(())
                } else {
                    Err(vec![SemanticError::General(
                        "Expressions within constraints must evaluate to a boolean".to_string(),
                        vec![cond_expr.span],
                    )])
                }
            }
            ExprHir::Val(_) => {
                let type_id = &self.compiler.values[cond_expr.val_id.id as usize].type_id;
                let ty = &self.compiler.types[type_id.id as usize].ty;

                if let Type::BuiltinType(BuiltinType::Bool) = ty {
                    Ok(())
                } else {
                    let msg =
                        "Expressions within constraints must evaluate to a boolean".to_string();
                    Err(vec![SemanticError::General(msg, vec![cond_expr.span])])
                }
            }
        }
    }

    fn check_arg(
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

                    let arg_res =
                        self.check_arg(field.type_id, active_span, module, spanned_arg, visited);

                    if arg_res.is_err() {
                        return arg_res;
                    }
                }

                Ok(())
            }
            Type::Enum(enum_def) => {
                let symbol = &self.compiler.symbols[enum_def.sym_id.id as usize];
                // let ast_id = symbol.ast_id.expect("Core should not be resolved");
                // let abs_struct = &self.ast_info.get_enum(ast_id);
                visited.push(type_id);

                for variant in &enum_def.variants {
                    if let Some(id) = variant.type_id {
                        visited.push(id);

                        // Checking if one of it's variants are self referencing, or if the type we
                        // just came from, possibly a tuple, is referring to itself from a
                        // different context.
                        if visited.contains(&id) {
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

                        let arg_res = self.check_arg(id, active_span, module, spanned_arg, visited);

                        if arg_res.is_err() {
                            return arg_res;
                        }
                    }
                }

                Ok(())
            }
            Type::BuiltinType(builtin_type) => {
                match builtin_type {
                    BuiltinType::Set(type_id) | BuiltinType::List(type_id) => {
                        self.check_arg(*type_id, active_span, module, spanned_arg, visited)
                    }
                    BuiltinType::Map(key_id, val_id) => {
                        // This looks weird...
                        self.check_arg(*key_id, active_span, module, spanned_arg, visited)?;
                        self.check_arg(*val_id, active_span, module, spanned_arg, visited)
                    }
                    BuiltinType::Any => Ok(()),
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

                            self.check_arg(*element, active_span, module, spanned_arg, visited)?;
                        }

                        Ok(())
                    }
                    builtin_type => {
                        if !spanned_arg.arg.supports_builtin_ty(&builtin_type) {
                            return Err(SemanticError::UnsupportedArg(
                                spanned_arg.arg,
                                builtin_type.kind().to_fmt(),
                                vec![spanned_arg.span, active_span],
                            ));
                        }

                        Ok(())
                    }
                }
            }
            // Has constraints if a "default" expression is used
            Type::Alias(alias_def) => todo!("Aliased"),
            Type::Unknown => todo!("Unknowned"),
            Type::TypeDef(_) | Type::Func(_) => {
                unreachable!("Not syntactically possible")
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
    fn check_arg_constraints(
        &self,
        cond_expr_id: ExprId,
        expr_id_args: &[ExprId],
        constraints: &[ArgConstraint],
        kind: FuncKind,
        // Maybe a more explicit state of Recoverabilitiy as an enum of some sort would be better
        // eventually or at least a wrapper
    ) -> Result<(), Vec<SemanticError>> {
        // Maybe less terminal
        let mut sem_errs: Vec<SemanticError> = Vec::new();
        for constraint in constraints {
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
                            kind,
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
                                kind,
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
                                    kind,
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
                                    kind,
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
                                    kind,
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
                                    kind,
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
                                    kind,
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
                                    kind,
                                    vec![span],
                                ));
                            }
                        }
                    }
                }
                ArgConstraint::Variadic | ArgConstraint::DynType => (),
            }
        }

        if !sem_errs.is_empty() {
            return Err(sem_errs);
        }

        Ok(())
    }
}
