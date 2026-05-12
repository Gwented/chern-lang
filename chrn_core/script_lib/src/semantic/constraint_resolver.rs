pub mod value_context;

use chrn_utils::{
    builtins::BuiltinType,
    id_types::{AstId, InternedId, ModuleId, SymbolId, TypeId, ValueId},
    inner_args::{InnerArgs, SpannedInnerArgs},
    intern::{self, Intern},
    keywords::Keyword,
    values::ValueResult,
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
            ExprHir, FuncDef, FuncKind, PossibleMember, ResolvedExpr, Symbol, SymbolKind, Type,
        },
        scopes::ScopeType,
        semantic_reporter::SemanticReporter,
    },
};

pub struct ConstraintResolver<'a> {
    ast_info: &'a [AstInfo],
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
        ast_info: &'a [AstInfo],
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
        for (id, item) in self.ast_info[self.current_mod.id].items.iter().enumerate() {
            let ast_id = AstId::new(id as u32);

            match item {
                Item::TypeDef(abs_typedef) => {
                    let res = self.resolve_typedef(abs_typedef, ast_id);
                    if res.is_err() {
                        for err in &self.reporter.err_vec {
                            println!("{}", err.fmtted_diag);
                        }
                    } else {
                        dbg!(&self.compiler.types);
                    }
                    todo!("hi");
                }
                Item::Struct(abs_struct) => {
                    _ = self.resolve_struct(abs_struct, ast_id);
                    for err in &self.reporter.err_vec {
                        println!("{}", err.fmtted_diag);
                    }
                    todo!("hello");
                }
                Item::Enum(abs_enum) => {
                    todo!();
                    // _ = self.resolve_enum(abs_enum, ast_id);
                }
                Item::Alias(abs_alias) => {
                    todo!();
                    // _ = self.resolve_alias(abs_alias, ast_id);
                }
                Item::Var(abs_var) => {
                    todo!();
                    // _ = self.resolve_var(abs_var, ast_id);
                }
            }
        }
        todo!("I am constrained");
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
        // if !self.reporter.err_vec.is_empty() {
        //     let mut diags = Vec::new();
        //     diags.append(&mut self.reporter.err_vec);
        //
        //     return Err(diags);
        // }

        Ok(())
    }

    // fn report_job(&mut self, job: Job) {
    //     let symbol = &self.compiler.symbols[&job.sym_id];
    //     match &symbol.kind {
    //         SymbolKind::Val(val_id) => {
    //             let val_info = &self.compiler.values[val_id.id as usize];
    //             let msg = format!("Could not evaluate variable");
    //             self.reporter.report_spanned(
    //                 &msg,
    //                 None,
    //                 &[job.span],
    //                 &self.compiler.mods[self.current_mod.id].metadata,
    //             );
    //         }
    //         SymbolKind::Type(type_id) => todo!(),
    //         SymbolKind::Unknown => todo!(),
    //     }
    // }
    //
    // fn resolve_leftover_jobs(&mut self) -> Result<(), ()> {
    //     // Tracking if a full cycle was reached given the amount of jobs which should be
    //     // deterministic (Assuming it's right)
    //     let mut full_cycle = self.val_ctx.jobs.len();
    //     let mut cycle = 0;
    //
    //     while let Some(job) = self.val_ctx.jobs.pop_front() {
    //         self.current_mod = job.mod_id;
    //         let val_res = match &self.ast_info[job.mod_id.id].items[job.ast_id.id as usize] {
    //             Item::Var(abs_var) => {
    //                 match self.resolve_expr(&abs_var.spanned_expr, job.scope_type) {
    //                     Ok(res) => res,
    //                     Err(sem_err) => {
    //                         self.reporter
    //                             .report_semantic(sem_err, &self.compiler.mods[job.mod_id.id]);
    //
    //                         return Err(());
    //                     }
    //                 }
    //             }
    //             // Item::Var(abs_typedef) => resolve_typedef(abs_typedef, job.ast_id),
    //             // Item::Struct(abs_struct) => self.resolve_struct(abs_struct, job.ast_id),
    //             // Item::Enum(abs_enum) => self.resolve_enum(abs_enum, job.ast_id),
    //             // Item::Alias(abs_alias) => todo!(),
    //             _ => todo!(),
    //         };
    //
    //         match val_res {
    //             ValueResult::Resolved(val_id) => {
    //                 // I can't
    //                 cycle = 0;
    //                 full_cycle -= 1;
    //                 let symbol = self.compiler.symbols.get_mut(&job.sym_id).expect("Exists");
    //
    //                 match &mut symbol.kind {
    //                     SymbolKind::Val(val_id) => {
    //                         let val_info = &self.compiler.values[val_id.id as usize];
    //                         todo!();
    //                     }
    //                     SymbolKind::Type(type_id) => todo!(),
    //                     SymbolKind::Unknown => todo!(),
    //                 }
    //             }
    //             ValueResult::Unresolved => {
    //                 cycle += 1;
    //                 self.val_ctx.jobs.push_back(job);
    //
    //                 if cycle > full_cycle {
    //                     return Err(());
    //                 }
    //             }
    //         }
    //     }
    //
    //     Ok(())
    // }
    //
    // fn resolve_var(&mut self, abs_var: &AbstractVar, ast_id: AstId) -> Result<(), ()> {
    //     let module = &self.compiler.mods[self.current_mod.id];
    //     let scope_id = module.extract_scope_id(ScopeType::Neutral);
    //     let table = &module.get_scope(scope_id).table;
    //
    //     let sym_id = table.sym_ids[&ast_id];
    //
    //     //TEST:
    //     todo!();
    //     let val_id = match self.resolve_expr(&abs_var.spanned_expr, ScopeType::Neutral) {
    //         Ok(v) => match v {
    //             ValueResult::Resolved(v_inner) => v_inner,
    //             // TODO: Who is the one that pushes the job? Only the original caller?
    //             ValueResult::Unresolved => {
    //                 let job = Job::new(
    //                     sym_id,
    //                     self.current_mod,
    //                     ast_id,
    //                     abs_var.spanned_expr.span,
    //                     scope_id,
    //                     ScopeType::Neutral,
    //                 );
    //
    //                 self.val_ctx.jobs.push_back(job);
    //                 return Ok(());
    //             }
    //         },
    //         Err(sem_err) => {
    //             self.reporter
    //                 .report_semantic(sem_err, &self.compiler.mods[self.current_mod.id]);
    //             return Err(());
    //         }
    //     };
    //
    //     // Setting var from an `Unknown` value to whatever was found
    //     let symbol = self.compiler.symbols.get_mut(&sym_id).expect("Must exist");
    //     symbol.kind = SymbolKind::Val(val_id);
    //
    //     Ok(())
    // }
    //
    // fn resolve_expr(
    //     &mut self,
    //     spanned_expr: &SpannedExpr,
    //     scope_type: ScopeType,
    // ) -> Result<ValueResult, SemanticError> {
    //     todo!();
    // }
    //
    //TODO: Needs to check validate conditions and arguments
    fn resolve_typedef(&mut self, abs_typedef: &AbstractTypeDef, ast_id: AstId) -> Result<(), ()> {
        // First borrow starts here
        let scope_id = self
            .compiler
            .extract_scope_id(ScopeType::Var, self.current_mod);
        let table = &self.compiler.get_scope(scope_id).scope.table;
        let sym_id = table.ast_to_sym[&ast_id];

        let type_def = self.compiler.get_typedef(sym_id);

        // Checking if condition is valid for the given type
        // Using the Ast node's condition so that the span information is not lost
        // for (i, cond) in conds.iter().enumerate() {
        //     let ast_span = &abs_typedef.conds[i].span;
        //
        //     match ty {
        //         Type::Struct(_) | Type::Enum(_) => {
        //             //NOTE: Would be better as a note
        //             let msg = "Cannot give a `var->` defined variable a condition when it has a `struct` or `enum` type, define\nthis within `nest->`";
        //
        //             self.reporter.report_spanned(
        //                 msg,
        //                 None,
        //                 &[ast_span.clone()],
        //                 &self.compiler.mods[self.current_mod.id as usize]
        //                     .src_metadata
        //                     .as_ref()
        //                     .expect("core should not be resolved"),
        //             );
        //
        //             return Err(());
        //         }
        //         _ => (),
        //     }
        //
        //     // if let Err(sem_err) = self.check_cond_constraints(type_id, &ast_span, cond, &mut vec![])
        //     // {
        //     //     self.reporter.report_semantic(sem_err, &module);
        //     //     return Err(());
        //     // }
        // }
        //TODO: RESOLVE FUNC CONSTRAINTS HERE

        let ty_info = &self.compiler.types[type_def.type_id.id as usize];
        let module = &self.compiler.mods[self.current_mod.id];
        //TODO: Make less terminal and have a better solution for this
        for spanned_arg in &abs_typedef.args {
            match &ty_info.ty {
                Type::Struct(_) | Type::Enum(_) => {
                    if !spanned_arg.arg.is_basic() {
                        let span = Span::new(spanned_arg.span.start, spanned_arg.span.end);
                        let sem_err = SemanticError::VagueArg(spanned_arg.arg, vec![span]);

                        self.reporter.report_semantic(
                            sem_err,
                            &self.compiler.mods[self.current_mod.id as usize]
                                .src_metadata
                                .as_ref()
                                .expect("core should not be resolved"),
                        );
                        return Err(());
                    }
                }
                _ => (),
            }

            if let Err(sem_err) =
                self.check_arg(type_def.type_id, module, &spanned_arg, &mut vec![])
            {
                self.reporter.report_semantic(
                    sem_err,
                    module
                        .src_metadata
                        .as_ref()
                        .expect("core should not be resolved"),
                );
                return Err(());
            }
        }
        dbg!(type_def);
        todo!("Typedef gone through");

        Ok(())
    }
    //
    // fn resolve_alias(&mut self, abs_alias: &AbstractAlias, ast_id: AstId) -> Result<(), ()> {
    //     todo!("Alias resolution")
    // }
    //
    // //NOTE: The reason this would need to look at the struct again would be because it is iterating
    // // through items despite there already being a known struct id, which could be prevented if the
    // // struct id itself was passed, but then the loop would iterate over everything by default
    // // which seems bad if they're just builtins etc.
    fn resolve_struct(&mut self, abs_struct: &AbstractStruct, ast_id: AstId) -> Result<(), ()> {
        let scope_id = self
            .compiler
            .extract_scope_id(ScopeType::Nest, self.current_mod);
        let table = &self.compiler.get_scope(scope_id).scope.table;

        //TODO: global condition and argument setting.
        //field arg and cond settings.
        //same for enums.

        let sym_id = table.ast_to_sym[&ast_id];
        //
        //     let mut conds: Vec<Cond> = Vec::new();
        //
        //     for expr in &abs_struct.glob_conds {
        //         let cond = match self.check_cond(expr, ast_id) {
        //             Ok(c) => c,
        //             Err(sem_err) => {
        //                 self.reporter.report_semantic(
        //                     sem_err,
        //                     &self.compiler.mods[self.current_mod.id as usize],
        //                 );
        //
        //                 return Err(());
        //             }
        //         };
        //
        //         conds.push(cond);
        //     }
        //
        //     let module = &self.compiler.mods[self.current_mod.id];
        //     let fields = &self.compiler.get_struct(sym_id).fields;
        //
        //     for (i, cond) in conds.iter().enumerate() {
        //         let ast_span = &abs_struct.glob_conds[i].span;
        //
        //         for field in fields {
        //             // if let Err(sem_err) =
        //             //     self.check_cond_constraints(field.type_id, &ast_span, cond, &mut vec![])
        //             // {
        //             //     self.reporter.report_semantic(sem_err, &module);
        //             //     return Err(());
        //             // }
        //         }
        //     }
        let fields = &self.compiler.get_struct(sym_id).fields;
        let module = &self.compiler.mods[self.current_mod.id];

        for field in fields {
            for spanned_arg in &abs_struct.glob_args {
                if let Err(sem_err) =
                    self.check_arg(field.type_id, module, spanned_arg, &mut vec![])
                {
                    self.reporter.report_semantic(
                        sem_err,
                        module
                            .src_metadata
                            .as_ref()
                            .expect("core should not be resolved"),
                    );
                    return Err(());
                }
            }
        }

        todo!("Alright lot");
        Ok(())
    }
    //
    // fn resolve_enum(&mut self, abs_enum: &AbstractEnum, ast_id: AstId) -> Result<(), ()> {
    //     let module = &self.compiler.mods[self.current_mod.id];
    //     let scope_id = module.extract_scope_id(ScopeType::Nest);
    //     let table = &module.get_scope(scope_id).table;
    //
    //     let sym_id = table.sym_ids[&ast_id];
    //
    //     let mut conds: Vec<Cond> = Vec::new();
    //
    //     for expr in &abs_enum.glob_conds {
    //         let cond = match self.check_cond(expr, ast_id) {
    //             Ok(c) => c,
    //             Err(sem_err) => {
    //                 self.reporter.report_semantic(
    //                     sem_err,
    //                     &self.compiler.mods[self.current_mod.id as usize],
    //                 );
    //
    //                 return Err(());
    //             }
    //         };
    //
    //         conds.push(cond);
    //     }
    //
    //     // First borrow
    //     let module = &self.compiler.mods[self.current_mod.id];
    //     let variants = &self.compiler.get_enum(sym_id).variants;
    //
    //     for (i, cond) in conds.iter().enumerate() {
    //         let ast_span = &abs_enum.glob_conds[i].span;
    //
    //         for variant in variants {
    //             // if let Some(type_id) = variant.type_id {
    //             //     if let Err(sem_err) =
    //             //         self.check_cond_constraints(type_id, &ast_span, cond, &mut Vec::new())
    //             //     {
    //             //         self.reporter.report_semantic(sem_err, &module);
    //             //     }
    //             // }
    //         }
    //     }
    //
    //     // Second borrow
    //     let variants = &self.compiler.get_enum(sym_id).variants;
    //
    //     let mut args: Vec<InnerArgs> = Vec::new();
    //
    //     for variant in variants {
    //         for spanned_arg in &abs_enum.glob_args {
    //             if let Some(type_id) = variant.type_id {
    //                 if let Err(sem_err) =
    //                     self.resolve_arg(type_id, module, spanned_arg, &mut vec![])
    //                 {
    //                     self.reporter.report_semantic(sem_err, &module);
    //
    //                     return Err(());
    //                 };
    //             }
    //
    //             args.push(spanned_arg.arg);
    //         }
    //     }
    //
    //     let enumeration = self.compiler.get_enum_mut(sym_id);
    //
    //     // enumeration.conds = conds;
    //     enumeration.args = args;
    //
    //     Ok(())
    // }
    //
    // // Do we need ast id?
    // fn check_cond(
    //     &mut self,
    //     spanned_expr: &SpannedExpr,
    //     ast_id: AstId,
    // ) -> Result<Cond, SemanticError> {
    //     todo!("Cond resolution");
    // }
    //
    // //TODO: Make this less horrific looking
    fn check_arg(
        &self,
        type_id: TypeId,
        module: &Module,
        // active_span: Span,
        spanned_arg: &SpannedInnerArgs,
        visited: &mut Vec<TypeId>,
    ) -> Result<(), SemanticError> {
        match &self.compiler.types[type_id.id as usize].ty {
            Type::Struct(struct_def) => {
                let symbol = &self.compiler.symbols[&struct_def.sym_id];
                visited.push(type_id);

                for field in &struct_def.fields {
                    // Checking if one of it's variants are self referencing, or if the type from
                    // the last call stack, possibly a tuple, is self referencing the current
                    // struct.
                    if visited.contains(&field.type_id) {
                        //FIXME:
                        //COPY
                        if !spanned_arg.arg.is_basic() {
                            // let field_span = match &self.ast_info[self.current_mod.id].items
                            //     [symbol.ast_id.id as usize]
                            // {
                            //     // Weird looking hack
                            //     Item::Struct(abs_struct) => {
                            //         // abs_struct.fields[field.ast_id.id as usize]
                            //         //     .spanned_ty_expr
                            //         //     .span
                            //         todo!()
                            //     }
                            //     _ => unreachable!(),
                            // }
                            // .clone();
                            //NOTE:

                            // return Err(SemanticError::CircularArg(
                            //     spanned_arg.arg,
                            //     Formatted::Struct,
                            //     vec![field_span, spanned_arg.span.clone()],
                            // ));
                            todo!()
                        }

                        continue;
                    }

                    visited.push(field.type_id);
                    //FIXME:

                    let arg_res = self.check_arg(field.type_id, module, spanned_arg, visited);

                    // Need to get circular span in a more composed way that's not WEIRD
                    // if let Err(SemanticError::UnsupportedArg(arg, kind, _)) = arg_res {
                    //     //COPY
                    //     let abs_struct =
                    //         self.ast_info[self.current_mod.id].get_struct(symbol.ast_id);
                    //     // let field_span = abs_struct.fields[field.ast_id.id as usize]
                    //     //     .spanned_ty_expr
                    //     //     .span;
                    //
                    //     //NOTE:
                    //
                    //     // return Err(SemanticError::UnsupportedArg(
                    //     //     arg,
                    //     //     kind,
                    //     //     vec![field_span, spanned_arg.span.clone()],
                    //     // ));
                    //     todo!()
                    // }

                    if arg_res.is_err() {
                        return arg_res;
                    }
                }

                Ok(())
            }
            //         Type::Enum(enum_def) => {
            //             let symbol = &self.compiler.symbols[&enum_def.sym_id];
            //             visited.push(type_id);
            //
            //             for variant in &enum_def.variants {
            //                 if let Some(ty) = variant.type_id {
            //                     visited.push(ty);
            //                     //FIXME:
            //                     //COPY
            //
            //                     // Checking if one of it's variants are self referencing, or if the type we
            //                     // just came from, possibly a tuple, is referring to itself from a
            //                     // different context.
            //                     //WARN: Changed so could be broken. Removed "enum_def.type_id == type_id"
            //                     if type_id.id == ty.id {
            //                         if !spanned_arg.arg.is_basic() {
            //                             //FIX: Not field's span, just the symbol's.
            //                             if let Item::Enum(abs_enum) = &self.ast_info[self.current_mod.id]
            //                                 .items[symbol.ast_id.id as usize]
            //                             {
            //                                 // or field span
            //                                 let ast_span = abs_enum.variants[variant.ast_id.id as usize]
            //                                     .ty_expr
            //                                     .as_ref()
            //                                     .expect("The type was already found")
            //                                     .span;
            //
            //                                 //NOTE:
            //                                 // This should be restructured
            //                                 return Err(SemanticError::CircularArg(
            //                                     spanned_arg.arg,
            //                                     Formatted::Enum,
            //                                     vec![ast_span, spanned_arg.span.clone()],
            //                                 ));
            //                             }
            //                         }
            //                         //FIXME:
            //
            //                         // If the type id is self referencing it just skips since we're checking
            //                         // the enum anyways
            //                         continue;
            //                     }
            //
            //                     let arg_res = self.resolve_arg(ty, module, spanned_arg, visited);
            //
            //                     if let Err(SemanticError::UnsupportedArg(arg, fmted, _)) = arg_res {
            //                         let abs_enum =
            //                             &self.ast_info[self.current_mod.id].get_enum(symbol.ast_id);
            //                         let variant_span = abs_enum.variants[variant.ast_id.id as usize]
            //                             .ty_expr
            //                             .as_ref()
            //                             .expect("Type already exists")
            //                             .span
            //                             .clone();
            //
            //                         //NOTE:
            //
            //                         // fmted or fmtted...
            //                         return Err(SemanticError::UnsupportedArg(
            //                             arg,
            //                             fmted,
            //                             vec![variant_span, spanned_arg.span.clone()],
            //                         ));
            //                     }
            //
            //                     // If err != nil { return err }
            //                     if arg_res.is_err() {
            //                         return arg_res;
            //                     }
            //                 }
            //             }
            //
            //             Ok(())
            //         }
            Type::BuiltinType(builtin_type) => {
                match builtin_type {
                    BuiltinType::Set(type_id) | BuiltinType::List(type_id) => {
                        self.check_arg(*type_id, module, spanned_arg, visited)
                    }
                    BuiltinType::Map(key_id, val_id) => {
                        // This looks weird...
                        self.check_arg(*key_id, module, spanned_arg, visited)?;
                        self.check_arg(*val_id, module, spanned_arg, visited)
                    }
                    BuiltinType::Any => Ok(()),
                    BuiltinType::Tuple(elements) => {
                        visited.push(type_id);
                        for element in elements {
                            if visited.contains(&*element) {
                                if !spanned_arg.arg.is_basic() {
                                    return Err(SemanticError::CircularArg(
                                        spanned_arg.arg,
                                        Formatted::Tuple,
                                        vec![spanned_arg.span],
                                    ));
                                }
                            }

                            let ty = &self.compiler.types[element.id as usize].ty;
                            match ty {
                                Type::BuiltinType(_) => (),
                                _ => visited.push(*element),
                            }

                            self.check_arg(*element, module, spanned_arg, visited)?;
                        }
                        // todo!("Out of tup");

                        Ok(())
                    }
                    builtin_type => {
                        if !spanned_arg.arg.supports_builtin_type(&builtin_type) {
                            return Err(SemanticError::UnsupportedArg(
                                spanned_arg.arg,
                                builtin_type.kind().to_fmt(),
                                vec![spanned_arg.span.clone()],
                            ));
                        }

                        Ok(())
                    }
                }
            }
            //         Type::Func(sym_id) => todo!("Func"),
            //         Type::Alias(_) | Type::Unknown => {
            //             unreachable!("Parser and semantic cannot produce these variants. I think.")
            //         }
            //         Type::TypeDef(type_def) => todo!("typedef"),
            _ => todo!("heyy"),
        }
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
