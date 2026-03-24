//FIXME: CONDITIONS HAVE NO CONSTRAINTS
// FIXME: FIGURE OUT IF CACHING SHOULD START HERE, AND IN A NODE OR DATA STRUCTURE OUTSIDE OF IT
use common::{
    builtins::{BuiltinType, BuiltinTypeKind},
    fmter::{Formatable, Formatted},
    intern::Intern,
    keywords::{self, Keyword},
    metadata::FileMetadata,
    symbols::{AstId, FuncId, InnerArgs, Span, SpannedInnerArgs, SymbolId, TypeId},
};

use crate::{
    parser::ast::{AbstractEnum, AbstractStruct, AbstractTypeDef, AstInfo, Expr, Item, UnaryOp},
    semantic::{
        constraints::ArgConstraint,
        error::SemanticError,
        representation::{
            FieldRepre, FuncArgsKind, FuncArgsRepre, FuncKind, FuncRepre, Symbol, Table, Type,
            VariantRepre,
        },
        semantic_reporter::SemanticReporter,
    },
    types::symbols::Cond,
};

pub struct ConstraintResolver<'a> {
    ast_info: &'a AstInfo,
    interner: &'a Intern,
    table: &'a mut Table,
    reporter: SemanticReporter<'a>,
}

impl ConstraintResolver<'_> {
    pub fn new<'a>(
        ast_info: &'a AstInfo,
        metadata: &'a FileMetadata,
        interner: &'a Intern,
        table: &'a mut Table,
    ) -> ConstraintResolver<'a> {
        ConstraintResolver {
            ast_info,
            interner,
            table,
            reporter: SemanticReporter::new(metadata),
        }
    }

    pub fn resolve(&mut self) {
        for (id, item) in self.ast_info.items.iter().enumerate() {
            let ast_id = AstId::new(id as u32);

            match item {
                Item::Var(type_def) => {
                    _ = self.resolve_typedef(type_def, ast_id);
                }
                Item::Struct(structure) => {
                    _ = self.resolve_struct(structure, ast_id);
                }
                Item::Enum(enumeration) => {
                    _ = self.resolve_enum(enumeration, ast_id);
                }
                //TEST:
                Item::Alias(abs_alias) => todo!(),
            }
        }

        if !self.reporter.err_vec.is_empty() {
            self.reporter.emit_errors();
            std::process::exit(1);
        }
    }

    fn resolve_typedef(&mut self, abs_typedef: &AbstractTypeDef, ast_id: AstId) -> Result<(), ()> {
        let sym_id = self.table.sym_ids[&ast_id];

        let mut args = Vec::new();

        let type_id = self.table.get_typedef(sym_id).type_id;
        let ty = &self.table.types[self.table.get_typedef(sym_id).type_id.id as usize];

        //TODO: Make less terminal and have a better solution for this
        for spanned_arg in &abs_typedef.args {
            match ty {
                Type::Struct(_) | Type::Enum(_) => {
                    if !spanned_arg.arg.is_basic() {
                        let span = Span::new(spanned_arg.span.start, spanned_arg.span.end);
                        let sem_err = SemanticError::VagueArg(spanned_arg.arg, vec![span]);

                        self.reporter.report_semantic(sem_err);
                        return Err(());
                    }
                }
                _ => (),
            }

            let resolved_arg = match self.resolve_arg(type_id, &spanned_arg) {
                Ok(a) => a,
                Err(sem_err) => {
                    self.reporter.report_semantic(sem_err);
                    return Err(());
                }
            };

            args.push(resolved_arg);
        }

        let mut conds = Vec::new();

        for expr in &abs_typedef.conds {
            conds.push(self.resolve_cond(expr, ast_id)?);
        }

        for cond in &conds {
            self.check_cond_constraints(type_id, cond);
        }

        let type_def = &mut self.table.get_typedef_mut(sym_id);
        type_def.conds = conds;
        type_def.args = args;

        Ok(())
    }

    //NOTE: The reason this would need to look at the struct again would be because it is iterating
    // through items despite there already being a known struct id, which could be prevented if the
    // struct id itself was passed, but then the loop would iterate over everything by default
    // which seems bad if they're just builtins etc.
    fn resolve_struct(&mut self, abs_struct: &AbstractStruct, ast_id: AstId) -> Result<(), ()> {
        // This looks weird

        let mut conds: Vec<Cond> = Vec::new();

        let sym_id = self.table.sym_ids[&ast_id];

        for expr in &abs_struct.glob_conds {
            conds.push(self.resolve_cond(expr, ast_id)?);
        }

        let mut args: Vec<InnerArgs> = Vec::new();
        // This looks odd too
        let fields = &self.table.get_struct(sym_id).fields;

        for field in fields {
            for spanned_arg in &abs_struct.glob_args {
                let arg = match self.resolve_arg(field.type_id, spanned_arg) {
                    Ok(a) => a,
                    Err(sem_err) => {
                        self.reporter.report_semantic(sem_err);
                        return Err(());
                    }
                };

                args.push(arg);
            }
        }

        let structure = &mut self.table.get_struct_mut(sym_id);

        // I'm scared of this
        structure.args = args;
        structure.conds = conds;

        Ok(())
    }

    fn resolve_enum(&mut self, abs_enum: &AbstractEnum, ast_id: AstId) -> Result<(), ()> {
        let sym_id = self.table.sym_ids[&ast_id];

        let mut conds: Vec<Cond> = Vec::new();

        for expr in &abs_enum.glob_conds {
            conds.push(self.resolve_cond(expr, ast_id)?);
        }

        let variants = &self.table.get_enum(sym_id).variants;
        let mut args: Vec<InnerArgs> = Vec::new();

        for variant in variants {
            for spanned_arg in &abs_enum.glob_args {
                if let Some(type_id) = variant.type_id {
                    let arg = match self.resolve_arg(type_id, spanned_arg) {
                        Ok(a) => a,
                        Err(sem_err) => {
                            self.reporter.report_semantic(sem_err);
                            return Err(());
                        }
                    };

                    args.push(arg);
                }
            }
        }

        let enumeration = &mut self.table.get_enum_mut(sym_id);

        enumeration.conds = conds;
        enumeration.args = args;

        Ok(())
    }

    // Do we need ast id?
    fn resolve_cond(&mut self, expr: &Expr, ast_id: AstId) -> Result<Cond, ()> {
        match expr {
            Expr::Var(name_id, span) => {
                if let Some(cond) = Cond::try_from_id(name_id.id) {
                    return Ok(cond);
                }

                let err_name = self.interner.search(name_id.id as usize);
                let err_msg = format!("\"{err_name}\" is not a valid condition");

                // If the error name IS a functional condition, it just says it's not a condition
                self.reporter
                    .report_spanned(&err_msg, Some(err_name), &[span.clone()]);

                Err(())
            }
            Expr::Unary(unary, _) => match unary.op {
                UnaryOp::Not => {
                    let cond = self.resolve_cond(&unary.expr, ast_id)?;
                    Ok(Cond::Not(Box::new(cond)))
                }
                UnaryOp::Negate => {
                    todo!();
                }
            },
            Expr::Call(call, span) => {
                let mut args: Vec<FuncArgsRepre> = Vec::new();

                for expr in &call.exprs {
                    let arg = self.resolve_func_arg(expr)?;
                    args.push(arg);
                }

                let sym_id = SymbolId::new(self.table.sym_ids.len() as u32);
                let type_id = TypeId::new(self.table.types.len() as u32);

                //TODO: Maybe handle this elsewhere
                let (constraints, kind) = match Keyword::try_as_kw(call.name_id.id) {
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
                        // Will this account for aliases?
                        _ => {
                            todo!("User defined");
                        }
                    },
                    None => {
                        todo!("User defined");
                    }
                };

                let func =
                    FuncRepre::new(call.name_id, type_id, span.clone(), kind, constraints, args);

                match self.check_func_constraints(&func) {
                    Ok(_) => (),
                    Err(sem_err) => {
                        self.reporter.report_semantic(sem_err);
                        return Err(());
                    }
                };

                self.table.symbols.insert(sym_id, Symbol::Func(func));

                Ok(Cond::Func(sym_id))
            }
            Expr::Str(name_id, span) => {
                let err_name = self.interner.search(name_id.id as usize);
                let err_msg = format!("\"{err_name}\" is not a valid condition");

                self.reporter
                    .report_spanned(&err_msg, Some(err_name), &[span.clone()]);

                Err(())
            }
            Expr::Integer(_, span) | Expr::Float(_, span) => {
                let err_msg = format!("Numerics cannot be used as conditions alone");

                self.reporter
                    .report_spanned(&err_msg, None, &[span.clone()]);

                Err(())
            }
            Expr::FieldAccess(field_access, span) => {
                //TODO: Is this worth evaluating as an expression just to get the name?
                // Sure

                let err_msg = format!("Conditions cannot be accessed as fields");

                self.reporter
                    .report_spanned(&err_msg, None, &[span.clone()]);

                Err(())
            }
            Expr::BinaryExpr { lhs, op, rhs } => todo!(),
            Expr::Char(_, _) => todo!(),
            Expr::Default(_, expr) => todo!(),
        }
    }

    fn resolve_arg(
        &self,
        type_id: TypeId,
        spanned_arg: &SpannedInnerArgs,
        // Returns SemanticError due to borrowing issues
    ) -> Result<InnerArgs, SemanticError> {
        match &self.table.types[type_id.id as usize] {
            Type::Struct(sym_id) => {
                let structure = self.table.get_struct(*sym_id);

                for field in &structure.fields {
                    // Checking if one of it's variants are self referencing, or if the type from
                    // the last call stack, possibly a tuple, is self referencing the current
                    // struct.
                    if structure.type_id.id == field.type_id.id || structure.type_id == type_id {
                        //FIXME:
                        //COPY
                        if !spanned_arg.arg.is_basic() {
                            if let Item::Struct(abs_struct) =
                                &self.ast_info.items[structure.ast_id.id as usize]
                            {
                                // or field span
                                let ast_span = abs_struct.fields[field.ast_id.id as usize]
                                    .ty
                                    .span()
                                    .clone();

                                //NOTE:

                                return Err(SemanticError::CircularRef(
                                    spanned_arg.arg,
                                    Formatted::Struct,
                                    vec![ast_span, spanned_arg.span.clone()],
                                ));
                            }
                        }

                        continue;
                    }
                    //FIXME:

                    let arg_res = self.resolve_arg(field.type_id, spanned_arg);

                    // Need to get circular span in a more composed way that's not WEIRD
                    if let Err(SemanticError::UnsupportedArg(arg, kind, span)) = arg_res {
                        //COPY
                        if let Item::Struct(abs_struct) =
                            &self.ast_info.items[structure.ast_id.id as usize]
                        {
                            // or field span
                            let ast_span = abs_struct.fields[field.ast_id.id as usize]
                                .ty
                                .span()
                                .clone();

                            //NOTE:

                            return Err(SemanticError::UnsupportedArg(
                                arg,
                                kind,
                                vec![ast_span, spanned_arg.span.clone()],
                            ));
                        }
                    }
                }

                Ok(spanned_arg.arg)
            }
            Type::Enum(sym_id) => {
                let enum_repre = self.table.get_enum(*sym_id);

                for variant in &enum_repre.variants {
                    if let Some(ty) = variant.type_id {
                        //FIXME:
                        //COPY

                        // Checking if one of it's variants are self referencing, or if the type we
                        // just came from, possibly a tuple, is referring to itself from a
                        // different context.
                        if enum_repre.type_id.id == ty.id || enum_repre.type_id == type_id {
                            if !spanned_arg.arg.is_basic() {
                                if let Item::Enum(abs_enum) =
                                    &self.ast_info.items[enum_repre.ast_id.id as usize]
                                {
                                    // or field span
                                    let ast_span = abs_enum.variants[variant.ast_id.id as usize]
                                        .ty
                                        .as_ref()
                                        .expect("The type was already found")
                                        .span()
                                        .clone();

                                    //NOTE:
                                    // This should be restructured
                                    return Err(SemanticError::CircularRef(
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

                        let arg_res = self.resolve_arg(ty, spanned_arg);

                        if let Err(SemanticError::UnsupportedArg(arg, fmted, _)) = arg_res {
                            if let Item::Enum(abs_enum) =
                                &self.ast_info.items[enum_repre.ast_id.id as usize]
                            {
                                // or field span
                                let ast_span = abs_enum.variants[variant.ast_id.id as usize]
                                    .ty
                                    .as_ref()
                                    .expect("Type already exists")
                                    .span()
                                    .clone();

                                //NOTE:

                                // fmted or fmtted...
                                return Err(SemanticError::UnsupportedArg(
                                    arg,
                                    fmted,
                                    vec![ast_span, spanned_arg.span.clone()],
                                ));
                            }
                        }
                    }
                }

                Ok(spanned_arg.arg)
            }
            Type::BuiltinType(builtin_type) => {
                match builtin_type {
                    BuiltinType::Set(type_id) | BuiltinType::List(type_id) => {
                        self.resolve_arg(*type_id, spanned_arg)
                    }
                    BuiltinType::Map(key_id, val_id) => {
                        // This looks weird...
                        self.resolve_arg(*key_id, spanned_arg)?;
                        self.resolve_arg(*val_id, spanned_arg)
                    }
                    BuiltinType::Any(_) => Ok(spanned_arg.arg),
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

                        Ok(spanned_arg.arg)
                    }
                }
            }
            // Another O(n) check...
            //TODO: More detailed error
            Type::Tuple(tuple) => {
                for element in &tuple.elements {
                    self.resolve_arg(*element, spanned_arg)?;
                }

                Ok(spanned_arg.arg)
            }
            Type::Func(sym_id) => todo!("Func"),
            Type::Alias(sym_id) => todo!("Alias"),
            Type::Unknown => todo!(),
            // TODO: Spanning may be off
        }
    }

    fn resolve_func_arg(&self, expr: &Expr) -> Result<FuncArgsRepre, ()> {
        match expr {
            Expr::Str(name_id, _) => Ok(FuncArgsRepre::Str(*name_id)),
            Expr::Integer(num, _) => Ok(FuncArgsRepre::Integer(*num)),
            Expr::Char(ch, _) => Ok(FuncArgsRepre::Char(*ch)),
            Expr::Float(num, _) => Ok(FuncArgsRepre::Float(*num)),
            Expr::Var(name_id, span) => {
                todo!()
            }
            Expr::Call(call, span) => todo!(),
            Expr::FieldAccess(abs_field_access, span) => todo!(),
            Expr::Unary(unary, span) => todo!(),
            Expr::BinaryExpr { lhs, op, rhs } => todo!(),
            Expr::Default(_, expr) => todo!(),
        }
    }

    /// Returns a success if all conditions align with the type of the given `type_id`
    fn check_cond_constraints(&self, type_id: TypeId, cond: &Cond) -> Result<(), SemanticError> {
        match &self.table.types[type_id.id as usize] {
            // The issue is that functions are not easily resolvable because then, I need to
            // reparse conditions,
            Type::BuiltinType(builtin_type) => {
                todo!();
            }
            Type::Struct(sym_id) => todo!(),
            Type::Enum(enum_id) => todo!(),
            Type::Func(func_id) => {
                let func = self.table.get_func(*func_id);
                cond.supports_func(func.kind);
            }
            Type::Alias(sym_id) => todo!(),
            Type::Tuple(type_ids) => todo!(),
            Type::Unknown => unimplemented!("No `Unknown` behavior"),
        }

        Ok(())
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
                // Maybe these shouldn't be constraints if they don't do anything. Boolean for
                // variadics perhaps
                ArgConstraint::DynType | ArgConstraint::Variadic => continue,
            }
        }

        Ok(())
    }
}
