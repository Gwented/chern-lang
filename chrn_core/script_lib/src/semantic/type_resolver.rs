pub mod type_context;

use std::collections::HashSet;

use chrn_utils::builtins::BuiltinTypeKind;
use chrn_utils::id_types::{AstId, ExprId, InternedId, ModuleId, SymbolId, TypeId, ValueId};
use chrn_utils::intern;
use chrn_utils::values::{Value, ValueInfo, ValueResult};
use chrn_utils::{builtins::BuiltinType, intern::Intern};
use common::chrn_settings::ChrnSettings;
use common::fmter::{Formattable, Formatted};
use common::{reporter::diagnostic::Diagnostic, span::Span};

use crate::parser::ast::{AbstractVar, Expr, SpannedExpr};
use crate::script_compiler::{self, ScriptCompiler};
use crate::semantic::error::{MathError, SemanticError};
use crate::semantic::evaluator;
use crate::semantic::representation::{ExprHir, Param, PossibleMember, ResolvedExpr, SymbolKind};
use crate::semantic::scopes::ScopeType;
use crate::semantic::type_resolver::type_context::{PendingExpr, PendingSymbol, TypeContext};

use crate::{
    parser::ast::{
        AbstractAlias, AbstractEnum, AbstractStruct, AbstractTypeDef, AstInfo, Item,
        SpannedTypeExpr, TypeExpr,
    },
    semantic::{
        representation::{FieldRepre, Type, TypeInfo, VariantRepre},
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
        settings: &'a ChrnSettings,
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
                Item::Var(abs_var) => _ = self.resolve_var(abs_var, ast_id),
            }
        }

        // Maybe shouldn't in-line this
        if self.ty_ctx.needs_check {
            // Clearing cache.
            self.ty_ctx.needs_check = false;

            // Giving ownership to a variable since the traversal chosen needs mutation while
            // traversing
            let mut pending_syms: Vec<(SymbolId, PendingSymbol)> = Vec::new();
            pending_syms.extend(self.ty_ctx.sym_queue.drain());

            let mut removable_syms: HashSet<SymbolId> = HashSet::new();

            for (sym_id, pending_sym) in &pending_syms {
                if !pending_sym.is_resolved {
                    continue;
                }

                match self.try_resolve_pending(*sym_id, pending_sym) {
                    Ok(can_remove) => {
                        if can_remove {
                            removable_syms.insert(*sym_id);
                        }
                    }
                    // Uhhh not sure what to do here
                    Err(_) => {}
                };
            }

            self.ty_ctx.sym_queue.extend(pending_syms);

            //TEMP or not I don't know
            for sym_id in removable_syms {
                self.ty_ctx.sym_queue.remove(&sym_id);
            }

            //FIXME:
            //FIX:
            // Not sure what to do with this yet
            //
            // This means the resolution failed at last module pass
            // if !self.ty_ctx.sym_queue.is_empty()
            //     && self.current_mod == self.compiler.mods[self.compiler.mods.len() - 1].mod_id
            // {
            //     dbg!(&self.ty_ctx);
            //     panic!("How to check if this is runtime..");
            // }
        }
        //
        // let symbol = &self.compiler.symbols[&SymbolId::new(0)];
        // match symbol.kind {
        //     SymbolKind::Type(type_id) => {
        //         let name = self.interner.search(symbol.name_id.id as usize);
        //         let ty = &self.compiler.types[type_id.id as usize];
        //         dbg!(name, &ty.ty);
        //     }
        //     SymbolKind::Val(value_id) => {
        //         let name = self.interner.search(symbol.name_id.id as usize);
        //         let val_info = &self.compiler.values[value_id.id as usize];
        //         let ty_info = &self.compiler.types[val_info.type_id.id as usize];
        //
        //         dbg!(name, ty_info);
        //     }
        //     _ => todo!(),
        // };
        //
        // if self.current_mod == self.compiler.mods[self.compiler.mods.len() - 1].mod_id {
        //     for symbol in self.compiler.symbols.values() {
        //         if self.interner.search(symbol.name_id.id as usize) == "x" {
        //             let name = self.interner.search(symbol.name_id.id as usize);
        //             dbg!(name);
        //             match symbol.kind {
        //                 SymbolKind::Val(value_id) => {
        //                     let val = &self.compiler.values[value_id.id as usize];
        //                     dbg!(value_id, val);
        //                     let expr = &self.compiler.exprs[val.expr_id.id as usize];
        //                     dbg!(expr.val_id, expr);
        //                     dbg!(&self.compiler.values[expr.val_id.id as usize]);
        //                 }
        //                 SymbolKind::Type(type_id) => todo!(),
        //                 SymbolKind::Unknown => todo!(),
        //             }
        //             panic!("Done");
        //         }
        //         // dbg!(self.interner.search(symbol.name_id.id as usize));
        //         // dbg!(symbol);
        //     }
        //
        //     for ty in &self.compiler.types {
        //         dbg!(ty);
        //     }
        //
        //     for expr_thing in &self.compiler.exprs {
        //         dbg!(expr_thing);
        //     }
        //
        //     for val in &self.compiler.values {
        //         dbg!(val);
        //     }
        // }
        //
        if !self.reporter.err_vec.is_empty() {
            let mut diags = Vec::new();
            diags.append(&mut self.reporter.err_vec);

            return Err(diags);
        }

        Ok(())
    }

    fn try_resolve_pending(
        &mut self,
        resolved_sym_id: SymbolId,
        pending_sym: &PendingSymbol,
    ) -> Result<bool, ()> {
        // Tells the caller if the given pending symbol is fully resolved to where it can be
        // removed as a pending symbol
        let mut can_remove = false;
        let mut queue: Vec<ExprId> = Vec::new();

        for pending_expr in &pending_sym.pending_exprs {
            let expr = &self.compiler.exprs[pending_expr.pending_id.id as usize];
            let is_solvable = expr.inputs.iter().all(|inner_expr_id| {
                let inner_type_id = self.compiler.exprs[inner_expr_id.id as usize].type_id;
                inner_type_id.id != script_compiler::TYPE_UNKNOWN_IDX
            });

            if !is_solvable {
                continue;
            }

            queue.push(pending_expr.pending_id);
        }
        dbg!(&queue);

        // In the example:
        //
        // ```
        // let y = x
        // let x = 2
        // ```
        //
        // root_expr = x
        // So, it needs to go x -> x + 2 -> y
        //

        // Tracking how many were resolved so it knows whether to remove or not
        let mut resolved_count = 0;

        // Needs to resolve first root
        for root_id in queue.iter().copied() {
            // Still need to repair root expr
            let root_expr = &mut self.compiler.exprs[root_id.id as usize];
            match self.compiler.symbols[&resolved_sym_id].kind {
                SymbolKind::Type(type_id) => todo!("Hi types"),
                SymbolKind::Val(val_id) => {
                    // Brain starting working now it works
                    let val_info = &self.compiler.values[val_id.id as usize];
                    let type_id = val_info.type_id;
                    // WARN: It's only 32 bytes so trying to see something here
                    // If there are no users then I could have the symbol just point to the same
                    // val_id since that would avoid said clone but fine for now
                    let const_val_opt = val_info.const_val.clone();

                    root_expr.type_id = type_id;
                    let inner_val = &mut self.compiler.values[root_expr.val_id.id as usize];
                    inner_val.type_id = type_id;
                    inner_val.const_val = const_val_opt;
                }
                // Uh is this possible
                SymbolKind::Unknown => todo!("Hi unknowns"),
            }

            if let Some(user) = root_expr.user {
                // dbg!(user);
                // panic!("Stop");
                // If this is not done then it would stay 0 even though this by default would mean
                // that there was 100% only 1 symbol expression found
                match self.traverse_expr(user) {
                    Ok(true) => resolved_count += 1,
                    // WARN: Works suspiciously well
                    Ok(false) => (),
                    // Reports the error and continues
                    Err(sem_err) => {
                        // Extracting module of origin from the pending expression by using the symbol
                        // attached to the expression upon it's creation
                        let parent_sym_id = pending_sym.pending_exprs[0].parent_sym;
                        let mod_id = self.compiler.get_owner(parent_sym_id);
                        self.reporter
                            .report_semantic(sem_err, &self.compiler.mods[mod_id.id as usize])
                    }
                };
            } else {
                // If the root has no users, then that means its, let y = x where there is nothing else
                // that needs resolution since the root is always a symbol.
                resolved_count += 1;
                break;
            }
        }

        // If all pending expressions were pushed into the queue and the entire queue was resolved then can
        // remove
        if queue.len() == pending_sym.pending_exprs.len() && resolved_count == queue.len() {
            can_remove = true;
        }

        Ok(can_remove)
    }

    /// Returns an `Ok(true)` upon fully resolving a tree of expressions.
    /// Returns an `Ok(false)` if the resolution failed because a value was unknown.
    /// Returns `Err` upon real user errors.
    /// Method to recursively mutate tree of unresolved expression
    /// This works as user -> user -> ... -> None
    // This needs to go from x -> x + 2 -> y recursively however long needed
    fn traverse_expr(&mut self, current_expr_id: ExprId) -> Result<bool, SemanticError> {
        let expr = &mut self.compiler.exprs[current_expr_id.id as usize];

        match expr.expr_hir {
            ExprHir::Val(val_id) => {
                let val_info = &self.compiler.values[val_id.id as usize];

                let type_id = val_info.type_id;
                let const_val_opt = val_info.const_val.clone();

                expr.type_id = type_id;

                let inner_val = &mut self.compiler.values[expr.val_id.id as usize];
                inner_val.type_id = type_id;
                inner_val.const_val = const_val_opt;
                dbg!(inner_val);
                todo!("Make sure this is ok")
            }
            // I don't think this is possible?
            //TODO:
            ExprHir::Var(sym_id) => {
                todo!("What is a varrrble")
            }
            //TODO:
            ExprHir::Default(sym_id, expr_id) => {
                todo!()
            }
            ExprHir::Unary { op, operand } => {
                // Getting the operand that could be resolved (Might be guarnteed but um..e)
                let operand_expr = &self.compiler.exprs[operand.id as usize];

                let is_unknown = operand_expr.type_id.id == script_compiler::TYPE_UNKNOWN_IDX;

                // This means that we reached an expression inside of a resolved expression that is
                // not fully resolved yet, which is fine
                if is_unknown {
                    return Ok(false);
                }

                let operand_val_info = &self.compiler.values[operand_expr.val_id.id as usize];

                // Basic validation of expression to see if it's const or runtime
                let const_val_opt = if let Some(const_val) = &operand_val_info.const_val {
                    if !evaluator::is_compatible_unary(op, const_val) {
                        return Err(MathError::UnaryOpMismatch(
                            const_val.kind().to_fmt(),
                            op.to_fmt(),
                            vec![operand_expr.span],
                        ))?;
                    } else {
                        Some(evaluator::apply_unary_op(op, const_val)?)
                    }
                } else {
                    None
                };

                let type_id = operand_expr.type_id;

                // Mutating expression's type so that the symbol using this expr reflects the new
                // information
                let expr = &mut self.compiler.exprs[current_expr_id.id as usize];
                expr.type_id = type_id;

                // Mutating inner value so that the symbol using this value reflects the new
                // information
                let inner_val = &mut self.compiler.values[expr.val_id.id as usize];
                inner_val.type_id = type_id;
                inner_val.const_val = const_val_opt;
            }
            ExprHir::BinaryExpr { lhs, op, rhs } => {
                //TODO: Considering a span vector so that they dont need to be duplicated or
                //computed by going inside items anymore.

                let lhs_expr = &self.compiler.exprs[lhs.id as usize];
                let rhs_expr = &self.compiler.exprs[rhs.id as usize];
                dbg!(lhs_expr, rhs_expr);

                // Not sure if thisi s possible issss
                let is_unknown = lhs_expr.type_id.id == script_compiler::TYPE_UNKNOWN_IDX
                    || rhs_expr.type_id.id == script_compiler::TYPE_UNKNOWN_IDX;

                // This means that we reached an expression inside of a resolved expression that is
                // not fully resolved yet
                if is_unknown {
                    return Ok(false);
                }

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
                        // If cannot perform operation and neither are unknown then there is actual
                        // corruption, and not one part just being unresolved
                        if !evaluator::is_compatible_binary(lhs_const, op, rhs_const) {
                            //TODO: Expressions need spans
                            // Removal of pending symbols need
                            let full_span = lhs_expr.span.merge(rhs_expr.span);

                            return Err(MathError::BinaryOpMismatch(
                                lhs_const.kind().to_fmt(),
                                rhs_const.kind().to_fmt(),
                                op.to_fmt(),
                                vec![full_span],
                            ))?;
                        } else {
                            Some(evaluator::apply_binary_op(lhs_const, op, rhs_const)?)
                        }
                    }
                    _ => None,
                };

                //WARN: Suspicious
                let type_id = lhs_expr.type_id;

                //NOTE: Only the type of the expression is altered here, the rest is the inner
                //value
                let expr = &mut self.compiler.exprs[current_expr_id.id as usize];
                expr.type_id = type_id;

                let inner_val = &mut self.compiler.values[expr.val_id.id as usize];
                inner_val.type_id = type_id;
                inner_val.const_val = const_val_opt;
            }
        }

        // Uh huh
        let expr = &mut self.compiler.exprs[current_expr_id.id as usize];
        if let Some(user) = expr.user {
            self.traverse_expr(user)?;
        }

        Ok(true)
    }

    fn resolve_var(&mut self, abs_var: &AbstractVar, ast_id: AstId) -> Result<(), ()> {
        let module = &mut self.compiler.mods[self.current_mod.id];
        let scope_id = module.extract_scope_id(ScopeType::Neutral);
        let table = &mut module.get_scope_mut(scope_id).table;

        let sym_id = table.sym_ids[&ast_id];

        //NOTE: Pipeline where expressions are always returned, just that some may have
        //unresolved parts, which are put into the queue, not the variable itself.
        let expr_id = match self.register_expr(sym_id, &abs_var.spanned_expr, ScopeType::Neutral) {
            Ok(expr_id) => expr_id,
            Err(sem_err) => {
                self.reporter
                    .report_semantic(sem_err, &self.compiler.mods[self.current_mod.id]);
                return Err(());
            }
        };

        let expr = &self.compiler.exprs[expr_id.id as usize];
        // dbg!(expr);
        // dbg!(&self.compiler.values[expr.val_id.id as usize]);
        let is_unknown = expr.type_id.id == script_compiler::TYPE_UNKNOWN_IDX;

        // let inferred_type_id = match self.type_check(&expr.expr_hir) {
        //     Ok(type_id) => type_id,
        //     Err(sem_err) => {
        //         self.reporter
        //             .report_semantic(sem_err, &self.compiler.mods[self.current_mod.id]);
        //         return Err(());
        //     }
        // };
        let val_id = expr.val_id;

        // Sets the symbol's value to be the last expression's value so that later, if it's
        // expression is resolved further, since it's already pointing the the same expression it
        // will by proxy be updated
        let symbol = self.compiler.symbols.get_mut(&sym_id).expect("Exists");
        symbol.kind = SymbolKind::Val(val_id);

        // If the symbol that was just examined is a pending symbol AND it was actually resolved,
        // then it'll be marked as resolved
        if let Some(pending_sym) = self.ty_ctx.sym_queue.get_mut(&sym_id)
            && !is_unknown
        {
            // Two caches
            //FIX: This maybe needs to be checked elsewhere too
            pending_sym.is_resolved = true;
            self.ty_ctx.needs_check = true;
        }

        Ok(())
    }

    //WARN: WEAK INFERENCE
    /// Infers type based off of expression
    fn type_check(&self, expr_hir: &ExprHir) -> Result<(), SemanticError> {
        match &expr_hir {
            ExprHir::Default(sym_id, expr_id) => todo!(),
            ExprHir::Val(val_id) => {
                let type_id = self.compiler.values[val_id.id as usize].type_id;
                Ok(())
            }
            ExprHir::Var(sym_id) => match &self.compiler.symbols[&sym_id].kind {
                SymbolKind::Type(type_id) => Ok(()),
                SymbolKind::Val(val_id) => Ok(()),
                SymbolKind::Unknown => Ok(()),
            },
            ExprHir::Unary { op, operand } => {
                let operand_expr = &self.compiler.exprs[operand.id as usize];
                if operand_expr.type_id.id == script_compiler::TYPE_UNKNOWN_IDX {
                    return Ok(());
                }

                todo!()
            }
            ExprHir::BinaryExpr { lhs, op, rhs } => {
                let lhs_type_id = self.compiler.exprs[lhs.id as usize].type_id;
                let rhs_type_id = self.compiler.exprs[rhs.id as usize].type_id;

                if lhs_type_id.id == script_compiler::TYPE_UNKNOWN_IDX
                    || rhs_type_id.id == script_compiler::TYPE_UNKNOWN_IDX
                {
                    return Ok(());
                }

                todo!()
            }
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
                    if sym_id == sym_parent {
                        let name = self
                            .interner
                            .search(self.compiler.symbols[&sym_id].name_id.id as usize);
                        let msg = format!("Cannot declare symbol `{name}` as itself");

                        let parent_ast_id = self.compiler.symbols[&sym_parent].ast_id;
                        let parent_span = self.ast_info.get_sym_span(parent_ast_id);

                        return Err(SemanticError::General(
                            msg,
                            vec![parent_span, spanned_expr.span],
                        ));
                    }

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

                            ResolvedExpr::new(
                                val_info.type_id,
                                expr_hir,
                                val_id,
                                spanned_expr.span,
                                Vec::new(),
                            )
                        }
                        SymbolKind::Unknown => {
                            let expr_id = ExprId::new(self.compiler.exprs.len() as u32);
                            let expr_hir = ExprHir::Var(sym_id);
                            let pending_expr = PendingExpr::new(expr_id, sym_parent);

                            //NOTE: ONLY THIS POINT SHOULD STORE THE SYMBOL. This is how the
                            //connection is made so that, y = x + 2, goes from x -> x + 2 -> y
                            //after x is resolved.
                            self.ty_ctx.store_pending_expr(sym_id, pending_expr);
                            // Will possibly call for others to be resolved here, or do it from the
                            // var resolution method itself

                            let type_id = TypeId::new(script_compiler::TYPE_UNKNOWN_IDX);

                            // Creates value id that has an unknown type, no constant value, and an
                            // unresolved expression.
                            let val_id = ValueId::new(self.compiler.values.len() as u32);
                            let val_info = ValueInfo::new(type_id, expr_id, None);

                            self.compiler.values.push(val_info);

                            ResolvedExpr::new(
                                type_id,
                                expr_hir,
                                val_id,
                                spanned_expr.span,
                                Vec::new(),
                            );

                            // Pushes expression that is referencing an unresolved symbol into a
                            // queue to be resolved when the symbol is seen later

                            let type_id = TypeId::new(script_compiler::TYPE_UNKNOWN_IDX);

                            // Creates value id that has an unknown type, no constant value, and an
                            // unresolved expression.
                            let val_id = ValueId::new(self.compiler.values.len() as u32);
                            let val_info = ValueInfo::new(type_id, expr_id, None);

                            self.compiler.values.push(val_info);

                            ResolvedExpr::new(
                                type_id,
                                expr_hir,
                                val_id,
                                spanned_expr.span,
                                Vec::new(),
                            )
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
                if let Ok(num) = self.interner.search(*id as usize).parse::<i64>() {
                    // Getting what it's spot would be when it's expression and value parts are
                    // pushed
                    let expr_id = ExprId::new(self.compiler.exprs.len() as u32);
                    let val_id = ValueId::new(self.compiler.values.len() as u32);

                    // Creating it's default type to the literal value of integer, as well as it's
                    // expression of just being a singular value type
                    let expr_hir = ExprHir::Val(val_id);
                    let type_id = TypeId::new(BuiltinTypeKind::I64 as u32);

                    let resolved_expr =
                        ResolvedExpr::new(type_id, expr_hir, val_id, spanned_expr.span, Vec::new());

                    // Creating the actual value portion of the expression
                    let val = Value::I64(num);
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
                    let expr =
                        ResolvedExpr::new(type_id, expr_hir, val_id, spanned_expr.span, Vec::new());

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

                let is_unknown = lhs_expr.type_id.id == script_compiler::TYPE_UNKNOWN_IDX
                    || rhs_expr.type_id.id == script_compiler::TYPE_UNKNOWN_IDX;

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
                        // If cannot perform operation and neither are unknown then there is actual
                        // corruption, and not one part just being unresolved
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

                // Hm
                self.compiler.exprs[lhs_id.id as usize].user = Some(expr_id);
                self.compiler.exprs[rhs_id.id as usize].user = Some(expr_id);

                // Expression points to the value so the expr_id is returned alone.
                let resolved_expr = ResolvedExpr::new(
                    type_id,
                    expr_hir,
                    val_id,
                    spanned_expr.span,
                    vec![lhs_id, rhs_id],
                );

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
            Expr::Char(c) => {
                let expr_id = ExprId::new(self.compiler.exprs.len() as u32);
                let val_id = ValueId::new(self.compiler.values.len() as u32);
                let type_id = TypeId::new(BuiltinTypeKind::Char as u32);

                let val = Value::Char(*c);
                let val_info = ValueInfo::new(type_id, expr_id, Some(val));
                self.compiler.values.push(val_info);

                let expr_hir = ExprHir::Val(val_id);
                let resolved_expr =
                    ResolvedExpr::new(type_id, expr_hir, val_id, spanned_expr.span, Vec::new());
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
                let resolved_expr =
                    ResolvedExpr::new(type_id, expr_hir, val_id, spanned_expr.span, Vec::new());
                self.compiler.exprs.push(resolved_expr);

                Ok(expr_id)
            }
            Expr::Call(caller, spanned_exprs) => {
                todo!();
            }
            // Maybe having "::" exist could help..
            Expr::MemberAccess(abs_member_access) => {
                match self.resolve_member(sym_parent, &abs_member_access.base, scope_type)? {
                    PossibleMember::Module(mod_id) => {
                        let extern_mod = &mut self.compiler.mods[mod_id.id];
                        if let Some(extern_sym_id) =
                            extern_mod.get_sym_id(abs_member_access.field, scope_type)
                        {
                            let symbol = &self.compiler.symbols[&extern_sym_id];

                            // symbol kind aware reporting.
                            // In need of cross-module reporting of where the not exported symbol
                            // is
                            if symbol.is_priv && symbol.owner != self.current_mod {
                                let name = self.interner.search(symbol.name_id.id as usize);
                                let msg = format!("The symbol `{name}` is private");

                                self.reporter.report_spanned(
                                    &msg,
                                    None,
                                    &[spanned_expr.span],
                                    &self.compiler.mods[self.current_mod.id],
                                );
                            }

                            match symbol.kind {
                                SymbolKind::Type(type_id) => todo!(),
                                SymbolKind::Val(val_id) => todo!(),
                                SymbolKind::Unknown => {
                                    let expr_id = ExprId::new(self.compiler.exprs.len() as u32);
                                    let expr_hir = ExprHir::Var(extern_sym_id);
                                    let pending_expr = PendingExpr::new(expr_id, sym_parent);

                                    self.ty_ctx.store_pending_expr(extern_sym_id, pending_expr);
                                    // Will possibly call for others to be resolved here, or do it from the
                                    // var resolution method itself

                                    let type_id = TypeId::new(script_compiler::TYPE_UNKNOWN_IDX);

                                    // Creates value id that has an unknown type, no constant value, and an
                                    // unresolved expression.
                                    let val_id = ValueId::new(self.compiler.values.len() as u32);
                                    let val_info = ValueInfo::new(type_id, expr_id, None);

                                    ResolvedExpr::new(
                                        type_id,
                                        expr_hir,
                                        val_id,
                                        spanned_expr.span,
                                        Vec::new(),
                                    );

                                    self.compiler.values.push(val_info);

                                    let expr = ResolvedExpr::new(
                                        type_id,
                                        expr_hir,
                                        val_id,
                                        spanned_expr.span,
                                        Vec::new(),
                                    );

                                    self.compiler.exprs.push(expr);

                                    Ok(expr_id)
                                }
                            }
                        } else {
                            // TODO: Should also show what scopes were searched or just in some
                            // form at all state why a symbol that exists wasn't seen
                            // Find similar symbols
                            let msg = format!(
                                "Could not find the symbol `{}` inside module `{}` as a value or type",
                                self.interner.search(abs_member_access.field.id as usize),
                                self.interner.search(extern_mod.name_id.id as usize)
                            );

                            return Err(SemanticError::General(msg, vec![spanned_expr.span]));
                        }
                    }
                    // Maybe this shouldn't be allowed here since parsing types is different from
                    // parinsg expressions within this resolver, meaning this should be an error
                    //
                    // But also, this is literally impossible since only `nest` sections can
                    // actually access types, but expressions use types to check for if a value is
                    // searchable so is it still needed?
                    PossibleMember::Type(type_id) => {
                        todo!("Type id");
                    }
                    PossibleMember::Var(val_id) => {
                        ValueResult::Resolved(val_id);
                        unimplemented!(
                            "Nothing matches this case yet but \"self.\" mentions would"
                        );
                    }
                    PossibleMember::Nothing => todo!("Unresolved"),
                }
            }
            Expr::Unary(unary) => {
                let operand_id = self.register_expr(sym_parent, &unary.spanned_expr, scope_type)?;
                let operand_expr = &self.compiler.exprs[operand_id.id as usize];

                let is_unknown = operand_expr.type_id.id == script_compiler::TYPE_UNKNOWN_IDX;

                let operand_val_opt = &self.compiler.values[operand_expr.val_id.id as usize];

                let const_val_opt = if let Some(const_val) = &operand_val_opt.const_val {
                    if !evaluator::is_compatible_unary(unary.op, const_val) && !is_unknown {
                        return Err(MathError::UnaryOpMismatch(
                            const_val.kind().to_fmt(),
                            unary.op.to_fmt(),
                            vec![spanned_expr.span],
                        ))?;
                    } else {
                        Some(evaluator::apply_unary_op(unary.op, const_val)?)
                    }
                } else {
                    None
                };

                let val_id = ValueId::new(self.compiler.values.len() as u32);
                let unary_expr_id = ExprId::new(self.compiler.exprs.len() as u32);

                let expr_hir = ExprHir::Unary {
                    op: unary.op,
                    operand: operand_id,
                };

                let type_id = if const_val_opt.is_some() {
                    operand_expr.type_id
                } else {
                    TypeId::new(script_compiler::TYPE_UNKNOWN_IDX)
                };

                let resolved_expr = ResolvedExpr::new(
                    type_id,
                    expr_hir,
                    val_id,
                    spanned_expr.span,
                    vec![operand_id],
                );

                self.compiler.exprs.push(resolved_expr);
                self.compiler.exprs[operand_id.id as usize].user = Some(unary_expr_id);

                let val_info = ValueInfo::new(type_id, unary_expr_id, const_val_opt);
                self.compiler.values.push(val_info);

                Ok(unary_expr_id)
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
                if name_id.id == intern::INTERNED_SELF as u32 {
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

        dbg!(member);
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
                    "More than one field has the identifier \"{dup_name}\" within struct `{struct_name}`"
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
                    "More than one variant has the identifier \"{dup_name}\" within enum `{enum_name}`"
                );

                self.reporter.report_spanned(
                    &msg,
                    None,
                    &[orig_span, variant_span],
                    &self.compiler.mods[self.current_mod.id],
                );
            }

            seen.push((i, variant.name_id));

            let variant_repre = if let Some(spanned_ty_expr) = &variant.ty_expr {
                let type_id = self.resolve_type_expr(&spanned_ty_expr, ScopeType::Nest, ast_id)?;
                VariantRepre::new(variant.name_id, Some(type_id), AstId::new(i as u32))
            } else {
                VariantRepre::new(variant.name_id, None, AstId::new(i as u32))
            };

            variants.push(variant_repre);
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
        let mut params: Vec<Param> = Vec::new();
        let mut seen: Vec<(usize, InternedId)> = Vec::new();

        for (i, spanned_ty_expr) in abs_alias.params.iter().enumerate() {
            match &spanned_ty_expr.ty_expr {
                TypeExpr::Var(interned_id) | TypeExpr::Escaped(interned_id) => {
                    if let Some(original) = seen.iter().find(|other| *interned_id == other.1) {
                        let alias_name = self.interner.search(abs_alias.name_id.id as usize);
                        let dup_name = self.interner.search(interned_id.id as usize);

                        let orig_span = abs_alias.params[original.0].span;
                        let field_span = abs_alias.params[i].span;

                        let msg = format!(
                            "More than one variable has the identifier \"{dup_name}\" within alias `{alias_name}`"
                        );

                        self.reporter.report_spanned(
                            &msg,
                            None,
                            &[orig_span, field_span],
                            &self.compiler.mods[self.current_mod.id],
                        );
                    }

                    seen.push((i, *interned_id));

                    let param = Param::new(
                        *interned_id,
                        // spanned_ty_expr.span,
                        AstId::new(i as u32),
                        TypeId::new(script_compiler::TYPE_UNKNOWN_IDX),
                    );

                    params.push(param);
                }
                // The parser checks for these so this will probably become just interned ids
                _ => unreachable!("Should maybe change this to ids if possible"),
            }
        }

        let module = &mut self.compiler.mods[self.current_mod.id];
        let scope_id = module.extract_scope_id(ScopeType::Neutral);
        let table = &module.get_scope_mut(scope_id).table;

        let sym_id = table.sym_ids[&ast_id];

        let alias_def = self.compiler.get_alias_mut(sym_id);
        alias_def.params = params;

        Ok(())
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

                            for arg in &generic.args {
                                elements.push(self.resolve_type_expr(arg, scope_type, ast_id)?);
                            }

                            let type_id = TypeId::new(self.compiler.types.len() as u32);
                            let tuple = Type::BuiltinType(BuiltinType::Tuple(elements));

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
                                "Found identifier \"{err_name}\" before generic parameters, but only `List`, `Set`, `Map`, and `Tuple` are valid data structures"
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
                            "Found identifier \"{err_name}\" before generic parameters, but only `List`, `Set`, `Map`, and `Tuple` are valid data structures"
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
            // TypeExpr::Tuple(unres_tuple) => {
            //     let mut elements: Vec<TypeId> = Vec::new();
            //
            //     for element in unres_tuple {
            //         let type_id = self.resolve_type_expr(element, scope_type, ast_id)?;
            //         elements.push(type_id);
            //     }
            //
            //     let tuple_id = TypeId::new(self.compiler.types.len() as u32);
            //     let tuple = Type::Tuple(Tuple::new(elements));
            //     panic!("Intrinsic stuff");
            //
            //     let ty_info = TypeInfo::new(tuple, Some(self.current_mod));
            //     self.compiler.types.push(ty_info);
            //
            //     Ok(tuple_id)
            // }
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
                    let symbol = &self.compiler.symbols[&sym_id];

                    //WARN: Only scoping issue left is alias and const collision and maybe some
                    //others
                    let type_id = match symbol.kind {
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

                    if symbol.is_priv && symbol.owner != self.current_mod {
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
