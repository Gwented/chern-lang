pub mod type_context;

use chrn_core::builtins::BuiltinTypeKind;
use chrn_core::id_types::{AstId, ExprId, InternedId, ModuleId, SymbolId, TypeId, ValueId};
use chrn_core::values::{Value, ValueInfo, ValueResult};
use chrn_core::{builtins::BuiltinType, intern::Intern, keywords::Keyword};
use common::chrn_settings::ChernSettings;
use common::fmter::{Formattable, Formatted};
use common::{reporter::diagnostic::Diagnostic, span::Span};

use crate::parser::ast::{AbstractVar, Expr, SpannedExpr};
use crate::script_compiler::{self, ScriptCompiler};
use crate::semantic::error::{MathError, SemanticError};
use crate::semantic::evaluator;
use crate::semantic::representation::{ExprHir, PossibleMember, ResolvedExpr, SymbolKind};
use crate::semantic::scopes::ScopeType;
use crate::semantic::type_resolver::type_context::{PendingExpr, PendingUser, TypeContext};
use crate::{
    parser::ast::{
        AbstractAlias, AbstractEnum, AbstractStruct, AbstractTypeDef, AstInfo, Item,
        SpannedTypeExpr, TypeExpr,
    },
    semantic::{
        representation::{FieldRepre, Tuple, Type, TypeInfo, VariantRepre},
        semantic_reporter::SemanticReporter,
    },
};
/// Resolves types and builds the rest of any structs or enums
pub struct TypeResolver<'a> {
    ast_info: &'a AstInfo,
    interner: &'a Intern,
    //WARN: Horrors
    compiler: &'a mut ScriptCompiler,
    current_mod: ModuleId,
    ty_ctx: &'a mut TypeContext,
    reporter: SemanticReporter<'a>,
    //NOTE: May handle this differently but ok for now
}

impl TypeResolver<'_> {
    pub fn new<'a>(
        settings: &'a ChernSettings,
        ast_info: &'a AstInfo,
        current_mod: ModuleId,
        ty_ctx: &'a mut TypeContext,
        interner: &'a Intern,
        compiler: &'a mut ScriptCompiler,
    ) -> TypeResolver<'a> {
        TypeResolver {
            ast_info,
            current_mod,
            ty_ctx,
            reporter: SemanticReporter::new(settings, interner),
            interner,
            compiler,
        }
    }

    //FIXME: USE A SINGULAR VECTOR INDEXED BY NAMEID LATER OVER A HASHMAP NOT NOW PLEASE NOT NOW
    // Ok. But when. I don't know.
    //TODO: Check structures of data for same name symbols
    pub fn resolve(&mut self) -> Result<(), Vec<Diagnostic>> {
        // This is resolving types but not resolving args or conditions.
        // Everything is in order so this cannot fail unless something internally went wrong.
        for (id, item) in self.ast_info.items.iter().enumerate() {
            let ast_id = AstId::new(id as u32);

            match item {
                Item::TypeDef(abs_typedef) => _ = self.resolve_typedef(abs_typedef, ast_id),
                Item::Struct(abs_struct) => _ = self.resolve_struct(abs_struct, ast_id),
                Item::Enum(abs_enum) => _ = self.resolve_enum(abs_enum, ast_id),
                Item::Alias(abs_alias) => _ = self.resolve_alias(abs_alias, ast_id),
                // Type inference is required
                Item::VarDecl(abs_var) => _ = self.resolve_var(abs_var, ast_id),
            }
        }

        if !self.reporter.err_vec.is_empty() {
            let mut diags = Vec::new();
            diags.append(&mut self.reporter.err_vec);

            return Err(diags);
        }

        self.try_resolve().unwrap();
        panic!("tried once");

        Ok(())
    }

    fn try_resolve(&mut self) -> Result<(), SemanticError> {
        todo!()
    }

    fn resolve_var(&mut self, abs_var: &AbstractVar, ast_id: AstId) -> Result<(), ()> {
        let module = &mut self.compiler.mods[self.current_mod.id];
        let scope_id = module.extract_scope_id(ScopeType::Neutral);
        let table = &mut module.get_scope_mut(scope_id).table;

        let sym_id = table.sym_ids[&ast_id];

        //TODO: Create pipeline where expressions are always returned, just that some may have
        //unresolved parts, which are put into the queue, not the variable itself.
        let expr_id = match self.register_expr(sym_id, &abs_var.spanned_expr, ScopeType::Neutral) {
            Ok(expr_id) => expr_id,
            Err(sem_err) => {
                self.reporter
                    .report_semantic(sem_err, &self.compiler.mods[self.current_mod.id]);
                return Err(());
            }
        };

        // TODO: Make pipeline: SymbolId -> ExprId where ExprId has expression users, and the
        // symbol itself that depends on it is not touched, just mutated.
        //
        // Example:
        // let y = 2 + x
        // let x = 2
        //
        // The outcome of y would be BinaryExpr { 2, +, x } where x is an unknown value.
        //
        // When x is declared, it is searched for in the hashmap for if expression depended on it.
        // If there are any, they are then evaluated now that x is known.
        //
        // To avoid eagerly attempting
        // evaluation, expressions will keep all of their dependencies cached.
        // So given, let y = x + z, x and z are stored as dependencies and cache their own boolean of if
        // they've bene resolved.
        //
        // The only way I can currently see doing this is giving a vec to register_expr so that it
        // can build up any unknowns it finds, then it can be dealt with if !empty
        //TODO: What deps pipeline should be

        let symbol = &self.compiler.symbols[&sym_id];
        let name = self.interner.search((symbol.name_id.id as usize));
        let expr = &self.compiler.exprs[expr_id.id as usize];
        dbg!(
            &expr, // &self.compiler.values[expr.const_val.unwrap().id as usize],
            name
        );

        dbg!(&self.ty_ctx.expr_queue.get(&SymbolId::new(0)));
        panic!("I am outside");

        let resolved_expr = &self.compiler.exprs[expr_id.id as usize];

        let symbol = self.compiler.symbols.get_mut(&sym_id).expect("Exists");

        if self.compiler.values[resolved_expr.val_id.id as usize]
            .const_val
            .is_some()
        {
            symbol.kind = SymbolKind::Val(resolved_expr.val_id);
            let val = &self.compiler.values[resolved_expr.val_id.id as usize];
            dbg!(&self.compiler.types[val.type_id.id as usize]);
        }

        panic!("Inferring");

        let inferred_type_id = match self.type_check_and_infer(&resolved_expr.expr_hir) {
            Ok(type_id) => type_id,
            Err(sem_err) => {
                self.reporter
                    .report_semantic(sem_err, &self.compiler.mods[self.current_mod.id]);
                return Err(());
            }
        };

        //FIX: This needs to be done since value info needs to be stored and...um..
        let val_id = ValueId::new(self.compiler.values.len() as u32);
        let val_info = ValueInfo::new(inferred_type_id, expr_id, None);
        self.compiler.values.push(val_info);

        let symbol = self.compiler.symbols.get_mut(&sym_id).expect("Exists");
        symbol.kind = SymbolKind::Val(val_id);

        // if let Some(solved_expr) = self.ty_ctx.expr_queue.get_mut(&sym_id) {
        //     solved_expr.is_resolved = true;
        //
        //     for user_sym in solved_expr.users.iter().cloned() {
        //         let ast_id = self.compiler.symbols[&user_sym].ast_id;
        //         todo!();
        //         // match self.try_resolve_user(sym_id, ast_id) {
        //         //     Ok(expr_id_opt) => match expr_id_opt {
        //         //         Some(_) => todo!(),
        //         //         None => (),
        //         //     },
        //         //     Err(sem_err) => {
        //         //         let mod_id = &self.compiler.get_owner(user_sym);
        //         //         self.reporter
        //         //             .report_semantic(sem_err, &self.compiler.mods[mod_id.id as usize]);
        //         //
        //         //         return Err(());
        //         //     }
        //         // };
        //     }
        //     panic!();
        // }

        // dbg!(&self.compiler.values[val_id.id as usize]);
        // dbg!(&self.compiler.types[inferred_type_id.id as usize]);
        // panic!("Inferred");

        // let resolved_expr = self.compiler.exprs[expr_id.id as usize];
        // dbg!(&resolved_expr);
        // dbg!(
        //     &self.compiler.types[resolved_expr.type_id.id as usize],
        //     &resolved_expr.expr_hir,
        //     resolved_expr.const_val
        // );
        // dbg!(&self.ty_ctx);
        //
        // panic!("Hi");

        Ok(())
    }

    fn try_resolve_user(
        &mut self,
        sym_id: SymbolId,
        ast_id: AstId,
    ) -> Result<Option<ExprId>, SemanticError> {
        todo!()
    }

    //WARN: WEAK INFERENCE
    /// Infers type based off of expression
    fn type_check_and_infer(&self, expr_hir: &ExprHir) -> Result<TypeId, SemanticError> {
        match &expr_hir {
            ExprHir::Default(sym_id, expr_id) => todo!(),
            ExprHir::Val(val_id) => {
                //TODO: Not deeply searched
                let type_id = self.compiler.values[val_id.id as usize].type_id;
                Ok(type_id)
            }
            ExprHir::Var(sym_id) => match &self.compiler.symbols[&sym_id].kind {
                SymbolKind::Type(type_id) => Ok(*type_id),
                SymbolKind::Val(val_id) => Ok(self.compiler.values[val_id.id as usize].type_id),
                SymbolKind::Unknown => Ok(TypeId::new(script_compiler::TYPE_UNKNOWN_IDX)),
            },
            ExprHir::Unary { op, operand } => todo!(),
            ExprHir::BinaryExpr { lhs, op, rhs } => todo!(),
        }
    }

    fn register_expr(
        &mut self,
        sym_parent: SymbolId,
        spanned_expr: &SpannedExpr,
        scope_type: ScopeType,
    ) -> Result<ExprId, SemanticError> {
        match &spanned_expr.expr {
            Expr::Var(name_id) => {
                let module = &self.compiler.mods[self.current_mod.id];

                if let Some(sym_id) = module.get_sym_id(*name_id, scope_type) {
                    let symbol = &self.compiler.symbols[&sym_id];
                    let expr_id = ExprId::new(self.compiler.exprs.len() as u32);

                    let resolved_expr = match symbol.kind {
                        SymbolKind::Type(type_id) => {
                            let ty_info = &self.compiler.types[type_id.id as usize];
                            todo!("type symbol")
                        }
                        SymbolKind::Val(val_id) => {
                            let val_info = &self.compiler.values[val_id.id as usize];
                            let expr_hir = ExprHir::Var(sym_id);

                            ResolvedExpr::new(val_info.type_id, expr_hir, val_id)
                        }
                        SymbolKind::Unknown => {
                            // Will possibly call for others to be resolved here, or do it from the
                            // var resolution method itself
                            if let Some(pending_expr) = self.ty_ctx.expr_queue.get_mut(&sym_id) {
                                // pending_sym.users.push(value);
                                todo!();
                            }

                            // Pushes expression that is referencing an unresolved symbol into a
                            // queue to be resolved when the symbol is seen later
                            let expr_id = ExprId::new(self.compiler.exprs.len() as u32);
                            let expr_hir = ExprHir::Var(sym_id);
                            let pending_expr = PendingExpr::new(expr_id, false, sym_parent);

                            let type_id = TypeId::new(script_compiler::TYPE_UNKNOWN_IDX);

                            self.ty_ctx.expr_queue.insert(sym_id, vec![pending_expr]);

                            // Creates value id that has an unknown type, no constant value, and an
                            // unresolved expression.
                            let val_id = ValueId::new(self.compiler.values.len() as u32);
                            let val_info = ValueInfo::new(type_id, expr_id, None);

                            self.compiler.values.push(val_info);

                            ResolvedExpr::new(type_id, expr_hir, val_id)
                        }
                    };

                    self.compiler.exprs.push(resolved_expr);

                    Ok(expr_id)
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
                    // Getting what it's spot would be when it's expression and value parts are
                    // pushed
                    let expr_id = ExprId::new(self.compiler.exprs.len() as u32);
                    let val_id = ValueId::new(self.compiler.values.len() as u32);

                    // Creating it's default type to the literal value of integer, as well as it's
                    // expression of just being a singular value type
                    let expr_hir = ExprHir::Val(val_id);
                    let type_id = TypeId::new(BuiltinTypeKind::I128 as u32);

                    let resolved_expr = ResolvedExpr::new(type_id, expr_hir, val_id);

                    // Creating the actual value portion of the expression
                    let val = Value::I128(num);
                    let val_info = ValueInfo::new(type_id, expr_id, Some(val));

                    self.compiler.values.push(val_info);
                    self.compiler.exprs.push(resolved_expr);

                    Ok(expr_id)
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
                    let expr_id = ExprId::new(self.compiler.exprs.len() as u32);
                    let val_id = ValueId::new(self.compiler.values.len() as u32);

                    let expr_hir = ExprHir::Val(val_id);
                    let type_id = TypeId::new(BuiltinTypeKind::F64 as u32);
                    let expr = ResolvedExpr::new(type_id, expr_hir, val_id);

                    let val = Value::F64(num);
                    let val_info = ValueInfo::new(type_id, expr_id, Some(val));

                    self.compiler.values.push(val_info);
                    self.compiler.exprs.push(expr);

                    Ok(expr_id)
                } else {
                    Err(SemanticError::NumericOverflow(
                        *id,
                        Formatted::Float,
                        vec![spanned_expr.span],
                    ))
                }
            }
            Expr::BinaryExpr { lhs, op, rhs } => {
                let lhs_id = self.register_expr(sym_parent, &*lhs, scope_type)?;
                let rhs_id = self.register_expr(sym_parent, &*rhs, scope_type)?;

                let lhs_expr = &self.compiler.exprs[lhs_id.id as usize];
                let rhs_expr = &self.compiler.exprs[rhs_id.id as usize];

                let is_unknown = if lhs_expr.type_id.id == script_compiler::TYPE_UNKNOWN_IDX
                    || rhs_expr.type_id.id == script_compiler::TYPE_UNKNOWN_IDX
                {
                    true
                } else {
                    false
                };

                // if evaluator::is_compatible_binary(lhs, op, rhs) {
                //     panic!("Something mismatch");
                //     let full_span = lhs.span.merge(rhs.span);
                //
                //     todo!();
                //     // return Err(MathError::BinaryOpMismatch(
                //     //     lhs_val.kind().to_fmt(),
                //     //     rhs_val.kind().to_fmt(),
                //     //     op.to_fmt(),
                //     //     vec![full_span],
                //     // ))?;
                // }

                // Composing this so it can be matched cleanly for if const eval can be performed
                let lhs_val_opt = self.compiler.values[lhs_expr.val_id.id as usize]
                    .const_val
                    .as_ref();

                let rhs_val_opt = self.compiler.values[rhs_expr.val_id.id as usize]
                    .const_val
                    .as_ref();

                // This just checks if both are const, not if they were comptaible in the first
                // place. So, if it's not a comptaible binary, that could either mean 2 + "hi" or 2
                // + x where we just don't know x yet
                let const_val_opt: Option<Value> = match (lhs_val_opt, rhs_val_opt) {
                    (Some(lhs_const), Some(rhs_const)) => {
                        if !evaluator::is_compatible_binary(lhs_const, *op, rhs_const)
                            && !is_unknown
                        {
                            let full_span = lhs.span.merge(rhs.span);

                            return Err(MathError::BinaryOpMismatch(
                                lhs_const.kind().to_fmt(),
                                rhs_const.kind().to_fmt(),
                                op.to_fmt(),
                                vec![full_span],
                            ))?;
                        } else {
                            Some(evaluator::apply_binary_op(lhs_const, *op, rhs_const)?)
                        }
                    }
                    _ => None,
                };

                let val_id = ValueId::new(self.compiler.values.len() as u32);
                let expr_id = ExprId::new(self.compiler.exprs.len() as u32);

                let expr_hir = ExprHir::BinaryExpr {
                    lhs: lhs_id,
                    op: *op,
                    rhs: rhs_id,
                };

                //WARN: Assuming this means they're the same type, or at least, uh. Um. Yeah.
                let type_id = if const_val_opt.is_some() {
                    lhs_expr.type_id
                } else {
                    TypeId::new(script_compiler::TYPE_UNKNOWN_IDX)
                };

                // Expression points to the value so the expr_id is returned alone.
                let resolved_expr = ResolvedExpr::new(type_id, expr_hir, val_id);
                let val_info = ValueInfo::new(type_id, expr_id, const_val_opt);

                // dbg!(
                //     self.compiler.exprs[lhs_id.id as usize],
                //     self.compiler.exprs[rhs_id.id as usize],
                //     resolved_expr,
                //     val_info
                // );

                self.compiler.exprs.push(resolved_expr);
                self.compiler.values.push(val_info);

                Ok(expr_id)
            }
            // Um...Maybe this is a little much?
            Expr::Char(c) => {
                let expr_id = ExprId::new(self.compiler.exprs.len() as u32);
                let val_id = ValueId::new(self.compiler.values.len() as u32);

                let type_id = TypeId::new(BuiltinTypeKind::Char as u32);

                let val = Value::Char(*c);
                let val_info = ValueInfo::new(type_id, expr_id, Some(val));
                self.compiler.values.push(val_info);

                let expr_hir = ExprHir::Val(val_id);
                let resolved_expr = ResolvedExpr::new(type_id, expr_hir, val_id);

                self.compiler.exprs.push(resolved_expr);

                Ok(expr_id)
            }
            Expr::Default(name_id, spanned_expr) => {
                // DO NOT QUESTION THIS
                if self.interner.search(name_id.id as usize) == "_" {}

                todo!();
            }
            Expr::Str(name_id) => {
                let expr_id = ExprId::new(self.compiler.exprs.len() as u32);
                let val_id = ValueId::new(self.compiler.values.len() as u32);

                let type_id = TypeId::new(BuiltinTypeKind::Str as u32);

                let val = Value::InternedStr(*name_id);
                let val_info = ValueInfo::new(type_id, expr_id, Some(val));
                self.compiler.values.push(val_info);

                let expr_hir = ExprHir::Val(val_id);
                let resolved_expr = ResolvedExpr::new(type_id, expr_hir, val_id);
                self.compiler.exprs.push(resolved_expr);

                Ok(expr_id)
            }
            Expr::Call(caller, spanned_exprs) => {
                todo!();
            }
            Expr::MemberAccess(abs_member_access) => {
                match self.resolve_member(sym_parent, &abs_member_access.base, scope_type)? {
                    PossibleMember::Module(mod_id) => {
                        let extern_mod = &mut self.compiler.mods[mod_id.id];
                        if let Some(sym_id) =
                            extern_mod.get_sym_id(abs_member_access.field, scope_type)
                        {
                            let symbol = &self.compiler.symbols[&sym_id];
                            match symbol.kind {
                                SymbolKind::Type(type_id) => todo!(),
                                SymbolKind::Val(value_id) => todo!(),
                                SymbolKind::Unknown => todo!(),
                            }
                        } else {
                            todo!("Unresolved");
                        }
                    }
                    PossibleMember::Type(type_id) => {
                        todo!("Type id");
                    }
                    PossibleMember::Var(val_id) => {
                        ValueResult::Resolved(val_id);
                        unimplemented!("Nothing matches this case yet");
                    }
                    PossibleMember::Nothing => todo!("Unresolved"),
                }
            }
            Expr::Unary(unary) => {
                let operand_id = self.register_expr(sym_parent, &unary.spanned_expr, scope_type)?;

                let operand = &self.compiler.values[operand_id.id as usize];

                todo!("Operand no operating");
                // if !evaluator::is_compatible_unary(unary.op, operand) {
                //     return Err(MathError::UnaryOpMismatch(
                //         operand.kind().to_fmt(),
                //         unary.op.to_fmt(),
                //         vec![spanned_expr.span],
                //     ))?;
                // }

                // let val = evaluator::apply_unary_op(unary.op, operand)?;
                let val_id = ValueId::new(self.compiler.values.len() as u32);

                // self.compiler.values.push(val);

                todo!();
            }
            Expr::Bool(boolean) => {
                //FIX:
                if *boolean == true {
                    Ok(ExprId::new(script_compiler::VALUE_TRUE_POS as u32))
                } else {
                    Ok(ExprId::new(script_compiler::VALUE_FALSE_POS as u32))
                }
            }
        }
    }

    fn resolve_member(
        &mut self,
        sym_parent: SymbolId,
        member: &SpannedExpr,
        scope_type: ScopeType,
    ) -> Result<PossibleMember, SemanticError> {
        if let Ok(expr_id) = self.register_expr(sym_parent, member, scope_type) {
            let resolved_expr = &self.compiler.exprs[expr_id.id as usize];

            todo!();
        }

        if let Expr::Var(name_id) = member.expr {
            if let Some(mod_id) = self.compiler.mod_map.get(&name_id) {
                return Ok(PossibleMember::Module(*mod_id));
            }

            let module = &self.compiler.mods[self.current_mod.id];
            if let Some(sym_id) = module.get_sym_id(name_id, scope_type) {
                todo!();
                // let type_id = self.compiler.symbols[&sym_id];
                // return Ok(PossibleMember::Type(type_id));
            } else {
                if name_id.id == Keyword::Self_ as u32 {
                    panic!();
                }
                // What if this was in order of priority
                // No
                // Dot reference sounds better
                let msg = format!(
                    "Could not find the symbol `{}` as a module, type, or value",
                    self.interner.search(name_id.id as usize)
                );

                return Err(SemanticError::General(msg, vec![member.span]));
            }
        }

        Err(SemanticError::UndefinedMember(member.span))
    }

    fn resolve_typedef(&mut self, abs_typedef: &AbstractTypeDef, ast_id: AstId) -> Result<(), ()> {
        let type_id =
            self.resolve_type_expr(&abs_typedef.spanned_ty_expr, ScopeType::Var, ast_id)?;

        let module = &mut self.compiler.mods[self.current_mod.id];
        let scope_id = module.extract_scope_id(ScopeType::Var);
        let table = &mut module.get_scope_mut(scope_id).table;

        let sym_id = table.sym_ids[&ast_id];

        // Assinging from `Unknown` to it's actual type
        let type_def = self.compiler.get_typedef_mut(sym_id);
        type_def.type_id = type_id;

        Ok(())
    }

    fn resolve_struct(&mut self, abs_struct: &AbstractStruct, ast_id: AstId) -> Result<(), ()> {
        let mut fields: Vec<FieldRepre> = Vec::new();
        let mut seen: Vec<(usize, InternedId)> = Vec::new();

        // Checking if there are duplicate name ids within the same struct along with resolution
        for (i, type_def) in abs_struct.fields.iter().enumerate() {
            let type_id =
                self.resolve_type_expr(&type_def.spanned_ty_expr, ScopeType::Nest, ast_id)?;

            if let Some(original) = seen.iter().find(|other| type_def.name_id == other.1) {
                let struct_name = self.interner.search(abs_struct.name_id.id as usize);
                let dup_name = self.interner.search(type_def.name_id.id as usize);

                let orig_span = abs_struct.fields[original.0].name_span;
                let field_span = abs_struct.fields[i].name_span;

                let msg = format!(
                    "More than one field has the identifier \"{dup_name}\" within struct \"{struct_name}\""
                );

                self.reporter.report_spanned(
                    &msg,
                    None,
                    &[orig_span, field_span],
                    &self.compiler.mods[self.current_mod.id],
                );
            }

            seen.push((i, type_def.name_id));

            let field_repre = FieldRepre::new(type_def.name_id, type_id, AstId::new(i as u32));

            fields.push(field_repre);
        }

        let module = &mut self.compiler.mods[self.current_mod.id];
        let scope_id = module.extract_scope_id(ScopeType::Nest);
        let table = &module.get_scope_mut(scope_id).table;

        let sym_id = table.sym_ids[&ast_id];

        let struct_def = self.compiler.get_struct_mut(sym_id);
        struct_def.fields.append(&mut fields);

        Ok(())
    }

    fn resolve_enum(&mut self, abs_enum: &AbstractEnum, ast_id: AstId) -> Result<(), ()> {
        let mut variants: Vec<VariantRepre> = Vec::new();

        // (ast_id, name_id)
        let mut seen: Vec<(usize, InternedId)> = Vec::new();
        //Maybe just compute this once after along with struct fields

        // Checking if there are duplicate name ids within the same enum
        for (i, variant) in abs_enum.variants.iter().enumerate() {
            if let Some(original) = seen.iter().find(|other| variant.name_id == other.1) {
                let enum_name = self.interner.search(abs_enum.name_id.id as usize);
                let dup_name = self.interner.search(variant.name_id.id as usize);

                let orig_span = abs_enum.variants[original.0].name_span;
                let variant_span = abs_enum.variants[i].name_span;

                let msg = format!(
                    "More than one variant has the identifier \"{dup_name}\" within enum \"{enum_name}\""
                );

                self.reporter.report_spanned(
                    &msg,
                    None,
                    &[orig_span, variant_span],
                    &self.compiler.mods[self.current_mod.id],
                );
            }

            seen.push((i, variant.name_id));

            if let Some(spanned_ty_expr) = &variant.ty_expr {
                let type_id = self.resolve_type_expr(&spanned_ty_expr, ScopeType::Nest, ast_id)?;

                let variant_repre =
                    VariantRepre::new(variant.name_id, Some(type_id), AstId::new(i as u32));

                variants.push(variant_repre);
            }
        }

        let module = &mut self.compiler.mods[self.current_mod.id];
        let scope_id = module.extract_scope_id(ScopeType::Nest);
        let table = &module.get_scope_mut(scope_id).table;

        let sym_id = table.sym_ids[&ast_id];
        let enum_def = self.compiler.get_enum_mut(sym_id);

        enum_def.variants.append(&mut variants);

        Ok(())
    }

    fn resolve_alias(&mut self, abs_alias: &AbstractAlias, ast_id: AstId) -> Result<(), ()> {
        // Should the variable check happen here?
        let mut params: Vec<TypeId> = Vec::new();
        for (i, spanned_ty_expr) in abs_alias.params.iter().enumerate() {
            let type_id = self.resolve_type_expr(&spanned_ty_expr, ScopeType::Neutral, ast_id)?;
            params.push(type_id);
        }
        dbg!(&params);
        todo!();
    }

    fn resolve_type_expr(
        &mut self,
        spanned_ty_expr: &SpannedTypeExpr,
        scope_type: ScopeType,
        ast_id: AstId,
    ) -> Result<TypeId, ()> {
        match &spanned_ty_expr.ty_expr {
            TypeExpr::Var(name_id) => {
                // Returns the name's id since it is a valid non-data structure intrinsic type
                if let Some(ty) = BuiltinType::try_from_interned_id(name_id.id) {
                    // This technically relies on the original pushing of values being in order so
                    // may also be changed to a const idx but fine for iteration purposes
                    return Ok(TypeId::new(ty.kind() as u32));
                }

                let module = &self.compiler.mods[self.current_mod.id];

                // Loop that checks if the name id was registered in a valid scope, then uses its
                // corresponding ast_id to extract the name id's type and returns that
                // as the type to be referenced
                //WARN:
                if let Some(sym_id) = module.get_sym_id(*name_id, scope_type) {
                    let type_id = match self.compiler.symbols[&sym_id].kind {
                        SymbolKind::Type(type_id) => type_id,
                        SymbolKind::Unknown => unreachable!("Not possible yet. Yet."),
                        SymbolKind::Val(val_id) => unreachable!("Values are not resolved yet"),
                    };

                    return Ok(type_id);
                }

                let err_name = self.interner.search(name_id.id as usize);

                let err_msg = format!("\"{err_name}\" is not defined as a type");

                self.reporter.report_spanned(
                    &err_msg,
                    Some(err_name),
                    &[spanned_ty_expr.span],
                    module,
                );

                return Err(());
            }
            TypeExpr::Escaped(name_id) => {
                let module = &self.compiler.mods[self.current_mod.id];

                if let Some(sym_id) = module.get_sym_id(*name_id, scope_type) {
                    let type_id = match self.compiler.symbols[&sym_id].kind {
                        SymbolKind::Type(type_id) => type_id,
                        SymbolKind::Unknown => unreachable!("Not possible yet. Yet."),
                        SymbolKind::Val(val_id) => unreachable!("Values are not resolved yet"),
                    };

                    return Ok(type_id);
                }

                let err_name = self.interner.search(name_id.id as usize);

                let err_msg = format!("\"{err_name}\" is not defined as a type");

                self.reporter
                    .report_spanned(&err_msg, None, &[spanned_ty_expr.span], &module);

                return Err(());
            }
            TypeExpr::Generic(generic) => {
                match BuiltinTypeKind::try_from_interned_id(generic.base.id) {
                    // Self referential type ids used here
                    Some(kw) => match kw {
                        //TODO: Should maybe put List | Set
                        BuiltinTypeKind::List => {
                            if generic.args.len() != 1 {
                                let msg = format!(
                                    "Expected 1 type within `List`, found {}",
                                    generic.args.len()
                                );

                                self.reporter.report_spanned(
                                    &msg,
                                    None,
                                    &[spanned_ty_expr.span],
                                    &self.compiler.mods[self.current_mod.id],
                                );

                                return Err(());
                            }

                            let inner =
                                self.resolve_type_expr(&generic.args[0], scope_type, ast_id)?;

                            let list = Type::BuiltinType(BuiltinType::List(inner));
                            let list_id = TypeId::new(self.compiler.types.len() as u32);

                            let ty_info = TypeInfo::new(list, Some(self.current_mod));
                            self.compiler.types.push(ty_info);

                            return Ok(list_id);
                        }
                        BuiltinTypeKind::Tuple => {
                            let mut elements: Vec<TypeId> = Vec::new();

                            panic!("Which tuple");
                            for arg in &generic.args {
                                elements.push(self.resolve_type_expr(arg, scope_type, ast_id)?);
                            }

                            let type_id = TypeId::new(self.compiler.types.len() as u32);
                            let tuple = Type::Tuple(Tuple::new(elements));

                            let ty_info = TypeInfo::new(tuple, Some(self.current_mod));
                            self.compiler.types.push(ty_info);

                            Ok(type_id)
                        }
                        BuiltinTypeKind::Map => {
                            if generic.args.len() != 2 {
                                let msg = format!(
                                    "Expected 2 types within `Map`, found {}",
                                    generic.args.len()
                                );

                                self.reporter.report_spanned(
                                    &msg,
                                    None,
                                    &[spanned_ty_expr.span],
                                    &self.compiler.mods[self.current_mod.id],
                                );

                                return Err(());
                            }

                            let key =
                                self.resolve_type_expr(&generic.args[0], scope_type, ast_id)?;
                            let val =
                                self.resolve_type_expr(&generic.args[1], scope_type, ast_id)?;

                            let map = Type::BuiltinType(BuiltinType::Map(key, val));
                            let map_id = self.compiler.types.len() as u32;

                            let ty_info = TypeInfo::new(map, Some(self.current_mod));
                            self.compiler.types.push(ty_info);

                            Ok(TypeId::new(map_id))
                        }
                        // Should probably just put this with list
                        BuiltinTypeKind::Set => {
                            if generic.args.len() != 1 {
                                let msg = format!(
                                    "Expected 1 type within `Set`, found {}",
                                    generic.args.len()
                                );

                                self.reporter.report_spanned(
                                    &msg,
                                    None,
                                    &[spanned_ty_expr.span],
                                    &self.compiler.mods[self.current_mod.id],
                                );

                                return Err(());
                            }

                            let inner =
                                self.resolve_type_expr(&generic.args[0], scope_type, ast_id)?;

                            let set = Type::BuiltinType(BuiltinType::Set(inner));
                            let set_id = TypeId::new(self.compiler.types.len() as u32);

                            let ty_info = TypeInfo::new(set, Some(self.current_mod));
                            self.compiler.types.push(ty_info);

                            return Ok(set_id);
                        }
                        // I'm sure this can be done better...
                        _ => {
                            let err_name = self.interner.search(generic.base.id as usize);
                            //WARN: Questionablly phrased error message
                            //This COULD change so this will not be upheld at the parsing stage
                            let err_msg = format!(
                                "Found identifier \"{err_name}\" before generic parameters, but only `List`, `Set`, `Tuple`, and `Map` are valid data structures"
                            );

                            self.reporter.report_spanned(
                                &err_msg,
                                Some(err_name),
                                &[spanned_ty_expr.span],
                                &self.compiler.mods[self.current_mod.id],
                            );

                            Err(())
                        }
                    },
                    None => {
                        // 2004 dog 2004 television
                        let err_name = self.interner.search(generic.base.id as usize);

                        let err_msg = format!(
                            "Found identifier \"{err_name}\" before generic parameters, but only `List`, `Set`, `Tuple`, and `Map` are valid data structures"
                        );

                        self.reporter.report_spanned(
                            &err_msg,
                            Some(err_name),
                            &[spanned_ty_expr.span],
                            &self.compiler.mods[self.current_mod.id],
                        );

                        Err(())
                    }
                }
            }
            TypeExpr::Any => {
                let id = self.compiler.types.len() as u32;

                let ty_info = TypeInfo::new(
                    Type::BuiltinType(BuiltinType::Any(None)),
                    Some(self.current_mod),
                );

                self.compiler.types.push(ty_info);

                Ok(TypeId::new(id))
            }
            TypeExpr::Tuple(unres_tuple) => {
                let mut elements: Vec<TypeId> = Vec::new();

                for element in unres_tuple {
                    let type_id = self.resolve_type_expr(element, scope_type, ast_id)?;
                    elements.push(type_id);
                }

                let tuple_id = TypeId::new(self.compiler.types.len() as u32);
                let tuple = Type::Tuple(Tuple::new(elements));
                panic!("Intrinsic stuff");

                let ty_info = TypeInfo::new(tuple, Some(self.current_mod));
                self.compiler.types.push(ty_info);

                Ok(tuple_id)
            }
            //FIX: Need to make sure MAYBE that the type referenced isn't a builtin one
            TypeExpr::Path(spanned_ty_exprs) => {
                // The parser disallows < 2 type pathing to actually exist so indexing should be
                // safe here
                if spanned_ty_exprs.len() != 2 {
                    let msg = format!(
                        "Only 1 dot reference can be used for types but {} were found",
                        spanned_ty_exprs.len() - 1
                    );

                    let spans: Vec<Span> = spanned_ty_exprs
                        .iter()
                        .skip(1)
                        .map(|expr| expr.span)
                        .collect();

                    self.reporter.report_spanned(
                        &msg,
                        None,
                        &spans,
                        &self.compiler.mods[self.current_mod.id],
                    );
                }

                let extern_mod = match &spanned_ty_exprs[0].ty_expr {
                    TypeExpr::Var(name_id) => {
                        if let Some(mod_id) = self.compiler.mod_map.get(name_id) {
                            &self.compiler.mods[mod_id.id]
                        } else {
                            let err_name = self.interner.search(name_id.id as usize);
                            let msg = format!("The module `{err_name}` does not exist");

                            self.reporter.report_spanned(
                                &msg,
                                None,
                                &[spanned_ty_exprs[0].span],
                                &self.compiler.mods[self.current_mod.id],
                            );

                            return Err(());
                        }
                    }
                    _ => unreachable!("Parser does not pick this up"),
                };

                let name_id = match &spanned_ty_exprs[1].ty_expr {
                    TypeExpr::Var(name_id) | TypeExpr::Escaped(name_id) => name_id,
                    _ => unreachable!("Parser does not pick this up"),
                };

                if let Some(sym_id) = extern_mod.get_sym_id(*name_id, scope_type) {
                    let sym_info = &self.compiler.symbols[&sym_id];

                    //WARN: Only scoping issue left is alias and const collision and maybe some
                    //others
                    let type_id = match sym_info.kind {
                        SymbolKind::Type(type_id) => type_id,
                        _ => {
                            // Suspicious error message
                            let msg = format!(
                                "Only `enum` and `struct` can be used as type path annotated references",
                            );

                            self.reporter.report_spanned(
                                &msg,
                                None,
                                &[spanned_ty_exprs[1].span],
                                &self.compiler.mods[self.current_mod.id],
                            );

                            return Err(());
                        }
                    };

                    if sym_info.is_priv && sym_info.owner != self.current_mod {
                        let err_name = self.interner.search(name_id.id as usize);
                        let msg = format!("The type `{err_name}` is private",);

                        self.reporter.report_spanned(
                            &msg,
                            None,
                            &[spanned_ty_exprs[1].span],
                            &self.compiler.mods[self.current_mod.id],
                        );
                    }

                    // HAPPY PATH DONE
                    return Ok(type_id);
                }

                // No matching namespace within the module given was found for name_id

                let err_name = self.interner.search(name_id.id as usize);
                let err_mod_name = self.interner.search(extern_mod.name_id.id as usize);

                // FIND SIMILAR CAN BE DONE, IT CAN BE DONE later.
                let msg = format!(
                    "The type `{err_name}` does not exist within the module `{err_mod_name}`",
                );

                self.reporter.report_spanned(
                    &msg,
                    None,
                    &[spanned_ty_exprs[1].span],
                    &self.compiler.mods[self.current_mod.id],
                );

                Err(())
            }
        }
    }
}
