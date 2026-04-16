pub mod value_context;
use std::collections::VecDeque;

use chern_core::{
    builtins::BuiltinType,
    id_types::{AstId, ModuleId, NameId, SymbolId, TypeId, ValueId},
    inner_args::{InnerArgs, SpannedInnerArgs},
    intern::Intern,
    keywords::Keyword,
    values::{Value, ValueResult},
};
use common::{
    chern_settings::ChernSettings,
    fmter::{Formattable, Formatted},
    reporter::diagnostic::Diagnostic,
    span::Span,
};

use crate::{
    conditions::Cond,
    modules::Module,
    parser::ast::{
        AbstractConst, AbstractEnum, AbstractStruct, AbstractTypeDef, AstInfo, Expr, Item,
        SpannedExpr, UnaryOp,
    },
    script_compiler::{ScriptCompiler, VALUE_FALSE_POS, VALUE_TRUE_POS},
    semantic::{
        constraint_resolver::value_context::{Job, JobStatus, ValueContext},
        constraints::ArgConstraint,
        error::{MathError, SemanticError},
        evaluator,
        representation::{
            FuncArgsRepre, FuncKind, FuncRepre, PossibleMember, Symbol, SymbolInfo, Type,
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

// Maybe just give module control flow to the resolver
impl<'a> ConstraintResolver<'a> {
    pub fn new(
        settings: &'a ChernSettings,
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
        for (id, item) in self.ast_info[self.current_mod.id].items.iter().enumerate() {
            let ast_id = AstId::new(id as u32);

            match item {
                Item::Var(abs_typedef) => {
                    _ = self.resolve_typedef(abs_typedef, ast_id);
                }
                Item::Struct(abs_struct) => {
                    _ = self.resolve_struct(abs_struct, ast_id);
                }
                Item::Enum(abs_enum) => {
                    _ = self.resolve_enum(abs_enum, ast_id);
                }
                Item::Alias(abs_alias) => todo!(),
                Item::Const(abs_const) => {
                    _ = self.resolve_const(abs_const, ast_id);
                }
            }
        }

        //NOTE: Subject to change

        // Starts jobs upon resolving everything from all modules
        if self.current_mod == self.compiler.mods[self.compiler.mods.len() - 1].mod_id {
            match self.resolve_leftover_jobs() {
                Ok(_) => (),
                Err(_) => {
                    let mut jobs: VecDeque<Job> = VecDeque::new();
                    jobs.append(&mut self.val_ctx.jobs);
                    for job in jobs {
                        self.report_job(job);
                    }
                }
            };
        }

        if !self.reporter.err_vec.is_empty() {
            let mut diags = Vec::new();
            diags.append(&mut self.reporter.err_vec);

            return Err(diags);
        }

        Ok(())
    }

    fn report_job(&mut self, job: Job) {
        let sym_info = &self.compiler.symbols[&job.sym_id];
        match &sym_info.symbol {
            Symbol::Const(_) => {
                let msg = format!("Could not evaluate constant");
                self.reporter.report_spanned(
                    &msg,
                    None,
                    &[job.span],
                    &self.compiler.mods[self.current_mod.id],
                );
            }
            Symbol::TypeDef(type_def_repre) => todo!(),
            Symbol::Struct(struct_repre) => todo!(),
            Symbol::Func(func_repre) => todo!(),
            Symbol::Enum(enum_repre) => todo!(),
            Symbol::Alias(alias_repre) => todo!(),
        }
    }

    fn resolve_leftover_jobs(&mut self) -> Result<(), ()> {
        // Tracking if a full cycle was reached given the amount of jobs which should be
        // deterministic (Assuming it's right)
        let mut full_cycle = self.val_ctx.jobs.len();
        let mut cycle = 0;

        while let Some(job) = self.val_ctx.jobs.pop_front() {
            self.current_mod = job.mod_id;
            let val_res = match &self.ast_info[job.mod_id.id].items[job.ast_id.id as usize] {
                Item::Const(abs_const) => {
                    match self.resolve_expr(&abs_const.spanned_expr, job.scope_type) {
                        Ok(res) => res,
                        Err(sem_err) => {
                            self.reporter
                                .report_semantic(sem_err, &self.compiler.mods[job.mod_id.id]);

                            return Err(());
                        }
                    }
                }
                // Item::Var(abs_typedef) => resolve_typedef(abs_typedef, job.ast_id),
                // Item::Struct(abs_struct) => self.resolve_struct(abs_struct, job.ast_id),
                // Item::Enum(abs_enum) => self.resolve_enum(abs_enum, job.ast_id),
                // Item::Alias(abs_alias) => todo!(),
                _ => todo!(),
            };

            match val_res {
                ValueResult::Resolved(val_id) => {
                    // I can't
                    cycle = 0;
                    full_cycle -= 1;
                    let sym_info = self.compiler.symbols.get_mut(&job.sym_id).expect("Exists");

                    match &mut sym_info.symbol {
                        Symbol::Const(const_repre) => {
                            const_repre.val_id = Some(val_id);
                        }
                        Symbol::TypeDef(type_def_repre) => todo!(),
                        Symbol::Struct(struct_repre) => todo!(),
                        Symbol::Func(func_repre) => todo!(),
                        Symbol::Enum(enum_repre) => todo!(),
                        Symbol::Alias(alias_repre) => todo!(),
                    }
                }
                ValueResult::Unresolved => {
                    cycle += 1;
                    self.val_ctx.jobs.push_back(job);

                    if cycle > full_cycle {
                        return Err(());
                    }
                }
            }
        }

        Ok(())
    }

    fn resolve_const(&mut self, abs_const: &AbstractConst, ast_id: AstId) -> Result<(), ()> {
        let module = &self.compiler.mods[self.current_mod.id];
        let scope_id = module.extract_scope_id(ScopeType::Neutral);
        let table = &module.get_scope(scope_id).table;

        let sym_id = table.sym_ids[&ast_id];

        //TEST:
        let val_id = match self.resolve_expr(&abs_const.spanned_expr, ScopeType::Neutral) {
            Ok(v) => match v {
                ValueResult::Resolved(v_inner) => v_inner,
                // TODO: Who is the one that pushes the job? Only the original caller?
                ValueResult::Unresolved => {
                    let job = Job::new(
                        sym_id,
                        self.current_mod,
                        ast_id,
                        abs_const.spanned_expr.span,
                        scope_id,
                        ScopeType::Neutral,
                    );

                    self.val_ctx.jobs.push_back(job);
                    return Ok(());
                }
            },
            Err(sem_err) => {
                self.reporter
                    .report_semantic(sem_err, &self.compiler.mods[self.current_mod.id]);
                return Err(());
            }
        };

        // Setting const from an `Unknown` value to whatever was found
        let const_repre = self.compiler.get_const_mut(sym_id);
        const_repre.val_id = Some(val_id);

        Ok(())
    }

    fn resolve_typedef(&mut self, abs_typedef: &AbstractTypeDef, ast_id: AstId) -> Result<(), ()> {
        // First borrow starts here
        let module = &self.compiler.mods[self.current_mod.id];
        let scope_id = module.extract_scope_id(ScopeType::Var);
        let table = &module.get_scope(scope_id).table;

        let sym_id = table.sym_ids[&ast_id];
        let type_id = self.compiler.get_typedef(sym_id).type_id;

        let mut conds = Vec::new();

        for expr in &abs_typedef.conds {
            conds.push(self.resolve_cond(expr, ast_id)?);
        }

        // Second borrow
        let module = &self.compiler.mods[self.current_mod.id];
        let ty = &self.compiler.types[self.compiler.get_typedef(sym_id).type_id.id as usize].ty;

        // Checking if condition is valid for the given type
        // Using the Ast node's condition so that the span information is not lost
        for (i, cond) in conds.iter().enumerate() {
            let ast_span = &abs_typedef.conds[i].span;

            match ty {
                Type::Struct(_) | Type::Enum(_) => {
                    let msg = "Cannot give a `var->` defined variable a condition when it has a `struct` or `enum` type, define\nthis within `nest->`";

                    self.reporter
                        .report_spanned(msg, None, &[ast_span.clone()], &module);

                    return Err(());
                }
                _ => (),
            }

            if let Err(sem_err) = self.check_cond_constraints(type_id, &ast_span, cond, &mut vec![])
            {
                self.reporter.report_semantic(sem_err, &module);
                return Err(());
            }
        }
        //TODO: RESOLVE FUNC CONSTRAINTS HERE

        // Third borrow
        // Re-borrowing due to resolution happening above being mutable
        let ty = &self.compiler.types[self.compiler.get_typedef(sym_id).type_id.id as usize].ty;

        let mut args = Vec::new();

        //TODO: Make less terminal and have a better solution for this
        for spanned_arg in &abs_typedef.args {
            match ty {
                Type::Struct(_) | Type::Enum(_) => {
                    if !spanned_arg.arg.is_basic() {
                        let span = Span::new(spanned_arg.span.start, spanned_arg.span.end);
                        let sem_err = SemanticError::VagueArg(spanned_arg.arg, vec![span]);

                        self.reporter.report_semantic(sem_err, &module);
                        return Err(());
                    }
                }
                _ => (),
            }

            if let Err(sem_err) = self.resolve_arg(type_id, module, &spanned_arg, &mut vec![]) {
                self.reporter.report_semantic(sem_err, &module);
                return Err(());
            }

            args.push(spanned_arg.arg);
        }

        // Fourth borrow...
        let type_def = &mut self.compiler.get_typedef_mut(sym_id);
        type_def.conds = conds;
        type_def.args = args;

        Ok(())
    }

    //NOTE: The reason this would need to look at the struct again would be because it is iterating
    // through items despite there already being a known struct id, which could be prevented if the
    // struct id itself was passed, but then the loop would iterate over everything by default
    // which seems bad if they're just builtins etc.
    fn resolve_struct(&mut self, abs_struct: &AbstractStruct, ast_id: AstId) -> Result<(), ()> {
        let module = &self.compiler.mods[self.current_mod.id];
        let scope_id = module.extract_scope_id(ScopeType::Nest);
        let table = &module.get_scope(scope_id).table;

        let sym_id = table.sym_ids[&ast_id];

        let mut conds: Vec<Cond> = Vec::new();

        for expr in &abs_struct.glob_conds {
            conds.push(self.resolve_cond(expr, ast_id)?);
        }

        let module = &self.compiler.mods[self.current_mod.id];
        let fields = &self.compiler.get_struct(sym_id).fields;

        for (i, cond) in conds.iter().enumerate() {
            let ast_span = &abs_struct.glob_conds[i].span;

            for field in fields {
                if let Err(sem_err) =
                    self.check_cond_constraints(field.type_id, &ast_span, cond, &mut vec![])
                {
                    self.reporter.report_semantic(sem_err, &module);
                    return Err(());
                }
            }
        }

        let mut args: Vec<InnerArgs> = Vec::new();

        let fields = &self.compiler.get_struct(sym_id).fields;

        for field in fields {
            for spanned_arg in &abs_struct.glob_args {
                if let Err(sem_err) =
                    self.resolve_arg(field.type_id, module, spanned_arg, &mut vec![])
                {
                    self.reporter.report_semantic(sem_err, &module);
                    return Err(());
                }

                args.push(spanned_arg.arg);
            }
        }

        let structure = self.compiler.get_struct_mut(sym_id);

        // I'm scared of this
        structure.args = args;
        structure.conds = conds;

        Ok(())
    }

    fn resolve_enum(&mut self, abs_enum: &AbstractEnum, ast_id: AstId) -> Result<(), ()> {
        let module = &self.compiler.mods[self.current_mod.id];
        let scope_id = module.extract_scope_id(ScopeType::Nest);
        let table = &module.get_scope(scope_id).table;

        let sym_id = table.sym_ids[&ast_id];

        let mut conds: Vec<Cond> = Vec::new();

        for expr in &abs_enum.glob_conds {
            conds.push(self.resolve_cond(expr, ast_id)?);
        }

        // First borrow
        let module = &self.compiler.mods[self.current_mod.id];
        let variants = &self.compiler.get_enum(sym_id).variants;

        for (i, cond) in conds.iter().enumerate() {
            let ast_span = &abs_enum.glob_conds[i].span;

            for variant in variants {
                if let Some(type_id) = variant.type_id {
                    if let Err(sem_err) =
                        self.check_cond_constraints(type_id, &ast_span, cond, &mut Vec::new())
                    {
                        self.reporter.report_semantic(sem_err, &module);
                    }
                }
            }
        }

        // Second borrow
        let variants = &self.compiler.get_enum(sym_id).variants;

        let mut args: Vec<InnerArgs> = Vec::new();

        for variant in variants {
            for spanned_arg in &abs_enum.glob_args {
                if let Some(type_id) = variant.type_id {
                    if let Err(sem_err) =
                        self.resolve_arg(type_id, module, spanned_arg, &mut vec![])
                    {
                        self.reporter.report_semantic(sem_err, &module);

                        return Err(());
                    };
                }

                args.push(spanned_arg.arg);
            }
        }

        let enumeration = self.compiler.get_enum_mut(sym_id);

        enumeration.conds = conds;
        enumeration.args = args;

        Ok(())
    }

    // Do we need ast id?
    fn resolve_cond(&mut self, spanned_expr: &SpannedExpr, ast_id: AstId) -> Result<Cond, ()> {
        match &spanned_expr.expr {
            Expr::Var(name_id) => {
                if let Some(cond) = Cond::try_from_id(name_id.id) {
                    return Ok(cond);
                }

                let err_name = self.interner.search(name_id.id as usize);
                let err_msg = format!("\"{err_name}\" is not a valid condition");

                // If the error name IS a functional condition, it just says it's not a condition
                self.reporter.report_spanned(
                    &err_msg,
                    Some(err_name),
                    &[spanned_expr.span.clone()],
                    &self.compiler.mods[self.current_mod.id],
                );

                Err(())
            }
            Expr::Unary(unary) => match unary.op {
                UnaryOp::Not => {
                    let cond = self.resolve_cond(&unary.spanned_expr, ast_id)?;
                    Ok(Cond::Not(Box::new(cond)))
                }
                UnaryOp::Negate => {
                    todo!();
                }
            },
            Expr::Call(caller, args) => {
                let mut func_args: Vec<FuncArgsRepre> = Vec::new();

                for expr in args {
                    let arg = match self.resolve_func_arg(expr) {
                        Ok(a) => a,
                        Err(sem_err) => {
                            self.reporter.report_semantic(todo!(), todo!());
                            todo!();
                        }
                    };
                    func_args.push(arg);
                }

                let name_id = match caller.as_ref().expr {
                    Expr::Var(name_id) => name_id.id,
                    Expr::MemberAccess(ref abs_field_access) => {
                        todo!();
                    }
                    _ => {
                        let msg = "Condition blocks must contain either keywords, or functions";
                        self.reporter.report_spanned(
                            msg,
                            None,
                            &[caller.span.clone()],
                            &self.compiler.mods[self.current_mod.id],
                        );
                        return Err(());
                    }
                };

                let module = &self.compiler.mods[self.current_mod.id];
                let sym_id = SymbolId::new(self.compiler.symbols.len() as u32);
                let type_id = TypeId::new(self.compiler.types.len() as u32);

                //TODO: Maybe handle this elsewhere
                let (constraints, kind) = match Keyword::try_as_kw(name_id) {
                    Some(kw) => match kw {
                        Keyword::Range => (
                            ArgConstraint::from_builtin(FuncKind::Range),
                            FuncKind::Range,
                        ),
                        Keyword::StartsW => (
                            ArgConstraint::from_builtin(FuncKind::StartsW),
                            FuncKind::StartsW,
                        ),
                        Keyword::EndsW => (
                            ArgConstraint::from_builtin(FuncKind::EndsW),
                            FuncKind::EndsW,
                        ),
                        Keyword::Contains => (
                            ArgConstraint::from_builtin(FuncKind::Contains),
                            FuncKind::Contains,
                        ),
                        Keyword::Equals => (
                            ArgConstraint::from_builtin(FuncKind::Equals),
                            FuncKind::Equals,
                        ),
                        // Will this account for aliases?
                        _ => {
                            todo!("User defined");
                        }
                    },
                    None => {
                        todo!("User defined");
                    }
                };

                let func = FuncRepre::new(
                    NameId::new(name_id),
                    type_id,
                    spanned_expr.span.clone(),
                    kind,
                    constraints,
                    func_args,
                );

                // Needs the type to check all constraints. Must put this elsewhere
                match self.check_func_constraints(&func) {
                    Ok(_) => (),
                    Err(sem_err) => {
                        self.reporter.report_semantic(sem_err, &module);
                        return Err(());
                    }
                };

                let func_kind = func.kind;

                // Conditions are private by default
                let sym_info = SymbolInfo::new(Symbol::Func(func), true, self.current_mod);
                self.compiler.symbols.insert(sym_id, sym_info);

                Ok(Cond::Func(sym_id, func_kind))
            }
            Expr::Str(name_id) => {
                let err_name = self.interner.search(name_id.id as usize);
                let err_msg = format!("\"{err_name}\" is not a valid condition");

                self.reporter.report_spanned(
                    &err_msg,
                    Some(err_name),
                    &[spanned_expr.span.clone()],
                    &self.compiler.mods[self.current_mod.id],
                );

                Err(())
            }
            Expr::Integer(_, _) | Expr::Float(_, _) => {
                let err_msg = format!("Numerics cannot be used as conditions alone");

                self.reporter.report_spanned(
                    &err_msg,
                    None,
                    &[spanned_expr.span.clone()],
                    &self.compiler.mods[self.current_mod.id],
                );

                Err(())
            }
            // Parsing-wise this is not possible. Maybe I don't know.
            Expr::MemberAccess(member_access) => {
                //TODO: Is this worth evaluating as an expression just to get the name?
                // Sure

                let err_msg = format!("Conditions cannot be accessed as fields");

                self.reporter.report_spanned(
                    &err_msg,
                    None,
                    &[spanned_expr.span.clone()],
                    &self.compiler.mods[self.current_mod.id],
                );

                Err(())
            }
            Expr::BinaryExpr { lhs, op, rhs } => todo!(),
            Expr::Char(_) => todo!(),
            Expr::Default(_, expr) => todo!(),
        }
    }

    //TODO: Make this less horrific looking
    fn resolve_arg(
        &self,
        type_id: TypeId,
        module: &Module,
        spanned_arg: &SpannedInnerArgs,
        visited: &mut Vec<TypeId>,
    ) -> Result<(), SemanticError> {
        match &self.compiler.types[type_id.id as usize].ty {
            Type::Struct(sym_id) => {
                let structure = self.compiler.get_struct(*sym_id);
                visited.push(structure.type_id);

                for field in &structure.fields {
                    // Checking if one of it's variants are self referencing, or if the type from
                    // the last call stack, possibly a tuple, is self referencing the current
                    // struct.
                    if visited.contains(&field.type_id) {
                        //FIXME:
                        //COPY
                        if !spanned_arg.arg.is_basic() {
                            let field_span = match &self.ast_info[self.current_mod.id].items
                                [structure.ast_id.id as usize]
                            {
                                // Weird looking hack
                                Item::Struct(abs_struct) => {
                                    abs_struct.fields[field.ast_id.id as usize]
                                        .spanned_ty_expr
                                        .span
                                }
                                _ => unreachable!(),
                            }
                            .clone();
                            //NOTE:

                            return Err(SemanticError::CircularArg(
                                spanned_arg.arg,
                                Formatted::Struct,
                                vec![field_span, spanned_arg.span.clone()],
                            ));
                        }

                        continue;
                    }

                    visited.push(field.type_id);
                    //FIXME:

                    let arg_res = self.resolve_arg(field.type_id, module, spanned_arg, visited);

                    // Need to get circular span in a more composed way that's not WEIRD
                    if let Err(SemanticError::UnsupportedArg(arg, kind, _)) = arg_res {
                        //COPY
                        let abs_struct =
                            self.ast_info[self.current_mod.id].get_struct(structure.ast_id);
                        let field_span = abs_struct.fields[field.ast_id.id as usize]
                            .spanned_ty_expr
                            .span;

                        //NOTE:

                        return Err(SemanticError::UnsupportedArg(
                            arg,
                            kind,
                            vec![field_span, spanned_arg.span.clone()],
                        ));
                    }

                    if arg_res.is_err() {
                        return arg_res;
                    }
                }

                Ok(())
            }
            Type::Enum(sym_id) => {
                let enumeration = self.compiler.get_enum(*sym_id);
                visited.push(enumeration.type_id);

                for variant in &enumeration.variants {
                    if let Some(ty) = variant.type_id {
                        visited.push(ty);
                        //FIXME:
                        //COPY

                        // Checking if one of it's variants are self referencing, or if the type we
                        // just came from, possibly a tuple, is referring to itself from a
                        // different context.
                        if enumeration.type_id.id == ty.id || enumeration.type_id == type_id {
                            if !spanned_arg.arg.is_basic() {
                                if let Item::Enum(abs_enum) = &self.ast_info[self.current_mod.id]
                                    .items[enumeration.ast_id.id as usize]
                                {
                                    // or field span
                                    let ast_span = abs_enum.variants[variant.ast_id.id as usize]
                                        .ty_expr
                                        .as_ref()
                                        .expect("The type was already found")
                                        .span;

                                    //NOTE:
                                    // This should be restructured
                                    return Err(SemanticError::CircularArg(
                                        spanned_arg.arg,
                                        Formatted::Enum,
                                        vec![ast_span, spanned_arg.span.clone()],
                                    ));
                                }
                            }
                            //FIXME:

                            // If the type id is self referencing it just skips since we're checking
                            // the enum anyways
                            continue;
                        }

                        let arg_res = self.resolve_arg(ty, module, spanned_arg, visited);

                        if let Err(SemanticError::UnsupportedArg(arg, fmted, _)) = arg_res {
                            let abs_enum =
                                &self.ast_info[self.current_mod.id].get_enum(enumeration.ast_id);
                            let variant_span = abs_enum.variants[variant.ast_id.id as usize]
                                .ty_expr
                                .as_ref()
                                .expect("Type already exists")
                                .span
                                .clone();

                            //NOTE:

                            // fmted or fmtted...
                            return Err(SemanticError::UnsupportedArg(
                                arg,
                                fmted,
                                vec![variant_span, spanned_arg.span.clone()],
                            ));
                        }

                        // If err != nil { return err }
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
                        self.resolve_arg(*type_id, module, spanned_arg, visited)
                    }
                    BuiltinType::Map(key_id, val_id) => {
                        // This looks weird...
                        self.resolve_arg(*key_id, module, spanned_arg, visited)?;
                        self.resolve_arg(*val_id, module, spanned_arg, visited)
                    }
                    BuiltinType::Any(_) => Ok(()),
                    builtin_type => {
                        //BUG: Does reach this error correctly but doesn't send it back?
                        if !spanned_arg.arg.supports_builtin_type(&builtin_type) {
                            // panic!();
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
            Type::Tuple(tuple) => {
                visited.push(tuple.type_id);

                for element in &tuple.elements {
                    if visited.contains(&*element) {
                        if !spanned_arg.arg.is_basic() {
                            return Err(SemanticError::CircularArg(
                                spanned_arg.arg,
                                Formatted::Tuple,
                                vec![spanned_arg.span.clone(), spanned_arg.span.clone()],
                            ));
                        }
                    }

                    visited.push(*element);

                    self.resolve_arg(*element, module, spanned_arg, visited)?;
                }

                Ok(())
            }
            Type::Func(sym_id) => todo!("Func"),
            Type::Alias(_) | Type::Unknown => {
                unreachable!("Parser and semantic cannot produce these variants. I think.")
            }
            Type::Const(symbol_id) => todo!(),
        }
    }

    // Maybe the goal is to resolve as many expressions as possible and attach the value to it's ast
    // id?
    fn resolve_func_arg(&self, spanned_expr: &SpannedExpr) -> Result<FuncArgsRepre, SemanticError> {
        todo!();
    }

    fn resolve_expr(
        &mut self,
        spanned_expr: &SpannedExpr,
        scope_type: ScopeType,
        // This assists for lazily evaluating expressions.
    ) -> Result<ValueResult, SemanticError> {
        match &spanned_expr.expr {
            Expr::Var(name_id) => {
                if name_id.id == Keyword::True as u32 {
                    return Ok(ValueResult::Resolved(ValueId::new(VALUE_TRUE_POS)));
                } else if name_id.id == Keyword::False as u32 {
                    return Ok(ValueResult::Resolved(ValueId::new(VALUE_FALSE_POS)));
                }

                let module = &self.compiler.mods[self.current_mod.id];

                if let Some(sym_id) = module.get_sym_id(*name_id, scope_type) {
                    let symbol = &self.compiler.symbols[&sym_id].symbol;
                    match symbol {
                        Symbol::Const(const_repre) => {
                            if let Some(val_id) = const_repre.val_id {
                                return Ok(ValueResult::Resolved(val_id));
                            }

                            Ok(ValueResult::Unresolved)
                        }
                        // I don't think most of these are possible
                        Symbol::Struct(struct_repre) => todo!(),
                        Symbol::Func(func_repre) => todo!(),
                        Symbol::Enum(enum_repre) => todo!(),
                        Symbol::Alias(alias_repre) => todo!(),
                        // Scoping disallows this entirely
                        Symbol::TypeDef(type_def_repre) => unreachable!(),
                    }
                } else {
                    // SemanticError needs centralization
                    let name = self.interner.search(name_id.id as usize);
                    let mod_name = self.interner.search(module.name_id.id as usize);
                    let msg =
                        format!("The variable `{name}` was not found in the module `{mod_name}`");

                    Err(SemanticError::General(msg, vec![spanned_expr.span]))
                }
            }
            Expr::Integer(id, _) => {
                if let Ok(num) = self.interner.search(*id as usize).parse::<i128>() {
                    let value_id = ValueId::new(self.compiler.values.len());
                    self.compiler.values.push(Value::I128(num));

                    Ok(ValueResult::Resolved(value_id))
                } else {
                    Err(SemanticError::NumericOverflow(
                        *id,
                        Formatted::Integer,
                        vec![spanned_expr.span],
                    ))
                }
            }
            Expr::Float(id, _) => {
                // No BigFloat yet
                if let Ok(num) = self.interner.search(*id as usize).parse::<f64>() {
                    let value_id = ValueId::new(self.compiler.values.len());
                    self.compiler.values.push(Value::F64(num));

                    Ok(ValueResult::Resolved(value_id))
                } else {
                    Err(SemanticError::NumericOverflow(
                        *id,
                        Formatted::Float,
                        vec![spanned_expr.span],
                    ))
                }
            }
            Expr::BinaryExpr { lhs, op, rhs } => {
                let lhs_id = match self.resolve_expr(&*lhs, scope_type)? {
                    ValueResult::Resolved(inner) => inner,
                    ValueResult::Unresolved => return Ok(ValueResult::Unresolved),
                };

                let rhs_id = match self.resolve_expr(&*rhs, scope_type)? {
                    ValueResult::Resolved(inner) => inner,
                    ValueResult::Unresolved => return Ok(ValueResult::Unresolved),
                };

                let lhs_val = &self.compiler.values[lhs_id.id];
                let rhs_val = &self.compiler.values[rhs_id.id];

                if !evaluator::is_compatible_binary(&lhs_val, *op, &rhs_val) {
                    let full_span = lhs.span.merge(rhs.span);

                    return Err(MathError::BinaryOpMismatch(
                        lhs_val.kind().to_fmt(),
                        rhs_val.kind().to_fmt(),
                        op.to_fmt(),
                        vec![full_span],
                    ))?;
                }

                let val_id = ValueId::new(self.compiler.values.len());
                let val = evaluator::apply_binary_op(&lhs_val, *op, &rhs_val)?;
                self.compiler.values.push(val);

                Ok(ValueResult::Resolved(val_id))
            }
            Expr::Char(c) => {
                let val_id = ValueId::new(self.compiler.values.len());
                self.compiler.values.push(Value::Char(*c));

                Ok(ValueResult::Resolved(val_id))
            }
            Expr::Default(name_id, spanned_expr) => {
                // DO NOT QUESTION THIS
                if self.interner.search(name_id.id as usize) == "_" {}

                todo!();
            }
            Expr::Str(name_id) => {
                let val_id = ValueId::new(self.compiler.values.len());
                self.compiler.values.push(Value::CompileStr(*name_id));

                Ok(ValueResult::Resolved(val_id))
            }
            Expr::Call(caller, spanned_exprs) => {
                todo!();
            }
            Expr::MemberAccess(abs_member_access) => {
                match self.resolve_member(&abs_member_access.base, scope_type)? {
                    PossibleMember::Module(mod_id) => {
                        let extern_mod = &mut self.compiler.mods[mod_id.id];
                        if let Some(sym_id) =
                            extern_mod.get_sym_id(abs_member_access.field, scope_type)
                        {
                            let symbol = &self.compiler.symbols[&sym_id].symbol;
                            match symbol {
                                Symbol::Const(const_repre) => {
                                    if let Some(val_id) = const_repre.val_id {
                                        return Ok(ValueResult::Resolved(val_id));
                                    }

                                    Ok(ValueResult::Unresolved)
                                }
                                // I don't think most of these are possible
                                Symbol::Struct(struct_repre) => todo!(),
                                Symbol::Func(func_repre) => todo!(),
                                Symbol::Enum(enum_repre) => todo!(),
                                Symbol::Alias(alias_repre) => todo!(),
                                // Scoping disallows this entirely
                                Symbol::TypeDef(type_def_repre) => unreachable!(),
                            }
                        } else {
                            Ok(ValueResult::Unresolved)
                        }
                    }
                    PossibleMember::Var(val_id) => {
                        ValueResult::Resolved(val_id);
                        unimplemented!("Nothing matches this case yet");
                    }
                    PossibleMember::Nothing => Ok(ValueResult::Unresolved),
                }
            }
            Expr::Unary(unary) => {
                let operand_id = match self.resolve_expr(&unary.spanned_expr, scope_type)? {
                    ValueResult::Resolved(id) => id,
                    ValueResult::Unresolved => return Ok(ValueResult::Unresolved),
                };
                let operand = &self.compiler.values[operand_id.id];

                if !evaluator::is_compatible_unary(unary.op, operand) {
                    return Err(MathError::UnaryOpMismatch(
                        operand.kind().to_fmt(),
                        unary.op.to_fmt(),
                        vec![spanned_expr.span],
                    ))?;
                }

                let val = evaluator::apply_unary_op(unary.op, operand)?;
                let val_id = ValueId::new(self.compiler.values.len());

                self.compiler.values.push(val);

                Ok(ValueResult::Resolved(val_id))
            }
        }
    }

    fn resolve_member(
        &mut self,
        member: &SpannedExpr,
        scope_type: ScopeType,
    ) -> Result<PossibleMember, SemanticError> {
        if let Ok(val_res) = self.resolve_expr(member, scope_type) {
            match val_res {
                ValueResult::Resolved(val_id) => return Ok(PossibleMember::Var(val_id)),
                ValueResult::Unresolved => return Ok(PossibleMember::Nothing),
            };
        }

        if let Expr::Var(name_id) = member.expr {
            if let Some(mod_id) = self.compiler.mod_map.get(&name_id) {
                return Ok(PossibleMember::Module(*mod_id));
            }
        }

        Err(SemanticError::UndefinedMember(member.span))
    }

    //TEST:

    /// Returns a success if all conditions align with the type of the given `type_id`
    /// Takes in the type id that is being checked for validity, the ast id
    fn check_cond_constraints(
        &self,
        type_id: TypeId,
        cond_span: &Span,
        cond: &Cond,
        visited: &mut Vec<TypeId>,
    ) -> Result<(), SemanticError> {
        match &self.compiler.types[type_id.id as usize].ty {
            Type::BuiltinType(builtin_type) => match cond {
                Cond::IsEmpty | Cond::IsWhitespace => {
                    let kind = builtin_type.kind();

                    if !cond.supports_builtin_type(kind) {
                        return Err(SemanticError::UnsupportedCond(
                            cond.clone(),
                            kind.to_fmt(),
                            vec![cond_span.clone()],
                        ));
                    }

                    Ok(())
                }
                Cond::Not(inner) => self.check_cond_constraints(type_id, cond_span, inner, visited),
                // Need to check const, alias, and condition namespaces, and let modules stay
                // lazily resolved. Exclamation point!
                Cond::Func(sym_id, func_kind) => Ok(()),
            },
            Type::Struct(sym_id) => {
                let structure = self.compiler.get_struct(*sym_id);

                for (i, field) in structure.fields.iter().enumerate() {
                    //BUG: The structure.type_id == type_id does check if the last type it saw is
                    //itself, but that could also just mean the last type was a structure that just
                    //so happened to have the same type id
                    //
                    if structure.type_id == field.type_id || structure.type_id == type_id {
                        let abs_struct =
                            &self.ast_info[self.current_mod.id].get_struct(structure.ast_id);
                        let field_span = abs_struct.fields[i].spanned_ty_expr.span;

                        return Err(SemanticError::CircularCond(
                            cond.clone(),
                            Formatted::Struct,
                            vec![cond_span.clone(), field_span],
                        ));
                    }

                    let cond_res =
                        self.check_cond_constraints(field.type_id, cond_span, cond, visited);

                    if let Err(SemanticError::UnsupportedCond(cond, fmted_ty, mut spans)) = cond_res
                    {
                        let abs_struct =
                            &self.ast_info[self.current_mod.id].get_struct(structure.ast_id);
                        let field_span = abs_struct.fields[i].spanned_ty_expr.span;
                        spans.push(field_span);

                        return Err(SemanticError::UnsupportedCond(cond, fmted_ty, spans));
                    }

                    if cond_res.is_err() {
                        return cond_res;
                    }
                }

                Ok(())
            }
            Type::Enum(sym_id) => {
                let enumeration = self.compiler.get_enum(*sym_id);
                visited.push(enumeration.type_id);

                for (i, variant) in enumeration.variants.iter().enumerate() {
                    if let Some(ty) = variant.type_id {
                        // Circular ref checking
                        if visited.contains(&ty) {
                            let abs_variant = &self.ast_info[self.current_mod.id]
                                .get_enum(enumeration.ast_id)
                                .variants[i];

                            let variant_span =
                                abs_variant.ty_expr.as_ref().expect("Already found").span;

                            return Err(SemanticError::CircularCond(
                                cond.clone(),
                                Formatted::Enum,
                                vec![cond_span.clone(), variant_span],
                            ));
                        }

                        visited.push(ty);

                        let cond_res = self.check_cond_constraints(ty, cond_span, cond, visited);

                        if let Err(SemanticError::UnsupportedCond(cond, fmted_ty, mut spans)) =
                            cond_res
                        {
                            let abs_struct =
                                &self.ast_info[self.current_mod.id].get_enum(enumeration.ast_id);
                            let field_span = abs_struct.variants[i]
                                .ty_expr
                                .as_ref()
                                .expect("Already found")
                                .span;

                            spans.push(field_span);

                            return Err(SemanticError::UnsupportedCond(cond, fmted_ty, spans));
                        }

                        if cond_res.is_err() {
                            return cond_res;
                        }
                    }
                }

                Ok(())
            }
            Type::Tuple(tuple) => {
                visited.push(tuple.type_id);

                for element in &tuple.elements {
                    if visited.contains(&element) {
                        return Err(SemanticError::CircularCond(
                            cond.clone(),
                            Formatted::Tuple,
                            vec![cond_span.clone()],
                        ));
                    }

                    self.check_cond_constraints(element.clone(), cond_span, cond, visited)?;
                }

                Ok(())
            }
            Type::Alias(sym_id) => todo!(),
            Type::Unknown | Type::Func(_) => {
                unreachable!("Parser and semantic cannot produce these variants")
            }
            Type::Const(symbol_id) => todo!(),
        }
    }

    /// Returns a success if all constraints within the given function align with the function's
    /// signature.
    fn check_func_constraints(&self, func: &FuncRepre) -> Result<(), SemanticError> {
        for constraint in func.constraints.iter().copied() {
            match constraint {
                ArgConstraint::Numeric => {
                    for arg in &func.args {
                        match arg {
                            FuncArgsRepre::Integer(_) | FuncArgsRepre::Float(_) => continue,
                            FuncArgsRepre::Var(_, type_kind) => {
                                if !type_kind.is_numeric() {
                                    return Err(SemanticError::FuncConstraintMismatch(
                                        ArgConstraint::Numeric,
                                        type_kind.to_fmt(),
                                        func.kind,
                                        vec![func.call_span.clone()],
                                    ));
                                }
                            }
                            invalid_type => {
                                return Err(SemanticError::FuncConstraintMismatch(
                                    ArgConstraint::Numeric,
                                    invalid_type.to_builtin_kind().to_fmt(),
                                    func.kind,
                                    vec![func.call_span.clone()],
                                ));
                            }
                        }
                    }
                }
                ArgConstraint::MatchingType => {
                    // Maybe this is dangerous?
                    let req_type = if let Some(arg) = func.args.get(0) {
                        arg.kind()
                    } else {
                        continue;
                    };

                    for arg in func.args.iter().skip(1) {
                        if arg.kind() != req_type {
                            // There is no general "number" to give so may adjust this

                            return Err(SemanticError::FuncConstraintMismatch(
                                constraint,
                                arg.to_builtin_kind().to_fmt(),
                                func.kind,
                                vec![func.call_span.clone()],
                            ));
                        }
                    }
                }
                ArgConstraint::ArgCount(count) => {
                    if func.args.len() != count as usize {
                        return Err(SemanticError::ArgMiscount(
                            constraint,
                            func.kind,
                            func.args.len() as u8,
                            vec![func.call_span.clone()],
                        ));
                    }
                }
                ArgConstraint::Integer => {
                    for arg in &func.args {
                        if !arg.is_integer() {
                            SemanticError::FuncConstraintMismatch(
                                ArgConstraint::Integer,
                                arg.to_builtin_kind().to_fmt(),
                                func.kind,
                                vec![func.call_span.clone()],
                            );
                        }
                    }
                }
                ArgConstraint::Float => {
                    for arg in &func.args {
                        if !arg.is_float() {
                            SemanticError::FuncConstraintMismatch(
                                ArgConstraint::Float,
                                arg.to_builtin_kind().to_fmt(),
                                func.kind,
                                vec![func.call_span.clone()],
                            );
                        }
                    }
                }
                ArgConstraint::Str => {
                    for arg in &func.args {
                        if !arg.is_str() {
                            SemanticError::FuncConstraintMismatch(
                                ArgConstraint::Str,
                                arg.to_builtin_kind().to_fmt(),
                                func.kind,
                                vec![func.call_span.clone()],
                            );
                        }
                    }
                }
                ArgConstraint::MirroredType => {
                    for arg in &func.args {
                        todo!();
                    }
                }
                ArgConstraint::DynType | ArgConstraint::Variadic => continue,
            }
        }

        Ok(())
    }
}
