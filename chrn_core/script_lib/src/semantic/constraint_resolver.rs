pub mod value_context;

use chrn_utils::{
    builtins::BuiltinType,
    id_types::{AstId, ExprId, InternedId, ModuleId, SymbolId, TypeId, ValueId},
    inner_args::{InnerArgs, SpannedInnerArgs},
    intern::{self, Intern},
    keywords::Keyword,
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
        constraints::ArgConstraint,
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
                    for err in &self.reporter.err_vec {
                        println!("{}", err.fmtted_diag);
                    }
                    todo!("Todol");
                }
                Item::Var(abs_var) => {
                    _ = self.resolve_var(abs_var, ast_id);
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
        for (i, cond_expr) in type_def.conds.iter().enumerate() {
            let ast_span = &abs_typedef.conds[i].span;

            match &ty_info.ty {
                Type::Struct(_) | Type::Enum(_) => {
                    //NOTE: Would be better as a note
                    let msg = "Cannot give a `var->` defined variable a condition when it has a `struct` or `enum` type, define\nthis within `nest->`";

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

            if let Err(sem_err) = self.check_cond(type_def.type_id, *cond_expr) {
                self.reporter.report_semantic(
                    sem_err,
                    module
                        .src_metadata
                        .as_ref()
                        .expect("core should not be resolved"),
                );
            }
        }

        let ty_span = abs_typedef.spanned_ty_expr.span;
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

    // Needs:
    // Check that only known parameters are used in condition expressions.
    //
    // Check if it's removable depending on if it ONLY has args.
    //
    // Check that all args used align with all function constraints
    //
    // Check that only functions that align with the alias's specific type is used if there is a
    // "Default" expression inside, which just means if all params are not of type `Unknown`
    fn resolve_alias(&mut self, abs_alias: &AbstractAlias, ast_id: AstId) -> Result<(), ()> {
        let scope_id = self
            .compiler
            .extract_scope_id(ScopeType::Neutral, self.current_mod);
        let table = &self.compiler.get_scope(scope_id).scope.table;
        let sym_id = table.ast_to_sym[&ast_id];

        let alias_def = self.compiler.get_alias(sym_id);

        let mut local_vars: Vec<InternedId> = Vec::new();

        for param in &alias_def.params {
            local_vars.push(param.name_id);
        }

        dbg!(local_vars);

        dbg!(alias_def);

        todo!("Alias resolution")
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
        for field in &struct_def.fields {
            for cond_expr in &struct_def.glob_conds {
                if let Err(sem_err) = self.check_cond(field.type_id, *cond_expr) {
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

        // Field conds
        for field in &struct_def.fields {
            for cond_expr in &field.conds {
                if let Err(sem_err) = self.check_cond(field.type_id, *cond_expr) {
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
        for variant in &enum_def.variants {
            if let Some(inner_id) = variant.type_id {
                for cond_expr in &enum_def.glob_conds {
                    if let Err(sem_err) = self.check_cond(inner_id, *cond_expr) {
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

        // Variant conds
        for variant in &enum_def.variants {
            if let Some(inner_id) = variant.type_id {
                for cond_expr in &variant.conds {
                    if let Err(sem_err) = self.check_cond(inner_id, *cond_expr) {
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

    fn check_cond(&self, parent_ty_id: TypeId, cond_expr_id: ExprId) -> Result<(), SemanticError> {
        let ty_info = &self.compiler.types[parent_ty_id.id as usize];
        let cond_expr = &self.compiler.exprs[cond_expr_id.id as usize];

        match &cond_expr.expr_hir {
            ExprHir::Call(callee_expr_id, arg_expr_ids) => {
                let callee = &self.compiler.exprs[callee_expr_id.id as usize];
                todo!("Calling")
            }
            // Ok
            ExprHir::Var(sym_id) => {
                let sym = &self.compiler.symbols[sym_id.id as usize];
                match sym.kind {
                    SymbolKind::Type(type_id) => match &self.compiler.types[type_id.id as usize].ty
                    {
                        Type::BuiltinType(builtin_type) => todo!(),
                        Type::Struct(struct_def) => todo!(),
                        Type::Enum(enum_def) => todo!(),
                        Type::Func(func_def) => todo!(),
                        Type::Alias(alias_def) => todo!(),
                        Type::TypeDef(type_def) => todo!(),
                        Type::Unknown => todo!(),
                    },
                    SymbolKind::Val(_) => {
                        let msg = "Cannot have a value within conditions".to_string();
                        Err(SemanticError::General(msg, vec![cond_expr.span]))
                    }
                    SymbolKind::Unknown => todo!("Is this unknownable?"),
                }
            }
            ExprHir::Val(_) => {
                let msg = "Cannot directly use a value within conditions".to_string();
                Err(SemanticError::General(msg, vec![cond_expr.span]))
            }
            ExprHir::Default(..) => {
                let msg = "Cannot have a `default` expression within conditions".to_string();
                Err(SemanticError::General(msg, vec![cond_expr.span]))
            }
            // Seems annoying to prevent this, might just allow it
            ExprHir::Unary { .. } | ExprHir::BinaryExpr { .. } => {
                let msg = "Cannot directly use expressions within a condition block without an `if()` function".to_string();
                Err(SemanticError::General(msg, vec![cond_expr.span]))
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
                        //FIXME:
                        //COPY
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
                    //FIXME:

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
                        // todo!("Out of tup");

                        Ok(())
                    }
                    builtin_type => {
                        if !spanned_arg.arg.supports_builtin_type(&builtin_type) {
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
            // Not sure how many of these can be reached
            //         Type::Func(sym_id) => todo!("Func"),
            //         // Since aliases have functions, and functions have type restrictions,
            //         argument be checked with the function's constraints
            //         Type::Alias(_) | Type::Unknown => {
            //             unreachable!("Parser and semantic cannot produce these variants. I think.")
            //         }
            //         Type::TypeDef(type_def) => todo!("typedef"),
            Type::Func(func_def) => todo!("Functioned"),
            Type::Alias(alias_def) => todo!("Aliased"),
            Type::TypeDef(type_def) => todo!("TypeDefed"),
            Type::Unknown => todo!("Unknowned"),
        }
    }

    // Maybe alias specific method not needed since alias is just a wrapper for calling multiple
    // functions
    fn check_func_constraints(
        &self,
        call_expr: ExprId,
        func_def: &FuncDef,
    ) -> Result<(), SemanticError> {
        todo!()
    }

    // // //TEST:
    // /// Returns a success if all constraints within the given function align with the function's
    // /// signature.
    // fn check_func_constraints(&self, func: &FuncDef) -> Result<(), SemanticError> {
    //     for constraint in func.constraints.iter().copied() {
    //         match constraint {
    //             ArgConstraint::Numeric => {
    //                 for arg in &func.args {
    //                     match arg {
    //                         FuncArgsRepre::Integer(_) | FuncArgsRepre::Float(_) => continue,
    //                         FuncArgsRepre::Var(_, type_kind) => {
    //                             if !type_kind.is_numeric() {
    //                                 return Err(SemanticError::FuncConstraintMismatch(
    //                                     ArgConstraint::Numeric,
    //                                     type_kind.to_fmt(),
    //                                     func.kind,
    //                                     vec![func.call_span.clone()],
    //                                 ));
    //                             }
    //                         }
    //                         invalid_type => {
    //                             return Err(SemanticError::FuncConstraintMismatch(
    //                                 ArgConstraint::Numeric,
    //                                 invalid_type.to_builtin_kind().to_fmt(),
    //                                 func.kind,
    //                                 vec![func.call_span.clone()],
    //                             ));
    //                         }
    //                     }
    //                 }
    //             }
    //             ArgConstraint::MatchingType => {
    //                 // Maybe this is dangerous?
    //                 let req_type = if let Some(arg) = func.args.get(0) {
    //                     arg.kind()
    //                 } else {
    //                     continue;
    //                 };
    //
    //                 for arg in func.args.iter().skip(1) {
    //                     if arg.kind() != req_type {
    //                         // There is no general "number" to give so may adjust this
    //
    //                         return Err(SemanticError::FuncConstraintMismatch(
    //                             constraint,
    //                             arg.to_builtin_kind().to_fmt(),
    //                             func.kind,
    //                             vec![func.call_span.clone()],
    //                         ));
    //                     }
    //                 }
    //             }
    //             ArgConstraint::ArgCount(count) => {
    //                 if func.args.len() != count as usize {
    //                     return Err(SemanticError::ArgMiscount(
    //                         constraint,
    //                         func.kind,
    //                         func.args.len() as u8,
    //                         vec![func.call_span.clone()],
    //                     ));
    //                 }
    //             }
    //             ArgConstraint::Integer => {
    //                 for arg in &func.args {
    //                     if !arg.is_integer() {
    //                         SemanticError::FuncConstraintMismatch(
    //                             ArgConstraint::Integer,
    //                             arg.to_builtin_kind().to_fmt(),
    //                             func.kind,
    //                             vec![func.call_span.clone()],
    //                         );
    //                     }
    //                 }
    //             }
    //             ArgConstraint::Float => {
    //                 for arg in &func.args {
    //                     if !arg.is_float() {
    //                         SemanticError::FuncConstraintMismatch(
    //                             ArgConstraint::Float,
    //                             arg.to_builtin_kind().to_fmt(),
    //                             func.kind,
    //                             vec![func.call_span.clone()],
    //                         );
    //                     }
    //                 }
    //             }
    //             ArgConstraint::Str => {
    //                 for arg in &func.args {
    //                     if !arg.is_str() {
    //                         SemanticError::FuncConstraintMismatch(
    //                             ArgConstraint::Str,
    //                             arg.to_builtin_kind().to_fmt(),
    //                             func.kind,
    //                             vec![func.call_span.clone()],
    //                         );
    //                     }
    //                 }
    //             }
    //             ArgConstraint::MirroredType => {
    //                 for arg in &func.args {
    //                     todo!();
    //                 }
    //             }
    //             ArgConstraint::DynType | ArgConstraint::Variadic => continue,
    //         }
    //     }
    //
    //     Ok(())
    // }
}
