use common::{
    builtins::BuiltinType,
    fmter::{Formattable, Formatted},
    intern::Intern,
    keywords::{self, Keyword},
    metadata::ChernSettings,
    reporter::diagnostic::Diagnostic,
    symbols::{AstId, InnerArgs, NameId, Span, SpannedInnerArgs, SymbolId, TypeId},
};

use crate::{
    modules::{Module, Program},
    parser::ast::{
        AbstractConst, AbstractEnum, AbstractStruct, AbstractTypeDef, AstInfo, Expr, Item,
        SpannedExpr, UnaryOp,
    },
    semantic::{
        constraints::ArgConstraint,
        error::SemanticError,
        representation::{FuncArgsRepre, FuncKind, FuncRepre, Symbol, Type},
        semantic_reporter::SemanticReporter,
    },
    types::symbols::Cond,
};

pub struct ConstraintResolver<'a> {
    ast_info: &'a AstInfo,
    interner: &'a Intern,
    program: &'a mut Program,
    current_idx: usize,
    reporter: SemanticReporter<'a>,
}

impl ConstraintResolver<'_> {
    pub fn new<'a>(
        settings: &'a ChernSettings,
        ast_info: &'a AstInfo,
        interner: &'a Intern,
        current_idx: usize,
        program: &'a mut Program,
    ) -> ConstraintResolver<'a> {
        ConstraintResolver {
            ast_info,
            interner,
            current_idx,
            program,
            reporter: SemanticReporter::new(settings, interner),
        }
    }

    pub fn resolve(&mut self) -> Result<(), Vec<Diagnostic>> {
        for (id, item) in self.ast_info.items.iter().enumerate() {
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
                Item::Const(abs_const) => (),
            }
        }

        if !self.reporter.err_vec.is_empty() {
            let mut diags = Vec::new();
            diags.append(&mut self.reporter.err_vec);

            return Err(diags);
        }

        Ok(())
    }

    fn resolve_typedef(&mut self, abs_typedef: &AbstractTypeDef, ast_id: AstId) -> Result<(), ()> {
        // First borrow starts here
        let module = &self.program.mods[self.current_idx];
        let sym_id = module.table.sym_ids[&ast_id];
        let type_id = module.table.get_typedef(sym_id).type_id;

        let mut conds = Vec::new();

        for expr in &abs_typedef.conds {
            conds.push(self.resolve_cond(expr, ast_id)?);
        }

        // Second borrow
        let module = &self.program.mods[self.current_idx];
        let ty = &module.table.types[module.table.get_typedef(sym_id).type_id.id as usize];

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

            if let Err(sem_err) =
                self.check_cond_constraints(type_id, module, &ast_span, cond, &mut vec![])
            {
                self.reporter.report_semantic(sem_err, &module);
                return Err(());
            }
        }
        //TODO: RESOLVE FUNC CONSTRAINTS HERE

        // Third borrow
        // Re-borrowing due to resolution happening above being mutable
        let module = &self.program.mods[self.current_idx];
        let ty = &module.table.types[module.table.get_typedef(sym_id).type_id.id as usize];

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
        let module = &mut self.program.mods[self.current_idx];
        let type_def = module.table.get_typedef_mut(sym_id);
        type_def.conds = conds;
        type_def.args = args;

        Ok(())
    }

    //NOTE: The reason this would need to look at the struct again would be because it is iterating
    // through items despite there already being a known struct id, which could be prevented if the
    // struct id itself was passed, but then the loop would iterate over everything by default
    // which seems bad if they're just builtins etc.
    fn resolve_struct(&mut self, abs_struct: &AbstractStruct, ast_id: AstId) -> Result<(), ()> {
        let module = &self.program.mods[self.current_idx];
        let sym_id = module.table.sym_ids[&ast_id];

        let mut conds: Vec<Cond> = Vec::new();

        for expr in &abs_struct.glob_conds {
            conds.push(self.resolve_cond(expr, ast_id)?);
        }

        let module = &self.program.mods[self.current_idx];
        let fields = &module.table.get_struct(sym_id).fields;

        for (i, cond) in conds.iter().enumerate() {
            let ast_span = &abs_struct.glob_conds[i].span;

            for field in fields {
                if let Err(sem_err) =
                    self.check_cond_constraints(field.type_id, module, &ast_span, cond, &mut vec![])
                {
                    self.reporter.report_semantic(sem_err, &module);
                    return Err(());
                }
            }
        }

        let mut args: Vec<InnerArgs> = Vec::new();

        let module = &self.program.mods[self.current_idx];
        let fields = &module.table.get_struct(sym_id).fields;

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

        let module = &mut self.program.mods[self.current_idx];
        let structure = module.table.get_struct_mut(sym_id);

        // I'm scared of this
        structure.args = args;
        structure.conds = conds;

        Ok(())
    }

    fn resolve_enum(&mut self, abs_enum: &AbstractEnum, ast_id: AstId) -> Result<(), ()> {
        let module = &self.program.mods[self.current_idx];
        let sym_id = module.table.sym_ids[&ast_id];

        let mut conds: Vec<Cond> = Vec::new();

        for expr in &abs_enum.glob_conds {
            conds.push(self.resolve_cond(expr, ast_id)?);
        }

        // First borrow
        let module = &self.program.mods[self.current_idx];
        let variants = &module.table.get_enum(sym_id).variants;

        for (i, cond) in conds.iter().enumerate() {
            let ast_span = &abs_enum.glob_conds[i].span;

            for variant in variants {
                if let Some(type_id) = variant.type_id {
                    if let Err(sem_err) =
                        self.check_cond_constraints(type_id, module, &ast_span, cond, &mut vec![])
                    {
                        self.reporter.report_semantic(sem_err, &module);
                    }
                }
            }
        }

        // Second borrow
        let module = &self.program.mods[self.current_idx];
        let variants = &module.table.get_enum(sym_id).variants;

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

        let module = &mut self.program.mods[self.current_idx];
        let enumeration = module.table.get_enum_mut(sym_id);

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
                    &self.program.mods[self.current_idx],
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
                    let arg = self.resolve_func_arg(expr)?;
                    func_args.push(arg);
                }

                let name_id = match caller.as_ref().expr {
                    Expr::Var(name_id) => name_id.id,
                    Expr::FieldAccess(ref abs_field_access) => {
                        todo!();
                    }
                    _ => {
                        let msg = "Condition blocks must contain either keywords, or functions";
                        self.reporter.report_spanned(
                            msg,
                            None,
                            &[caller.span.clone()],
                            &self.program.mods[self.current_idx],
                        );
                        return Err(());
                    }
                };

                let module = &self.program.mods[self.current_idx];
                let sym_id = SymbolId::new(module.table.sym_ids.len() as u32);
                let type_id = TypeId::new(module.table.types.len() as u32);

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

                let module = &mut self.program.mods[self.current_idx];
                module.table.symbols.insert(sym_id, Symbol::Func(func));

                Ok(Cond::Func(sym_id, func_kind))
            }
            Expr::Str(name_id) => {
                let err_name = self.interner.search(name_id.id as usize);
                let err_msg = format!("\"{err_name}\" is not a valid condition");

                self.reporter.report_spanned(
                    &err_msg,
                    Some(err_name),
                    &[spanned_expr.span.clone()],
                    &self.program.mods[self.current_idx],
                );

                Err(())
            }
            Expr::Integer(_) | Expr::Float(_) => {
                let err_msg = format!("Numerics cannot be used as conditions alone");

                self.reporter.report_spanned(
                    &err_msg,
                    None,
                    &[spanned_expr.span.clone()],
                    &self.program.mods[self.current_idx],
                );

                Err(())
            }
            Expr::FieldAccess(field_access) => {
                //TODO: Is this worth evaluating as an expression just to get the name?
                // Sure

                let err_msg = format!("Conditions cannot be accessed as fields");

                self.reporter.report_spanned(
                    &err_msg,
                    None,
                    &[spanned_expr.span.clone()],
                    &self.program.mods[self.current_idx],
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
        match &module.table.types[type_id.id as usize] {
            Type::Struct(sym_id) => {
                let structure = module.table.get_struct(*sym_id);
                visited.push(structure.type_id);

                for field in &structure.fields {
                    // Checking if one of it's variants are self referencing, or if the type from
                    // the last call stack, possibly a tuple, is self referencing the current
                    // struct.
                    if visited.contains(&field.type_id) {
                        //FIXME:
                        //COPY
                        if !spanned_arg.arg.is_basic() {
                            let field_span =
                                match &self.ast_info.items[structure.ast_id.id as usize] {
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
                        let abs_struct = self.ast_info.get_struct(structure.ast_id);
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
                let enumeration = module.table.get_enum(*sym_id);
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
                                if let Item::Enum(abs_enum) =
                                    &self.ast_info.items[enumeration.ast_id.id as usize]
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
                            let abs_enum = &self.ast_info.get_enum(enumeration.ast_id);
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

    fn resolve_func_arg(&self, spanned_expr: &SpannedExpr) -> Result<FuncArgsRepre, ()> {
        match &spanned_expr.expr {
            Expr::Str(name_id) => Ok(FuncArgsRepre::Str(*name_id)),
            Expr::Integer(num) => Ok(FuncArgsRepre::Integer(*num)),
            Expr::Char(ch) => Ok(FuncArgsRepre::Char(*ch)),
            Expr::Float(num) => Ok(FuncArgsRepre::Float(*num)),
            Expr::Var(name_id) => {
                todo!()
            }
            Expr::Call(_, _) => todo!(),
            Expr::FieldAccess(abs_field_access) => todo!(),
            Expr::Unary(unary) => todo!(),
            Expr::BinaryExpr { lhs, op, rhs } => todo!(),
            Expr::Default(_, expr) => todo!(),
        }
    }

    /// Returns a success if all conditions align with the type of the given `type_id`
    /// Takes in the type id that is being checked for validity, the ast id
    fn check_cond_constraints(
        &self,
        type_id: TypeId,
        module: &Module,
        cond_span: &Span,
        cond: &Cond,
        visited: &mut Vec<TypeId>,
    ) -> Result<(), SemanticError> {
        match &module.table.types[type_id.id as usize] {
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
                Cond::Not(inner) => {
                    self.check_cond_constraints(type_id, module, cond_span, inner, visited)
                }
                // Need to check const, alias, and condition namespaces, and let modules stay
                // lazily resolved. Exclamation point!
                Cond::Func(sym_id, func_kind) => Ok(()),
            },
            Type::Struct(sym_id) => {
                let structure = module.table.get_struct(*sym_id);

                for (i, field) in structure.fields.iter().enumerate() {
                    //BUG: The structure.type_id == type_id does check if the last type it saw is
                    //itself, but that could also just mean the last type was a structure that just
                    //so happened to have the same type id
                    //
                    // dbg!(structure.type_id, field.type_id, type_id);
                    if structure.type_id == field.type_id || structure.type_id == type_id {
                        let abs_struct = &self.ast_info.get_struct(structure.ast_id);
                        let field_span = abs_struct.fields[i].spanned_ty_expr.span;

                        return Err(SemanticError::CircularCond(
                            cond.clone(),
                            Formatted::Struct,
                            vec![cond_span.clone(), field_span],
                        ));
                    }

                    let cond_res = self.check_cond_constraints(
                        field.type_id,
                        module,
                        cond_span,
                        cond,
                        visited,
                    );

                    if let Err(SemanticError::UnsupportedCond(cond, fmted_ty, mut spans)) = cond_res
                    {
                        let abs_struct = &self.ast_info.get_struct(structure.ast_id);
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
                let enumeration = module.table.get_enum(*sym_id);
                visited.push(enumeration.type_id);

                for (i, variant) in enumeration.variants.iter().enumerate() {
                    if let Some(ty) = variant.type_id {
                        // Circular ref checking
                        if visited.contains(&ty) {
                            let abs_variant =
                                &self.ast_info.get_enum(enumeration.ast_id).variants[i];

                            let variant_span =
                                abs_variant.ty_expr.as_ref().expect("Already found").span;

                            return Err(SemanticError::CircularCond(
                                cond.clone(),
                                Formatted::Enum,
                                vec![cond_span.clone(), variant_span],
                            ));
                        }

                        visited.push(ty);

                        let cond_res =
                            self.check_cond_constraints(ty, module, cond_span, cond, visited);

                        if let Err(SemanticError::UnsupportedCond(cond, fmted_ty, mut spans)) =
                            cond_res
                        {
                            let abs_struct = &self.ast_info.get_enum(enumeration.ast_id);
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

                    self.check_cond_constraints(*element, module, cond_span, cond, visited)?;
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
                                    return Err(SemanticError::ConstraintMismatch(
                                        ArgConstraint::Numeric,
                                        type_kind.to_fmt(),
                                        func.kind,
                                        vec![func.call_span.clone()],
                                    ));
                                }
                            }
                            invalid_type => {
                                return Err(SemanticError::ConstraintMismatch(
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

                            return Err(SemanticError::ConstraintMismatch(
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
                            SemanticError::ConstraintMismatch(
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
                            SemanticError::ConstraintMismatch(
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
                            SemanticError::ConstraintMismatch(
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
