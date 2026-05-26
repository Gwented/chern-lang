//TODO: SHOULD IMPORT CHECKING HAPPEN HERE??
pub mod type_context;

use chrn_utils::id_types::{
    AstId, ExprId, InternedId, ModuleId, ScopeId, SymbolId, TypeId, ValueId,
};
use chrn_utils::intern::Intern;
use chrn_utils::types::builtins::{BuiltinType, BuiltinTypeKind};
use chrn_utils::values::{Value, ValueInfo};
use common::chrn_settings::ChrnSettings;
use common::fmter::{Formattable, Formatted};
use common::{reporter::diagnostic::Diagnostic, span::Span};

use crate::parser::ast::{AbstractVar, BinaryOp, Expr, SpannedExpr, SpannedPathSegment};
use crate::script_compiler::{self, ScriptCompiler};
use crate::semantic::error::{MathError, SemanticError};
use crate::semantic::representation::{
    ExprHir, Param, PossibleMember, ResolvedExpr, Symbol, SymbolKind,
};
use crate::semantic::scopes::{AssociatedScopeKind, LookupPattern, ScopeType};
use crate::semantic::type_resolver::type_context::{
    ParentInfo, ParentState, PendingExpr, PendingSymbol, TypeContext,
};
use crate::semantic::{evaluator, scopes};

use crate::{
    parser::ast::{
        AbstractAlias, AbstractEnum, AbstractStruct, AbstractTypeDef, AstInfo, Item, PathSegment,
        SpannedTypeExpr, TypeExpr,
    },
    semantic::{
        representation::{FieldRepre, Type, TypeInfo, VariantRepre},
        semantic_reporter::SemanticReporter,
    },
};

use super::constraints::ArgConstraint;

/// Resolves types and builds the rest of any structs, enums, or expressions that can be const
/// evaluated. Does so by mutating the compiler given, and maintaining context to retain it's last
/// state.
pub struct TypeResolver<'a> {
    ast_info: &'a AstInfo,
    interner: &'a Intern,
    //WARN: Horrors
    compiler: &'a mut ScriptCompiler,
    current_mod: ModuleId,
    ty_ctx: &'a mut TypeContext,
    reporter: SemanticReporter<'a>,
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

        // This is a system of tracking to where it dynamically through knowing the result
        // of the expression incremental resolution, and accounting for stale caching in regards
        // to not setting it's parent to resolved multiple times.
        // (which may be a little bit of over-complication but it works)
        //
        // The architecture is, say we have, let a = b, let b = c, let c = d + 5, let d = 4.
        // a and d have no resolved type or const value. c has a type because of inference regarding
        // literal 5, d has a const value AND type. In resolution, a, b and d remain unchanged, but c
        // being set to d is noticed, and d has a const value and const type, which results in the
        // incremental update marking c as both const booleans. Because expressions that were pending
        // inside of pending symbols know how far they went, they are checked. So, c would have b checked,
        // b would have it's parent's info that it's fully const and resolved, we then check if b is
        // dependended on, a depends on b, so now b has it's expressions attempted to be resolved, which
        // leads to a realizing it has 2 const values, which makes a resolved.

        // These variables are the sole determining factors as to how long the expression context
        // is looped, given any new information.

        let mut last_resolved_count: u32 = 0;
        let mut current_resolved_count: u32 = 0;
        while self.ty_ctx.needs_check {
            // let sym = &self.compiler.symbols[44];
            // let name = self.interner.search(sym.name_id.id as usize);
            // dbg!(name);
            //actually resolved already.
            self.ty_ctx.needs_check = false;
            // Giving ownership to a variable since the traversal chosen needs mutation while
            // traversing
            let mut pending_syms: Vec<(SymbolId, PendingSymbol)> = Vec::new();
            pending_syms.extend(self.ty_ctx.sym_queue.drain());

            // TODO: Should actually check if any pending symbol has only stale expressions
            let mut removable_syms: Vec<SymbolId> = Vec::new();

            for (sym_id, pending_sym) in &mut pending_syms {
                // If there is no resolved type then there cannot exist a const value
                if !pending_sym.has_resolved_ty {
                    continue;
                }

                match self.try_resolve_pending(*sym_id, pending_sym) {
                    //TODO: Can something be done with these?
                    //Succeeding just means no errors ocurred, not that new information was found,
                    //so maybe we can check here for removable symbols, say, if queue is empty?
                    //Is removing even worth it?
                    Ok(can_remove) => {
                        // Not sure about this yet
                        if can_remove {
                            removable_syms.push(*sym_id);
                        }
                    }
                    // Not sure if anything more can be done here since the diagnostic is already
                    // made
                    Err(_) => (),
                };
            }

            // Giving self back the data
            self.ty_ctx.sym_queue.extend(pending_syms);

            // Not changing this right now.
            //
            // The pending symbol the expression was found in
            // The index of the expression to set as stale.
            // The actual parent's info to fill in.
            let mut resolved_parents: Vec<(SymbolId, usize, ParentInfo)> = Vec::new();

            // Also needs to check if there exists a pending symbol which has ONLY stale
            // expressions inside, meaning it should be removed.

            // Finding all parents that recieved new information by checking if a pending expr has
            // the `Resolved` variant.
            for (pending_sym_id, pending_sym) in &self.ty_ctx.sym_queue {
                for (i, pending_expr) in pending_sym.pending_exprs.iter().enumerate() {
                    if let ParentState::Resolved(has_resolved_ty, has_const_val) =
                        pending_expr.parent_state
                    {
                        let possible_pending = pending_expr.parent_sym;
                        if self.ty_ctx.sym_queue.contains_key(&possible_pending) {
                            // Maybe current resolved can be removed now?
                            current_resolved_count += 1;
                            let parent_info =
                                ParentInfo::new(possible_pending, has_resolved_ty, has_const_val);

                            resolved_parents.push((*pending_sym_id, i, parent_info));
                        }
                    }
                }
            }

            // Integral loop that sets whatever resolution information regarding the parent to
            // true, so that it can actually be accounted for as a resolved pending symbol. Pending
            // symbol's expressions are never attempted for resolution unless they are marked to at
            // least have a resolved type.
            for (pending_sym_id, pending_expr_idx, parent_info) in resolved_parents {
                // Setting expr to stale
                let pending_sym = self
                    .ty_ctx
                    .sym_queue
                    .get_mut(&pending_sym_id)
                    .expect("Previous loop failed");

                pending_sym.pending_exprs[pending_expr_idx].parent_state =
                    ParentState::Notified(parent_info.has_resolved_ty, parent_info.has_const_val);

                // Allowing for parent to be searched in resolution
                let parent = self
                    .ty_ctx
                    .sym_queue
                    .get_mut(&parent_info.pending_sym_id)
                    .expect("Previous loop failed");

                parent.has_resolved_ty = parent_info.has_resolved_ty;
                parent.has_const_val = parent_info.has_const_val;
            }

            //WARN: By logic this seems fine since if the queue is empty then that means everything
            //found in pending_expr has a fully resolved parent.
            for sym_id in removable_syms {
                self.ty_ctx.sym_queue.remove(&sym_id);
            }

            if current_resolved_count == last_resolved_count {
                break;
            } else {
                last_resolved_count = current_resolved_count;
                self.ty_ctx.needs_check = true;
            }
        }

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

        // if self.current_mod == self.compiler.mods[self.compiler.mods.len() - 2].mod_id {
        //     dbg!(&self.ty_ctx);
        //     for symbol in &self.compiler.symbols {
        //         if self.interner.search(symbol.name_id.id as usize) == "a" {
        //             let name = self.interner.search(symbol.name_id.id as usize);
        //             dbg!(name);
        //             match symbol.kind {
        //                 SymbolKind::Val(value_id) => {
        //                     let val = &self.compiler.values[value_id.id as usize];
        //                     let expr = &self.compiler.exprs[val.expr_id.id as usize];
        //                     // dbg!(expr.val_id, expr);
        //                     dbg!(expr, val);
        //                 }
        //                 SymbolKind::Type(type_id) => {
        //                     let ty_info = &self.compiler.types[type_id.id as usize];
        //                     match &ty_info.ty {
        //                         Type::BuiltinType(builtin_type) => {
        //                             dbg!(builtin_type);
        //                         }
        //                         Type::Struct(struct_def) => todo!(),
        //                         Type::Enum(enum_def) => todo!(),
        //                         Type::Func(func_def) => todo!(),
        //                         Type::Alias(alias_def) => todo!(),
        //                         Type::TypeDef(type_def) => {
        //                             let ty = &self.compiler.types[type_def.type_id.id as usize];
        //                             dbg!(ty);
        //                         }
        //                         Type::Unknown => todo!(),
        //                         _ => todo!(),
        //                     }
        //                 }
        //                 _ => todo!(),
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
        pending_sym: &mut PendingSymbol,
        // Eyes
    ) -> Result<bool, ()> {
        // Tells the caller if the given pending symbol is fully resolved to where it can be
        // removed as a pending symbol
        let mut can_remove = false;
        let mut queue: Vec<ExprId> = Vec::new();

        //Suspicious
        for pending_expr in &pending_sym.pending_exprs {
            if let ParentState::Notified(true, true) = pending_expr.parent_state {
                continue;
            }

            // Error being treated the same as a resolved expression since it can't be mutated
            // further
            if pending_expr.parent_state == ParentState::Error {
                continue;
            }

            queue.push(pending_expr.pending_id);
        }

        // In the example:
        //
        // ```
        // let y = x + 2
        // let x = 2
        // ```
        //
        // root_expr = x
        // So, it needs to go x -> x + 2 -> None
        //

        // Needs to resolve first root
        for (i, root_id) in queue.iter().copied().enumerate() {
            // Still need to repair root expr
            let root_expr = &mut self.compiler.exprs[root_id.id as usize];
            match self.compiler.symbols[resolved_sym_id.id as usize].kind {
                SymbolKind::Val(val_id) => {
                    // Brain starting working now it works

                    if pending_sym.has_resolved_ty {
                        let val_info = &self.compiler.values[val_id.id as usize];
                        let other_type_id = val_info.type_id;

                        self.compiler.types[root_expr.type_id.id as usize].ty =
                            Type::Deferred(other_type_id);

                        let inner_val = &mut self.compiler.values[root_expr.val_id.id as usize];
                        self.compiler.types[inner_val.type_id.id as usize].ty =
                            Type::Deferred(other_type_id);
                    }

                    if pending_sym.has_const_val {
                        let val_info = &self.compiler.values[val_id.id as usize];
                        let const_val_opt = val_info.const_val.clone();

                        let inner_val = &mut self.compiler.values[root_expr.val_id.id as usize];
                        inner_val.const_val = const_val_opt;
                    }

                    // The question is, why is a root already assigned to a type anyways?
                    //
                    // Are roots re-assigned everytime?
                    //
                    // Mutating location so that it points to the canonical type id
                    // let pending = &self.compiler.symbols[resolved_sym_id.id as usize];
                    // let name = self.interner.search(pending.name_id.id as usize);
                    // dbg!(name);
                    // dbg!(&const_val_opt);

                    // Not sure if clone can be avoided here since mutating val_id so that it
                    // points to the current expr would mutate the symbol it was gotten from,
                    // which would mangle the resolved symbol itself even though we just want the
                    // const value if present.
                }
                // NOTE: Since expressions are initialized as `ReservedTypeSlot`, if there is say,
                // a cyclic dependency error, the error will exist and emit later, but this
                // technically still exists and needs to be ignored. Not currently aware of any
                // direct issues with this. Maybe an Error tag on a pending expression could help?
                SymbolKind::ReservedTypeSlot(_) => continue,
                SymbolKind::Type(_) | SymbolKind::Module(_) => {
                    unreachable!("Not possible")
                }
            }

            if let Some(user) = root_expr.user {
                // TEST: Not sure if this accurately tracks yet
                match self.traverse_expr(user) {
                    Ok((has_resolved_ty, has_const_val)) => {
                        let pending_expr = &mut pending_sym.pending_exprs[i];

                        let has_new_info = match pending_expr.parent_state {
                            ParentState::Unresolved => true,
                            // Only value matters here since being resolved previous means there at
                            // least is a resolved type present.
                            ParentState::Resolved(_, old_val)
                            | ParentState::Notified(_, old_val) => has_const_val && !old_val,
                            ParentState::Error => false,
                        };

                        if has_new_info {
                            if has_resolved_ty {
                                pending_expr.parent_state =
                                    ParentState::Resolved(has_resolved_ty, has_const_val);
                            }
                        }
                    }
                    // WARN: This case is not hit yet
                    // Reports the error and continues
                    Err(sem_err) => {
                        // Extracting module of origin from the pending expression by using the symbol
                        // attached to the expression upon it's creation
                        let parent_sym_id = pending_sym.pending_exprs[0].parent_sym;
                        let mod_id = self.compiler.get_owner(parent_sym_id);

                        //WARN: Suspicious
                        let module = &self.compiler.mods[mod_id.id];
                        self.reporter.report_semantic(
                            sem_err,
                            &module
                                .src_metadata
                                .as_ref()
                                .expect("core should not be resolved"),
                        )
                    }
                };
            } else {
                // If the root has no users, then that means its, let y = x where there is nothing else
                // that needs resolution since the root is always a single variable.

                // Also sending signal that the parent of this is resolved since it's a root.

                // But WOULD this always mean const value? Right now it does.
                let pending_expr = &mut pending_sym.pending_exprs[i];
                let has_resolved_ty = pending_sym.has_resolved_ty;
                let has_const_val = pending_sym.has_const_val;

                let has_new_info = match pending_expr.parent_state {
                    ParentState::Unresolved => true,
                    // Only value matters here since being resolved previous means there at
                    // least is a resolved type present.
                    ParentState::Notified(_, old_val) | ParentState::Resolved(_, old_val) => {
                        has_const_val && !old_val
                    }
                    ParentState::Error => false,
                };

                if has_new_info {
                    pending_expr.parent_state =
                        ParentState::Resolved(has_resolved_ty, has_const_val);
                }

                break;
            }
        }

        if queue.is_empty() {
            can_remove = true;
        }

        Ok(can_remove)
    }

    /// Returns an `Ok(true)` upon fully resolving a tree of expressions.
    /// Returns an `Ok(false)` if the resolution failed because a value was unknown.
    /// Returns `Err` upon real user errors.
    /// Method to recursively mutate tree of unresolved expression
    /// This works as root -> user -> user -> ... -> None
    // This needs to go from x -> x + 2 -> y recursively however long needed
    fn traverse_expr(&mut self, current_expr_id: ExprId) -> Result<(bool, bool), SemanticError> {
        let mut has_resolved_ty = false;
        let mut has_const_val = false;

        match &self.compiler.exprs[current_expr_id.id as usize].expr_hir {
            ExprHir::Val(val_id) => {
                // This is unreachable
                let val_info = &self.compiler.values[val_id.id as usize];

                let new_type_id = val_info.type_id;
                let const_val_opt = val_info.const_val.clone();

                has_resolved_ty = self.compiler.check_unknown(new_type_id);
                has_const_val = const_val_opt.is_some();

                let expr = &mut self.compiler.exprs[current_expr_id.id as usize];
                // Mutating the type address so that it is now deferred to it's real type
                self.compiler.types[expr.type_id.id as usize].ty = Type::Deferred(new_type_id);

                let inner_val = &mut self.compiler.values[expr.val_id.id as usize];
                self.compiler.types[inner_val.type_id.id as usize].ty = Type::Deferred(new_type_id);

                inner_val.type_id = new_type_id;
                inner_val.const_val = const_val_opt;

                todo!("Make sure this is ok")
            }
            ExprHir::Unary { op, operand } => {
                // Getting the operand that could be resolved (Might be guarnteed but um..e)
                let operand_expr = &self.compiler.exprs[operand.id as usize];

                let is_unknown = self.compiler.check_unknown(operand_expr.type_id);
                // This means that we reached an expression inside of a resolved expression that is
                // not fully resolved yet, which is fine.
                if is_unknown {
                    return Ok((false, false));
                }

                has_resolved_ty = true;

                let operand_val_info = &self.compiler.values[operand_expr.val_id.id as usize];

                // Basic validation of expression to see if it's const or runtime
                let const_val_opt = if let Some(const_val) = &operand_val_info.const_val {
                    if !evaluator::is_compatible_unary(*op, const_val) {
                        return Err(MathError::UnaryOpMismatch(
                            const_val.kind().to_fmt(),
                            op.to_fmt(),
                            vec![operand_expr.span],
                        ))?;
                    } else {
                        has_const_val = true;
                        Some(evaluator::apply_unary_op(*op, const_val)?)
                    }
                } else {
                    None
                };

                let new_type_id = operand_expr.type_id;

                // Should this be deferred or new?
                //
                // Mutating expression's type so that the symbol using this expr reflects the new
                // information
                let expr = &mut self.compiler.exprs[current_expr_id.id as usize];
                self.compiler.types[expr.type_id.id as usize].ty = Type::Deferred(new_type_id);

                // Mutating inner value so that the symbol using this value reflects the new
                // information
                let inner_val = &mut self.compiler.values[expr.val_id.id as usize];
                self.compiler.types[inner_val.type_id.id as usize].ty = Type::Deferred(new_type_id);
                inner_val.const_val = const_val_opt;
            }
            ExprHir::BinaryExpr { lhs, op, rhs } => {
                //TODO: Considering a span vector so that they dont need to be duplicated or
                //computed by going inside items anymore.

                let lhs_expr = &self.compiler.exprs[lhs.id as usize];
                let rhs_expr = &self.compiler.exprs[rhs.id as usize];

                let is_unknown = if self.compiler.check_unknown(lhs_expr.type_id)
                    || self.compiler.check_unknown(rhs_expr.type_id)
                {
                    true
                } else {
                    false
                };

                // This means that we reached an expression inside of a resolved expression that is
                // not fully resolved yet
                if is_unknown {
                    return Ok((false, false));
                }

                has_resolved_ty = true;

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
                        if !evaluator::is_compatible_binary(lhs_const, *op, rhs_const) {
                            let full_span = lhs_expr.span.merge(rhs_expr.span);

                            return Err(MathError::BinaryOpMismatch(
                                lhs_const.kind().to_fmt(),
                                rhs_const.kind().to_fmt(),
                                op.to_fmt(),
                                vec![full_span],
                            ))?;
                        } else {
                            has_const_val = true;
                            Some(evaluator::apply_binary_op(lhs_const, *op, rhs_const)?)
                        }
                    }
                    _ => None,
                };

                //WARN: Suspicious
                // Should this account for I$@)($$*#%)$?
                let new_type_id =
                    self.infer_type_from_binary_op(*lhs, *rhs, false, *op, false, &const_val_opt);

                //NOTE: Only the type of the expression is altered here, the rest is the inner
                //value
                let expr = &mut self.compiler.exprs[current_expr_id.id as usize];
                // Assigning directly since this is a newly created type id..
                expr.type_id = new_type_id;
                // dbg!(expr.type_id, new_type_id);
                // self.compiler.types[expr.type_id.id as usize].ty = panic!();

                let inner_val = &mut self.compiler.values[expr.val_id.id as usize];
                inner_val.type_id = new_type_id;
                // self.compiler.types[inner_val.type_id.id as usize].ty = Type::Deferred(new_type_id);
                inner_val.const_val = const_val_opt;
            }
            ExprHir::Call(expr_id, expr_ids) => todo!(),
            ExprHir::Var(sym_id) => {
                todo!("What is a varrrble")
            }
            ExprHir::Default(sym_id, expr_id) => {
                todo!("Default not finished")
            }
        }

        // Traversing up tree
        let expr = &self.compiler.exprs[current_expr_id.id as usize];
        //WARN: Seems to be working
        if let Some(user) = expr.user {
            return self.traverse_expr(user);
        }

        // has_resolved_ty && has_const_val
        Ok((has_resolved_ty, has_const_val))
    }

    fn resolve_var(&mut self, abs_var: &AbstractVar, ast_id: AstId) -> Result<(), ()> {
        let scope_id = self
            .compiler
            .extract_scope_id(ScopeType::Neutral, self.current_mod);
        let table = &mut self.compiler.get_scope_mut(scope_id).scope.table;

        let sym_id = table.ast_to_sym[&ast_id];
        let associated_scope = AssociatedScopeKind::Module(self.current_mod);

        //NOTE: Pipeline where expressions are always returned, just that some may have
        //unresolved parts, which are put into the queue, not the variable itself.
        let expr_id = match self.register_expr(
            sym_id,
            &abs_var.spanned_expr,
            None,
            associated_scope,
            ScopeType::Neutral,
            &mut vec![sym_id],
        ) {
            Ok(expr_id) => expr_id,
            Err(sem_err) => {
                let module = &self.compiler.mods[self.current_mod.id];
                self.reporter.report_semantic(
                    sem_err,
                    &module
                        .src_metadata
                        .as_ref()
                        .expect("core should not be resolved"),
                );
                return Err(());
            }
        };

        let expr = &self.compiler.exprs[expr_id.id as usize];
        let val = &self.compiler.values[expr.val_id.id as usize];

        //                      NOT unknown
        let has_resolved_ty = !self.compiler.check_unknown(expr.type_id);
        let has_const_val = val.const_val.is_some();

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

        //WARN: You were warned
        let symbol = self
            .compiler
            .symbols
            .get_mut(sym_id.id as usize)
            .expect("Exists");
        symbol.kind = SymbolKind::Val(val_id);

        let name = self
            .interner
            .search(self.compiler.symbols[sym_id.id as usize].name_id.id as usize);
        // If the symbol that was just examined is a pending symbol AND it was actually resolved,
        // then it'll be marked as resolved
        if let Some(pending_sym) = self.ty_ctx.sym_queue.get_mut(&sym_id) {
            // Three flags for resolver use
            pending_sym.has_resolved_ty = has_resolved_ty;
            pending_sym.has_const_val = has_const_val;

            self.ty_ctx.needs_check = true;
        }

        Ok(())
    }

    fn resolve_typedef(&mut self, abs_typedef: &AbstractTypeDef, ast_id: AstId) -> Result<(), ()> {
        let type_id = match self.resolve_type_expr(
            AssociatedScopeKind::Module(self.current_mod),
            &abs_typedef.spanned_ty_expr,
            ScopeType::Var,
            LookupPattern::NoRestrictions,
        ) {
            Ok(tid) => tid,
            Err(sem_err) => {
                let module = &self.compiler.mods[self.current_mod.id];
                self.reporter.report_semantic(
                    sem_err,
                    &module
                        .src_metadata
                        .as_ref()
                        .expect("core should not be resolved"),
                );

                return Err(());
            }
        };

        let scope_id = self
            .compiler
            .extract_scope_id(ScopeType::Var, self.current_mod);
        let table = &self.compiler.get_scope(scope_id).scope.table;
        let sym_id = table.ast_to_sym[&ast_id];
        let associated_scope = AssociatedScopeKind::Module(self.current_mod);

        let mut conds: Vec<ExprId> = Vec::new();
        for spanned_expr in &abs_typedef.conds {
            //FIX: Scope type is a little wrong here since it's a condition
            let cond_opt = match self.register_expr(
                sym_id,
                spanned_expr,
                None,
                associated_scope,
                ScopeType::Neutral,
                &mut vec![sym_id],
            ) {
                // For allowing for more diagnostics instead of just leaving the rest of the struct
                // unfinished upon singular errors
                Ok(c) => Some(c),
                Err(sem_err) => {
                    let module = &self.compiler.mods[self.current_mod.id];
                    self.reporter.report_semantic(
                        sem_err,
                        &module
                            .src_metadata
                            .as_ref()
                            .expect("core should not be resolved"),
                    );

                    None
                }
            };

            if let Some(cond) = cond_opt {
                conds.push(cond);
            }
        }

        let type_def = self.compiler.get_typedef_mut(sym_id);
        // Assinging from `Unknown` to it's actual type
        //TODO: Constraints should check if this is unknown
        type_def.type_id = type_id;
        type_def.conds = conds;
        type_def.args = abs_typedef.args.iter().map(|sp_arg| sp_arg.arg).collect();

        Ok(())
    }

    fn resolve_struct(&mut self, abs_struct: &AbstractStruct, ast_id: AstId) -> Result<(), ()> {
        // Not sure of if this should stay a Field type or just be a TypeDef since their intent
        // somewhat conflicts. For now, typedef is just consumed differently depending on if it's a
        // field declared in var-> or not since var-> fields may be made possible to reference, but
        // fields in structures can't. Will possibly just be unified in the future.
        let mut fields: Vec<FieldRepre> = Vec::new();
        let mut seen: Vec<(usize, InternedId)> = Vec::new();

        let scope_id = self
            .compiler
            .extract_scope_id(ScopeType::Nest, self.current_mod);
        let table = &self.compiler.get_scope(scope_id).scope.table;

        //TODO: global condition and argument setting.
        //field arg and cond settings.
        //same for enums.

        let sym_id = table.ast_to_sym[&ast_id];
        let associated_scope = AssociatedScopeKind::Module(self.current_mod);

        // Checking if there are duplicate name ids within the same struct along with resolution
        for (i, field_typedef) in abs_struct.fields.iter().enumerate() {
            let type_id = match self.resolve_type_expr(
                AssociatedScopeKind::Module(self.current_mod),
                &field_typedef.spanned_ty_expr,
                ScopeType::Nest,
                LookupPattern::NoRestrictions,
            ) {
                Ok(tid) => tid,
                Err(sem_err) => {
                    let module = &self.compiler.mods[self.current_mod.id];
                    self.reporter.report_semantic(
                        sem_err,
                        &module
                            .src_metadata
                            .as_ref()
                            .expect("core should not be resolved"),
                    );

                    return Err(());
                }
            };

            if let Some(original) = seen.iter().find(|other| field_typedef.name_id == other.1) {
                let struct_name = self.interner.search(abs_struct.name_id.id as usize);
                let dup_name = self.interner.search(field_typedef.name_id.id as usize);

                let orig_span = abs_struct.fields[original.0].name_span;
                let field_span = abs_struct.fields[i].name_span;

                let msg = format!(
                    "More than one field has the identifier \"{dup_name}\" within struct `{struct_name}`"
                );

                let module = &self.compiler.mods[self.current_mod.id];
                self.reporter.report_spanned(
                    &msg,
                    None,
                    &[orig_span, field_span],
                    &module
                        .src_metadata
                        .as_ref()
                        .expect("core should not be resolved"),
                );
            }

            seen.push((i, field_typedef.name_id));

            let field = FieldRepre::new(field_typedef.name_id, type_id, ast_id);

            fields.push(field);
        }

        for (i, field) in fields.iter_mut().enumerate() {
            let abs_field = &abs_struct.fields[i];
            let mut conds: Vec<ExprId> = Vec::new();

            for cond in &abs_field.conds {
                let cond_opt = match self.register_expr(
                    sym_id,
                    &cond,
                    None,
                    associated_scope,
                    ScopeType::Nest,
                    &mut vec![sym_id],
                ) {
                    Ok(c) => Some(c),
                    Err(sem_err) => {
                        let module = &self.compiler.mods[self.current_mod.id];
                        self.reporter.report_semantic(
                            sem_err,
                            &module
                                .src_metadata
                                .as_ref()
                                .expect("core should not be resolved"),
                        );

                        None
                    }
                };

                if let Some(cond) = cond_opt {
                    conds.push(cond);
                }
            }

            field.conds = conds;
            field.args = abs_field.args.iter().map(|sp_arg| sp_arg.arg).collect();
        }

        let mut glob_conds: Vec<ExprId> = Vec::new();

        for cond in &abs_struct.glob_conds {
            let cond_opt = match self.register_expr(
                sym_id,
                cond,
                None,
                associated_scope,
                ScopeType::Nest,
                &mut vec![sym_id],
            ) {
                Ok(c) => Some(c),
                Err(sem_err) => {
                    let module = &self.compiler.mods[self.current_mod.id];
                    self.reporter.report_semantic(
                        sem_err,
                        &module
                            .src_metadata
                            .as_ref()
                            .expect("core should not be resolved"),
                    );

                    None
                }
            };

            if let Some(cond) = cond_opt {
                glob_conds.push(cond);
            }
        }

        let struct_def = self.compiler.get_struct_mut(sym_id);

        struct_def.fields.append(&mut fields);
        struct_def.glob_conds = glob_conds;
        //TODO: Probably will be kept
        struct_def.glob_args = abs_struct
            .glob_args
            .iter()
            .map(|sp_arg| sp_arg.arg)
            .collect();

        Ok(())
    }

    fn resolve_enum(&mut self, abs_enum: &AbstractEnum, ast_id: AstId) -> Result<(), ()> {
        let mut variants: Vec<VariantRepre> = Vec::new();

        let scope_id = self
            .compiler
            .extract_scope_id(ScopeType::Nest, self.current_mod);
        let table = &self.compiler.get_scope(scope_id).scope.table;

        let sym_id = table.ast_to_sym[&ast_id];
        let associated_scope = AssociatedScopeKind::Module(self.current_mod);

        // (ast variant idx, name_id)
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

                let module = &self.compiler.mods[self.current_mod.id];
                self.reporter.report_spanned(
                    &msg,
                    None,
                    &[orig_span, variant_span],
                    &module
                        .src_metadata
                        .as_ref()
                        .expect("core should not be resolved"),
                );
            }

            seen.push((i, variant.name_id));

            let variant_repre = if let Some(spanned_ty_expr) = &variant.ty_expr {
                let type_id = match self.resolve_type_expr(
                    AssociatedScopeKind::Module(self.current_mod),
                    &spanned_ty_expr,
                    ScopeType::Nest,
                    LookupPattern::NoRestrictions,
                ) {
                    Ok(tid) => tid,
                    Err(sem_err) => {
                        let module = &self.compiler.mods[self.current_mod.id];
                        self.reporter.report_semantic(
                            sem_err,
                            &module
                                .src_metadata
                                .as_ref()
                                .expect("core should not be resolved"),
                        );

                        TypeId::new(script_compiler::CORE_UNKNOWN)
                    }
                };
                VariantRepre::new(variant.name_id, Some(type_id), AstId::new(i as u32))
            } else {
                VariantRepre::new(variant.name_id, None, AstId::new(i as u32))
            };

            variants.push(variant_repre);
        }

        for (i, variant) in variants.iter_mut().enumerate() {
            let abs_variant = &abs_enum.variants[i];
            let mut conds: Vec<ExprId> = Vec::new();

            for cond in &abs_variant.conds {
                let cond_opt = match self.register_expr(
                    sym_id,
                    &cond,
                    None,
                    associated_scope,
                    ScopeType::Nest,
                    &mut vec![sym_id],
                ) {
                    Ok(c) => Some(c),
                    Err(sem_err) => {
                        let module = &self.compiler.mods[self.current_mod.id];
                        self.reporter.report_semantic(
                            sem_err,
                            &module
                                .src_metadata
                                .as_ref()
                                .expect("core should not be resolved"),
                        );

                        None
                    }
                };

                if let Some(cond) = cond_opt {
                    conds.push(cond);
                }
            }

            variant.conds = conds;
            variant.args = abs_variant.args.iter().map(|sp_arg| sp_arg.arg).collect();
        }

        let mut glob_conds: Vec<ExprId> = Vec::new();
        for cond in &abs_enum.glob_conds {
            let cond_opt = match self.register_expr(
                sym_id,
                cond,
                None,
                associated_scope,
                ScopeType::Nest,
                &mut vec![sym_id],
            ) {
                Ok(c) => Some(c),
                Err(sem_err) => {
                    let module = &self.compiler.mods[self.current_mod.id];
                    self.reporter.report_semantic(
                        sem_err,
                        &module
                            .src_metadata
                            .as_ref()
                            .expect("core should not be resolved"),
                    );

                    None
                }
            };

            if let Some(cond) = cond_opt {
                glob_conds.push(cond);
            }
        }

        let enum_def = self.compiler.get_enum_mut(sym_id);

        enum_def.variants.append(&mut variants);
        enum_def.glob_conds = glob_conds;
        enum_def.glob_args = abs_enum.glob_args.iter().map(|sp_arg| sp_arg.arg).collect();

        Ok(())
    }

    fn resolve_alias(&mut self, abs_alias: &AbstractAlias, ast_id: AstId) -> Result<(), ()> {
        let scope_id = self
            .compiler
            .extract_scope_id(ScopeType::Neutral, self.current_mod);
        let table = &self.compiler.get_scope_mut(scope_id).scope.table;

        let alias_sym_id = table.ast_to_sym[&ast_id];
        let associated_scope = AssociatedScopeKind::Module(self.current_mod);

        let local_scope_id = self.compiler.get_alias(alias_sym_id).local_scope_id;

        let mut params: Vec<Param> = Vec::new();
        let mut seen: Vec<(usize, InternedId)> = Vec::new();

        // Just a bit crowded in here..
        // WARN: Ok this just looks like an inlined function now
        for (i, abs_param) in abs_alias.params.iter().enumerate() {
            if let Some(original) = seen.iter().find(|other| abs_param.name_id == other.1) {
                let alias_name = self.interner.search(abs_alias.name_id.id as usize);
                let dup_name = self.interner.search(abs_param.name_id.id as usize);

                let orig_span = abs_alias.params[original.0].name_span;

                let msg = format!(
                    "More than one variable has the identifier \"{dup_name}\" within alias `{alias_name}`"
                );

                let module = &self.compiler.mods[self.current_mod.id];
                self.reporter.report_spanned(
                    &msg,
                    None,
                    &[orig_span, abs_param.name_span],
                    &module
                        .src_metadata
                        .as_ref()
                        .expect("core should not be resolved"),
                );
            }

            seen.push((i, abs_param.name_id));

            let expr_id = ExprId::new(self.compiler.exprs.len() as u32);
            let val_id = ValueId::new(self.compiler.values.len() as u32);

            let type_id = match self.resolve_type_expr(
                AssociatedScopeKind::Module(self.current_mod),
                &abs_param.ty_expr,
                ScopeType::Neutral,
                LookupPattern::NoRestrictions,
            ) {
                Ok(tid) => tid,
                Err(sem_err) => {
                    let module = &self.compiler.mods[self.current_mod.id];
                    self.reporter.report_semantic(
                        sem_err,
                        &module
                            .src_metadata
                            .as_ref()
                            .expect("core should not be resolved"),
                    );

                    return Err(());
                }
            };

            let param_sym_id = SymbolId::new(self.compiler.symbols.len() as u32);
            let param_sym = Symbol::new(
                abs_param.name_id,
                param_sym_id,
                Some(AstId::new(i as u32)),
                self.current_mod,
                true,
                None,
                ScopeType::Local,
                SymbolKind::Val(val_id),
            );

            let expr_hir = ExprHir::Var(param_sym_id);
            let resolved_expr =
                ResolvedExpr::new(type_id, expr_hir, val_id, abs_param.name_span, Vec::new());

            // Can this be possibly const evaluated if if possible if?
            //
            // Not sure about this

            let val_info = ValueInfo::new(type_id, expr_id, None);

            self.compiler.symbols.push(param_sym);

            self.compiler.exprs.push(resolved_expr);
            self.compiler.values.push(val_info);

            let local_scope = &mut self.compiler.get_scope_mut(local_scope_id).scope;
            local_scope
                .table
                .interned_to_sym
                .insert(abs_param.name_id, param_sym_id);

            let param = Param::new(param_sym_id, type_id, AstId::new(i as u32));

            params.push(param);
        }

        let mut conds: Vec<ExprId> = Vec::new();
        for spanned_expr in &abs_alias.conds {
            let cond_opt = match self.register_expr(
                alias_sym_id,
                spanned_expr,
                Some(local_scope_id),
                //NOTE: Could this change?
                associated_scope,
                ScopeType::Neutral,
                &mut vec![alias_sym_id],
            ) {
                Ok(c) => Some(c),
                Err(sem_err) => {
                    let module = &self.compiler.mods[self.current_mod.id];
                    self.reporter.report_semantic(
                        sem_err,
                        &module
                            .src_metadata
                            .as_ref()
                            .expect("core should not be resolved"),
                    );

                    None
                }
            };

            if let Some(cond) = cond_opt {
                conds.push(cond);
            }
        }

        //TODO: Arg constraint and option tpe constraint.
        //Could technically happen in constraint resolver since it. Yes.
        let alias_def = self.compiler.get_alias_mut(alias_sym_id);
        let param_count = params.len() as u32;

        //WARN: Does not yet have constraints of params discovered
        alias_def.params = params;
        // This could just be an explicit field, but in case of future changes keeping it under the
        // same layer of arg constraints so it's compatible with the already present checks in
        // `ConstraintResolver`.
        alias_def
            .arg_constraints
            .push(ArgConstraint::ArgCount(param_count));

        alias_def.conds = conds;
        // May change it so spanning is preserved
        alias_def.args = abs_alias.args.iter().map(|sp_arg| sp_arg.arg).collect();

        Ok(())
    }
    // These params are getting a little inflated so maybe a ctx struct for this environment could
    // be @()@$_ something

    /// On `Ok`, Creates a HIR expression type and returns the `ExprId` which is either going to be
    /// fully resolved, or marked as pending to be resolved later if possible.
    fn register_expr(
        &mut self,
        parent_sym_id: SymbolId,
        spanned_expr: &SpannedExpr,
        // Only usable with something like, alias(x) where x is local, not section local overall
        // like var->
        local_scope_id: Option<ScopeId>,
        associated_scope: AssociatedScopeKind,
        scope_type: ScopeType,
        seen: &mut Vec<SymbolId>,
    ) -> Result<ExprId, SemanticError> {
        match &spanned_expr.expr {
            Expr::Var(name_id) => {
                if let Some(scope_id) = local_scope_id {
                    //FIXME:
                    if let Some(local_sym_id) =
                        scopes::get_sym_id_local(self.compiler, scope_id, *name_id)
                    {
                        // Stores it like this because local symbols can only be parameters, and
                        // parameters are inferred in type, so they are basically just variables
                        // that have their own process of being resolved

                        // Not sure if this should be a known index or not yet depending on what
                        // the constraint type becomes
                        let expr_id = ExprId::new(self.compiler.exprs.len() as u32);
                        let expr = match self.compiler.symbols[local_sym_id.id as usize].kind {
                            SymbolKind::Val(val_id) => {
                                let val_info = &self.compiler.values[val_id.id as usize];

                                let expr_hir = ExprHir::Var(local_sym_id);

                                ResolvedExpr::new(
                                    val_info.type_id,
                                    expr_hir,
                                    val_id,
                                    spanned_expr.span,
                                    Vec::new(),
                                )
                            }
                            SymbolKind::Type(type_id) => todo!(),
                            SymbolKind::Module(mod_id) => todo!(),
                            SymbolKind::ReservedTypeSlot(type_id) => todo!(),
                        };

                        self.compiler.exprs.push(expr);

                        return Ok(expr_id);
                    }
                }

                if let Some(found_sym_id) = scopes::get_sym_id(
                    self.compiler,
                    associated_scope,
                    *name_id,
                    scope_type,
                    // Should this be no restrictions?
                    LookupPattern::NoRestrictions,
                ) {
                    //WARN: Constant iteration upon seeing any symbol instead of a single check
                    //elsewhere
                    seen.push(found_sym_id);

                    // Code duplication reduction
                    self.check_cycle(seen, parent_sym_id, found_sym_id)?;

                    //NOTE: Only the PendingSymbol struct carries the PendingExpr struct, meaning
                    //there is no way to check for cycles outside of `TypeContext`, so this has to
                    //pick up the edge case of, "let x = x". Could change.
                    if found_sym_id == parent_sym_id {
                        let name = self.interner.search(
                            self.compiler.symbols[found_sym_id.id as usize].name_id.id as usize,
                        );

                        let msg = format!("Cannot declare symbol `{name}` as itself");

                        let parent_ast_id = self.compiler.symbols[parent_sym_id.id as usize].ast_id;
                        let mut spans = Vec::new();
                        spans.push(spanned_expr.span);

                        if let Some(ast_id) = parent_ast_id {
                            let ast_span = self.ast_info.get_sym_span(ast_id);
                            spans.push(ast_span);
                        };

                        return Err(SemanticError::General(msg, spans));
                    }

                    let symbol = &self.compiler.symbols[found_sym_id.id as usize];
                    let expr_id = ExprId::new(self.compiler.exprs.len() as u32);

                    // I don't think this is needed since types are already known
                    let resolved_expr = match symbol.kind {
                        //WARN: Should this be the same?
                        SymbolKind::Type(type_id) => {
                            // Not sure what to do with this yet
                            // This would make types expressions, which wasn't true before
                            let ty_info = &self.compiler.types[type_id.id as usize];
                            //TODO: Alias is being looked up and seen as a type, not a
                            //function-like entity
                            match &ty_info.ty {
                                Type::Func(_) | Type::Alias(_) => {
                                    let val_id = ValueId::new(self.compiler.values.len() as u32);
                                    let val_info = ValueInfo::new(type_id, expr_id, None);
                                    self.compiler.values.push(val_info);

                                    let expr_hir = ExprHir::Var(found_sym_id);

                                    ResolvedExpr::new(
                                        type_id,
                                        expr_hir,
                                        val_id,
                                        spanned_expr.span,
                                        Vec::new(),
                                    )
                                }
                                Type::BuiltinType(_)
                                | Type::Struct(_)
                                | Type::Enum(_)
                                | Type::TypeDef(_)
                                | Type::Unknown => {
                                    let msg = "Cannot have a type within expressions".to_string();

                                    return Err(SemanticError::General(
                                        msg,
                                        vec![spanned_expr.span],
                                    ));
                                }
                                Type::Constrained(ty_constraint) => todo!(),
                                Type::Deferred(type_id) => todo!("Is this possible?"),
                            }
                        }
                        SymbolKind::Val(val_id) => {
                            let val_info = &self.compiler.values[val_id.id as usize];
                            let ty = &self.compiler.types[val_info.type_id.id as usize].ty;

                            if let Type::Unknown = ty {
                                let pending_expr = PendingExpr::new(expr_id, parent_sym_id);
                                self.ty_ctx.store_pending_expr(found_sym_id, pending_expr);
                            }

                            let expr_hir = ExprHir::Var(found_sym_id);

                            ResolvedExpr::new(
                                val_info.type_id,
                                expr_hir,
                                val_id,
                                spanned_expr.span,
                                Vec::new(),
                            )
                        }
                        SymbolKind::ReservedTypeSlot(reserved_ty_id) => {
                            let expr_id = ExprId::new(self.compiler.exprs.len() as u32);
                            let expr_hir = ExprHir::Var(found_sym_id);
                            let pending_expr = PendingExpr::new(expr_id, parent_sym_id);

                            //NOTE: ONLY THIS POINT SHOULD STORE THE SYMBOL. This is how the
                            //connection is made so that, y = x + 2, goes from x -> x + 2 -> None
                            //after x is resolved.
                            self.ty_ctx.store_pending_expr(found_sym_id, pending_expr);
                            // Will possibly call for others to be resolved here, or do it from the
                            // var resolution method itself

                            // Creates value id that has an unknown type, no constant value, and an
                            // unresolved expression.
                            let val_id = ValueId::new(self.compiler.values.len() as u32);
                            let val_info = ValueInfo::new(reserved_ty_id, expr_id, None);

                            self.compiler.values.push(val_info);

                            ResolvedExpr::new(
                                reserved_ty_id,
                                expr_hir,
                                val_id,
                                spanned_expr.span,
                                Vec::new(),
                            )
                        }
                        SymbolKind::Module(_) => {
                            let err_mod_name = self.interner.search(name_id.id as usize);
                            let msg = format!(
                                "The symbol `{err_mod_name}` is a module, which cannot be assigned as an expression value"
                            );

                            return Err(SemanticError::General(msg, vec![spanned_expr.span]));
                        }
                    };

                    self.compiler.exprs.push(resolved_expr);

                    Ok(expr_id)
                } else {
                    let ident = self.interner.search(name_id.id as usize);
                    // if ident == "_" {
                    //     panic!("hi");
                    // }

                    // SemanticError needs centralization
                    let module = &self.compiler.mods[self.current_mod.id];
                    let mod_name = self.interner.search(module.name_id.id as usize);

                    let and_local = if local_scope_id.is_some() {
                        " and local"
                    } else {
                        ""
                    };

                    let msg = format!(
                        "The symbol `{ident}` was not found in the module `{mod_name}` within `{scope_type}`{and_local} searchable scopes"
                    );

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
                    let type_id = TypeId::new(script_compiler::CORE_I64);

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
                    let type_id = TypeId::new(script_compiler::CORE_F64);
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
                let lhs_id = self.register_expr(
                    parent_sym_id,
                    &*lhs,
                    local_scope_id,
                    associated_scope,
                    scope_type,
                    seen,
                )?;

                let rhs_id = self.register_expr(
                    parent_sym_id,
                    &*rhs,
                    local_scope_id,
                    associated_scope,
                    scope_type,
                    seen,
                )?;

                let lhs_expr = &self.compiler.exprs[lhs_id.id as usize];
                let rhs_expr = &self.compiler.exprs[rhs_id.id as usize];

                let lhs_is_unknown = self.compiler.check_unknown(lhs_expr.type_id);
                let rhs_is_unknown = self.compiler.check_unknown(rhs_expr.type_id);

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
                            && !lhs_is_unknown
                            && !rhs_is_unknown
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

                // Maybe apply BinaryOp shouuld account for unknowns and return unknowns
                let type_id = self.infer_type_from_binary_op(
                    lhs_id,
                    rhs_id,
                    lhs_is_unknown,
                    *op,
                    rhs_is_unknown,
                    &const_val_opt,
                );

                // Assigning the user so that if unresolved, the expression can later go up a tree
                // of all expressions that use it and have them be resolved alongside it where
                // possible.
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

                self.compiler.exprs.push(resolved_expr);
                self.compiler.values.push(val_info);

                Ok(expr_id)
            }
            Expr::Char(c) => {
                let expr_id = ExprId::new(self.compiler.exprs.len() as u32);
                let val_id = ValueId::new(self.compiler.values.len() as u32);
                let type_id = TypeId::new(script_compiler::CORE_CHAR);

                let val = Value::Char(*c);
                let val_info = ValueInfo::new(type_id, expr_id, Some(val));
                self.compiler.values.push(val_info);

                let expr_hir = ExprHir::Val(val_id);
                let resolved_expr =
                    ResolvedExpr::new(type_id, expr_hir, val_id, spanned_expr.span, Vec::new());
                self.compiler.exprs.push(resolved_expr);

                Ok(expr_id)
            }
            Expr::Default(ident_expr, spanned_expr) => {
                let expr_id = ExprId::new(self.compiler.exprs.len() as u32);
                let val_id = ValueId::new(self.compiler.values.len() as u32);

                //WARN: SUSPICIOUS
                let default_ident_expr_id = self.register_expr(
                    parent_sym_id,
                    &ident_expr,
                    local_scope_id,
                    associated_scope,
                    scope_type,
                    seen,
                )?;

                let default_val_expr_id = self.register_expr(
                    parent_sym_id,
                    &spanned_expr,
                    local_scope_id,
                    associated_scope,
                    scope_type,
                    seen,
                )?;

                // Need the entire alias to use this as it's type through checks
                let type_id = self.compiler.exprs[default_val_expr_id.id as usize].type_id;

                //TODO: Need symbol of name id
                //Need it's inputs to be the symbol and spanned expression

                // DO NOT QUESTION THIS
                //WARN: Needs to be a smimbol
                let expr_hir = ExprHir::Default(default_ident_expr_id, default_val_expr_id);

                // Is the parameter an input if it doesn't have a value?
                // The issue is, it's not a known input of any sort, it's just an identifier.
                // Also, default is just a default so default only defaults when default defaults
                let resolved_expr = ResolvedExpr::new(
                    type_id,
                    expr_hir,
                    val_id,
                    spanned_expr.span,
                    vec![default_val_expr_id],
                );

                self.compiler.exprs[default_ident_expr_id.id as usize].user = Some(expr_id);
                self.compiler.exprs[default_val_expr_id.id as usize].user = Some(expr_id);

                self.compiler.exprs.push(resolved_expr);

                Ok(expr_id)
            }
            Expr::Str(name_id) => {
                let expr_id = ExprId::new(self.compiler.exprs.len() as u32);
                let val_id = ValueId::new(self.compiler.values.len() as u32);

                let type_id = TypeId::new(script_compiler::CORE_STR);

                let val = Value::InternedStr(*name_id);
                let val_info = ValueInfo::new(type_id, expr_id, Some(val));
                self.compiler.values.push(val_info);

                let expr_hir = ExprHir::Val(val_id);
                let resolved_expr =
                    ResolvedExpr::new(type_id, expr_hir, val_id, spanned_expr.span, Vec::new());
                self.compiler.exprs.push(resolved_expr);

                Ok(expr_id)
            }
            Expr::Unary(unary) => {
                let operand_id = self.register_expr(
                    parent_sym_id,
                    &unary.spanned_expr,
                    local_scope_id,
                    associated_scope,
                    scope_type,
                    seen,
                )?;

                let operand_expr = &self.compiler.exprs[operand_id.id as usize];

                let is_unknown = self.compiler.check_unknown(operand_expr.type_id);

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
                    let type_id = TypeId::new(self.compiler.types.len() as u32);
                    let ty_info = TypeInfo::new(Type::Unknown, self.current_mod);
                    self.compiler.types.push(ty_info);

                    type_id
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
                let type_id = TypeId::new(script_compiler::CORE_BOOL);
                if *boolean == true {
                    let expr_id = ExprId::new(self.compiler.exprs.len() as u32);
                    let val_id = ValueId::new(self.compiler.values.len() as u32);

                    let val = Value::Bool(true);
                    let val_info = ValueInfo::new(type_id, expr_id, Some(val));

                    let expr_hir = ExprHir::Val(val_id);
                    let resolved_expr =
                        ResolvedExpr::new(type_id, expr_hir, val_id, spanned_expr.span, vec![]);

                    self.compiler.exprs.push(resolved_expr);
                    self.compiler.values.push(val_info);

                    Ok(expr_id)
                } else {
                    let expr_id = ExprId::new(self.compiler.exprs.len() as u32);
                    let val_id = ValueId::new(self.compiler.values.len() as u32);

                    // Generics can only be thest types so this can stay for now
                    let val = Value::Bool(false);
                    let val_info = ValueInfo::new(type_id, expr_id, Some(val));

                    let expr_hir = ExprHir::Val(val_id);
                    let resolved_expr =
                        ResolvedExpr::new(type_id, expr_hir, val_id, spanned_expr.span, vec![]);

                    self.compiler.exprs.push(resolved_expr);
                    self.compiler.values.push(val_info);

                    Ok(expr_id)
                }
            }
            Expr::Call(caller, arg_exprs) => {
                // The "Call" in "Call(x, y)"
                let caller_id = self.register_expr(
                    parent_sym_id,
                    caller,
                    local_scope_id,
                    associated_scope,
                    scope_type,
                    seen,
                )?;
                //WARN: Does this need something?
                let type_id = self.compiler.exprs[caller_id.id as usize].type_id;
                let mut call_args: Vec<ExprId> = Vec::new();

                for sp_expr in arg_exprs {
                    let arg = self.register_expr(
                        parent_sym_id,
                        sp_expr,
                        local_scope_id,
                        associated_scope,
                        scope_type,
                        seen,
                    )?;

                    call_args.push(arg);
                }

                let expr_id = ExprId::new(self.compiler.exprs.len() as u32);
                let val_id = ValueId::new(self.compiler.values.len() as u32);

                let inputs = call_args.clone();

                let expr_hir = ExprHir::Call(caller_id, call_args);
                // Are the arguments inputs if they are the expression itself?
                let resolved_expr =
                    ResolvedExpr::new(type_id, expr_hir, val_id, spanned_expr.span, inputs);
                let val_info = ValueInfo::new(type_id, expr_id, None);

                self.compiler.exprs.push(resolved_expr);
                self.compiler.values.push(val_info);

                Ok(expr_id)
            }
            Expr::MemberAccess(abs_member_access) => {
                match self.resolve_member(
                    parent_sym_id,
                    &abs_member_access.base,
                    local_scope_id,
                    associated_scope,
                    scope_type,
                    seen,
                )? {
                    PossibleMember::Module(extern_mod_id) => {
                        let extern_mod = &self.compiler.mods[extern_mod_id.id];
                        //NOTE: Maybe privacy should be checked from this resolver?
                        if let Some(extern_sym_id) = scopes::get_sym_id(
                            self.compiler,
                            todo!(),
                            abs_member_access.field,
                            scope_type,
                            LookupPattern::NamespaceOnly,
                        ) {
                            seen.push(extern_sym_id);

                            // Dirtiness to prevent O(n) check of each and every expression in
                            // whatever given symbol was used?
                            self.check_cycle(seen, parent_sym_id, extern_sym_id)?;

                            if extern_sym_id == parent_sym_id {
                                let name = self.interner.search(
                                    self.compiler.symbols[extern_sym_id.id as usize].name_id.id
                                        as usize,
                                );
                                let msg = format!("Cannot declare symbol `{name}` as itself");

                                let parent_ast_id =
                                    self.compiler.symbols[parent_sym_id.id as usize].ast_id;
                                let mut spans = Vec::new();
                                spans.push(spanned_expr.span);

                                if let Some(ast_id) = parent_ast_id {
                                    let ast_span = self.ast_info.get_sym_span(ast_id);
                                    spans.push(ast_span);
                                };

                                return Err(SemanticError::General(msg, spans));
                            }

                            let symbol = &self.compiler.symbols[extern_sym_id.id as usize];

                            // May move this privacy check elsewhere
                            if symbol.owner != self.current_mod && symbol.is_priv {
                                let name = self.interner.search(symbol.name_id.id as usize);
                                let msg = format!("The symbol `{name}` is private");

                                let mod_origin = &self.compiler.mods[self.current_mod.id];
                                self.reporter.report_spanned(
                                    &msg,
                                    None,
                                    &[spanned_expr.span],
                                    &mod_origin
                                        .src_metadata
                                        .as_ref()
                                        .expect("core should not be resolved"),
                                );
                            }

                            // Import checking

                            let current_module = &self.compiler.mods[self.current_mod.id];
                            if !current_module.contains_import(extern_mod) {
                                let extern_name =
                                    self.interner.search(extern_mod.name_id.id as usize);
                                let current_name =
                                    self.interner.search(current_module.name_id.id as usize);
                                let msg = format!(
                                    "The module `{extern_name}` exists but was not imported by `{current_name}`"
                                );

                                return Err(SemanticError::General(msg, vec![spanned_expr.span]));
                            };

                            // Turning the field into an expression so that it can be resolved as
                            // normal
                            // This SEEMS ok?
                            // WARN: No
                            let inline_expr = Expr::Var(abs_member_access.field);
                            let sp_expr = SpannedExpr::new(inline_expr, spanned_expr.span);

                            self.register_expr(
                                parent_sym_id,
                                &sp_expr,
                                local_scope_id,
                                todo!(),
                                scope_type,
                                seen,
                            )
                        } else {
                            // TODO: Should also show what scopes were searched or just in some
                            // form at all state why a symbol that exists wasn't seen
                            // Find similar symbols
                            let extern_mod = &self.compiler.mods[extern_mod_id.id];

                            // Misleading error message. Very misleading.
                            let msg = format!(
                                "Could not find the symbol `{}` inside module `{}` within `{scope_type}` searchable scopes",
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
                        unimplemented!("Nothing matches this case yet");
                    }
                    PossibleMember::Nothing => todo!("Unresolved"),
                }
            }
            // Should probably be delegated to a function so that it asks for spanned interned ids
            // in general
            Expr::StaticAccess(spanned_segments) => {
                let last_scope = self.resolve_static_access(
                    spanned_segments,
                    associated_scope,
                    scope_type,
                    false,
                )?;

                let last_seg = &spanned_segments[spanned_segments.len() - 1];

                // This is a little odd, but it technically isn't different from if it were
                // classified as an expr in the first place. This is done since, making paths take
                // in a generic "Expr" would be an insanely large amount of possibilites for
                // something that is enforced at parse-time, making it more confusing. But,
                // creating inline expressions is also confusing so, not sure.
                let inline_expr = match &last_seg.kind {
                    PathSegment::Ident(interned_id) => {
                        SpannedExpr::new(Expr::Var(*interned_id), last_seg.span)
                    }
                    PathSegment::Generic(_) => {
                        let msg = "Generics are not valid in expressions".to_string();
                        return Err(SemanticError::General(msg, vec![last_seg.span]));
                    }
                };

                self.register_expr(
                    parent_sym_id,
                    &inline_expr,
                    local_scope_id,
                    last_scope,
                    scope_type,
                    seen,
                )
            }
        }
    }

    // Ok maybe this should be separated a bit more
    /// Method so that code can be re-used for traversing scopes in a static access.
    ///
    /// Takes in the segments to traverse, scope to start in, scope type for scoping rules, and
    /// whether or not type expression restrictions should be applied.
    ///
    /// Returns an `Ok` with the last scope found so that wherever this was called from can use the
    /// last segment for it's correct use-case.
    /// Returns an `Err` upon any errors, given whether or not a type expression was the caller.
    fn resolve_static_access(
        &mut self,
        spanned_path_segs: &[SpannedPathSegment],
        mut current_scope: AssociatedScopeKind,
        scope_type: ScopeType,
        in_ty_expr: bool,
    ) -> Result<AssociatedScopeKind, SemanticError> {
        for (i, sp_path_seg) in spanned_path_segs.iter().enumerate() {
            match &sp_path_seg.kind {
                PathSegment::Ident(interned_id) => {
                    if let Some(sym_id) = scopes::get_sym_id(
                        self.compiler,
                        current_scope,
                        *interned_id,
                        scope_type,
                        LookupPattern::NamespaceOnly,
                    ) {
                        match self.compiler.symbols[sym_id.id as usize].associated_scope {
                            Some(new_scope) => current_scope = new_scope,
                            // meaning the search is DONE
                            None => {
                                if i + 1 < spanned_path_segs.len() {
                                    panic!("Woah");
                                }
                            }
                        }
                        // Symbol not found
                    } else {
                        let current_namespace = self.interner.search(interned_id.id as usize);

                        let prev_namespace_opt = if i > 0 {
                            Some(&spanned_path_segs[i - 1])
                        } else {
                            None
                        };

                        // Different error message depending on if at least the first
                        // member was resolved or not
                        let msg = if let Some(prev) = prev_namespace_opt {
                            let prev_namespace = match &prev.kind {
                                PathSegment::Ident(prev_name_id) => {
                                    self.interner.search(prev_name_id.id as usize)
                                }
                                PathSegment::Generic(_) => {
                                    // Represents "module::Generic<T>::stuff" where the middle
                                    // generic has the ability to access members.
                                    // Which is not possible right now.
                                    unimplemented!("Generics may never exist in this form.");
                                }
                            };

                            let msg = format!(
                                "Could not find the symbol `{}` in the namespace `{}`",
                                current_namespace, prev_namespace
                            );

                            msg
                        } else {
                            // Specific scope mention?
                            let msg = format!(
                                "The symbol `{current_namespace}` was not found in all `{scope_type}` searchable scopes"
                            );

                            msg
                        };

                        return Err(SemanticError::General(msg, vec![sp_path_seg.span]));
                    };
                }
                PathSegment::Generic(_) if in_ty_expr => {
                    // Still disallows something like, core.List<i32>.other_thing
                    if i + 1 != spanned_path_segs.len() {
                        let msg = "Generics cannot use \"::\" pathing at any point".to_string();
                        return Err(SemanticError::General(msg, vec![sp_path_seg.span]));
                    }

                    break;
                }
                PathSegment::Generic(_) => {
                    unreachable!("The parser does not pick this up in `parse_expr`");
                    let msg = "Generics cannot be used inside of expressions".to_string();
                    return Err(SemanticError::General(msg, vec![sp_path_seg.span]));
                }
            }
        }

        Ok(current_scope)
    }

    fn resolve_member(
        &mut self,
        sym_parent: SymbolId,
        member: &SpannedExpr,
        local_scope: Option<ScopeId>,
        associated_scope: AssociatedScopeKind,
        scope_type: ScopeType,
        seen: &mut Vec<SymbolId>,
    ) -> Result<PossibleMember, SemanticError> {
        let res = self.register_expr(
            sym_parent,
            member,
            local_scope,
            associated_scope,
            scope_type,
            seen,
        )?;
        dbg!(res);
        panic!();

        if let Ok(expr_id) = self.register_expr(
            sym_parent,
            member,
            local_scope,
            associated_scope,
            scope_type,
            seen,
        ) {
            let resolved_expr = &self.compiler.exprs[expr_id.id as usize];

            todo!();
        }

        if let Expr::Var(name_id) = member.expr {
            if let Some(mod_id) = self.compiler.mod_map.get(&name_id) {
                return Ok(PossibleMember::Module(*mod_id));
            }
            panic!();

            if let Some(sym_id) = scopes::get_sym_id(
                self.compiler,
                todo!(),
                name_id,
                scope_type,
                LookupPattern::NoRestrictions,
            ) {
                todo!();
                // let type_id = self.compiler.symbols[sym_id.id as usize];
                // return Ok(PossibleMember::Type(type_id));
            } else {
                let msg = format!(
                    "Could not find the symbol `{}` as a module or value",
                    self.interner.search(name_id.id as usize)
                );

                return Err(SemanticError::General(msg, vec![member.span]));
            }
        }

        Err(SemanticError::UndefinedMember(member.span))
    }

    // Ok
    // NOTE: Temp helper
    fn infer_type_from_binary_op(
        &mut self,
        lhs_expr_id: ExprId,
        rhs_expr_id: ExprId,
        lhs_is_unknown: bool,
        op: BinaryOp,
        rhs_is_unknown: bool,
        const_val_opt: &Option<Value>,
    ) -> TypeId {
        let lhs_expr = &self.compiler.exprs[lhs_expr_id.id as usize];
        let rhs_expr = &self.compiler.exprs[rhs_expr_id.id as usize];
        match &const_val_opt {
            Some(val) => match val {
                Value::I64(_) => TypeId::new(script_compiler::CORE_I64),
                Value::F64(_) => TypeId::new(script_compiler::CORE_F64),
                Value::Bool(_) => TypeId::new(script_compiler::CORE_BOOL),
                Value::Char(_) => TypeId::new(script_compiler::CORE_CHAR),
                Value::Func(func_sym) => {
                    let func_def = self.compiler.get_func(*func_sym);
                    func_def.ret_type
                }
                Value::InternedStr(_) => TypeId::new(script_compiler::CORE_STR),
                // Both of these are not possible as of right now from an operation
                // since there are no runtime values RIGHT NOW, and unknown is not a comptaible
                // binary op so it can't acually be produced.
                // Tuples also are not used outside of expressing type constraints.
                //
                // Value::RuntimeStr(_) => TypeId::new(script_compiler::CORE_STR),
                // Value::Tuple(_) => TypeId::new(script_compiler::CORE_TUPLE),
                // Value::Unknown => TypeId::new(script_compiler::TYPE_UNKNOWN_IDX),
                Value::Tuple(_) | Value::RuntimeStr(_) | Value::Unknown => unreachable!(),
            },
            None => match op {
                // Maybe this can still point to unknown?
                BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mult | BinaryOp::Div | BinaryOp::Mod => {
                    // Makes new reserved type which represents unknown
                    if lhs_is_unknown && rhs_is_unknown {
                        let type_id = TypeId::new(self.compiler.types.len() as u32);
                        let ty_info = TypeInfo::new(Type::Unknown, self.current_mod);
                        self.compiler.types.push(ty_info);

                        type_id
                    } else if rhs_is_unknown {
                        lhs_expr.type_id
                    } else {
                        rhs_expr.type_id
                    }
                }
                BinaryOp::Greater
                | BinaryOp::Less
                | BinaryOp::GreaterOrEq
                | BinaryOp::And
                | BinaryOp::Or
                | BinaryOp::EqTo
                | BinaryOp::NotEq
                | BinaryOp::LessOrEq => TypeId::new(script_compiler::CORE_BOOL),
                // Bitwise doesn't exist yet
                BinaryOp::BitOr => todo!(),
                BinaryOp::BitAnd => todo!(),
                BinaryOp::BitNot => todo!(),
                BinaryOp::BitRightShift => todo!(),
                BinaryOp::BitLeftShift => todo!(),
                BinaryOp::BitXor => todo!(),
            },
        }
    }

    // Helper
    fn check_cycle(
        &self,
        seen: &Vec<SymbolId>,
        parent_sym_id: SymbolId,
        found_sym_id: SymbolId,
    ) -> Result<(), SemanticError> {
        for seen_sym_id in seen.iter() {
            // In:
            // ```
            // let a = b
            // let b = c
            // let c = b
            // ```
            // Within b, it checks of the symbol a is inside of `TypeContext`, and
            // if that a depends on symbol b
            if let Some(pending_sym) = self.ty_ctx.sym_queue.get(seen_sym_id) {
                let has_cycle = pending_sym
                    .pending_exprs
                    .iter()
                    .any(|pend_expr| pend_expr.parent_sym == found_sym_id);

                if has_cycle {
                    let parent_sym = &self.compiler.symbols[parent_sym_id.id as usize];
                    let parent_name = self.interner.search(parent_sym.name_id.id as usize);
                    let parent_ast_id = parent_sym.ast_id.expect("core should not be resolved");

                    let found_sym = &self.compiler.symbols[found_sym_id.id as usize];
                    let found_ast_id = found_sym.ast_id.expect("core should not be resolved");
                    let found_name = self.interner.search(found_sym.name_id.id as usize);

                    let cycled_span = self.ast_info.get_sym_span(parent_ast_id);
                    let found_span = self.ast_info.get_sym_span(found_ast_id);

                    let msg = format!(
                        "`{}` depends on itself through `{}`",
                        parent_name, found_name
                    );

                    return Err(SemanticError::General(msg, vec![cycled_span, found_span]));
                }
            }
        }

        Ok(())
    }

    /// - active_mod_id: The target module to search which is only altered if an external module is
    /// used within a member access
    /// - spanned_ty_expr: The type expression to be resolved
    /// - scope_type: The scope which determines how much of a module can be searched.
    /// - lookup_pattern: The type of lookup which is recursively changed depending on if a direct
    /// member access is being searched, or if a library such as core can be searched externally.
    fn resolve_type_expr(
        &mut self,
        // Module that is actively being searched within, not the source. Source remains
        // current_mod
        associated_scope: AssociatedScopeKind,
        spanned_ty_expr: &SpannedTypeExpr,
        scope_type: ScopeType,
        lookup_pattern: LookupPattern,
    ) -> Result<TypeId, SemanticError> {
        match &spanned_ty_expr.ty_expr {
            //FIXME: If an error occurs while self.current_mod = extern_mod, it tries to report the
            //error from the external module instead of the actual module of origin.
            TypeExpr::Var(name_id) => {
                // Searching symbols because otherwise, the type of a variable would be valid
                // since it would just be looking at it's type, which is not a favorable allowable syntax
                // So, let x = 3, var-> field: x, would be valid if this weren't handled at the
                // symbol level here
                match scopes::get_sym_id(
                    self.compiler,
                    associated_scope,
                    *name_id,
                    scope_type,
                    lookup_pattern,
                ) {
                    Some(sym_id) => match self.compiler.symbols[sym_id.id as usize].kind {
                        SymbolKind::Type(type_id) => {
                            // NOTE: Will probably error later in resolution but fine for now
                            let symbol = &self.compiler.symbols[sym_id.id as usize];
                            if symbol.is_priv && symbol.owner != self.current_mod {
                                //FIX: Would need changes
                                let current_mod = &self.compiler.mods[self.current_mod.id];
                                let current_mod_name =
                                    self.interner.search(current_mod.name_id.id as usize);
                                let sym_name = self.interner.search(symbol.name_id.id as usize);

                                let msg = format!(
                                    "The type `{sym_name}` is private within namespace `{current_mod_name}`"
                                );

                                return Err(SemanticError::General(
                                    msg,
                                    vec![spanned_ty_expr.span],
                                ));
                            }

                            return Ok(type_id);
                        }
                        // Ok but what about, "core is a MODULE which is NOT a type?"
                        SymbolKind::Module(mod_id) => (),
                        SymbolKind::Val(_) | SymbolKind::ReservedTypeSlot(_) => (),
                    },
                    None => (),
                }
                //FIX:

                // If we have main, that imports def, that imports other, it tries to search for
                // things in the "other" module even though it's defined in "def".
                //
                // Within "def", it tries to search "other" for everything declared even if "other"
                // is never used.
                let err_name = self.interner.search(name_id.id as usize);
                let msg = match associated_scope {
                    AssociatedScopeKind::Module(mod_id) => {
                        let err_mod = &self.compiler.mods[mod_id.id];
                        let err_mod_name = self.interner.search(err_mod.name_id.id as usize);

                        format!(
                            "`{err_name}` is not defined as a type within the module `{err_mod_name}`"
                        )
                    }
                    AssociatedScopeKind::Scope(scope_id) => {
                        let scope_info = &self.compiler.scopes[scope_id.id];
                        // This is infailable because an associated scope having a scope variant
                        // means that the current search was performed by a namespace within a
                        // module, not a module directly.
                        let sym_owner = scope_info
                            .sym_owner
                            .expect("resolve_type_expr control flow broke");
                        let sym_name_id = self.compiler.symbols[sym_owner.id as usize].name_id;
                        let sym_name = self.interner.search(sym_name_id.id as usize);
                        format!(
                            "The symbol `{sym_name}` does not contain a type with the the identifier `{err_name}`"
                        )
                    }
                };

                Err(SemanticError::General(msg, vec![spanned_ty_expr.span]))
            }
            // Generics can only be these types so this can stay for now
            TypeExpr::Generic(generic) => {
                //FIX: This is still using the old id matching but maybe it's ok since this is
                //actually supposed to be specifically only known data structures
                match BuiltinTypeKind::try_from_interned_id(generic.base.id) {
                    // Self referential type ids used here
                    Some(kind) => match kind {
                        //TODO: Should maybe put List | Set
                        BuiltinTypeKind::List | BuiltinTypeKind::Set => {
                            if generic.args.len() != 1 {
                                let msg =
                                    format!("Expected only 1 type within `{}`", kind.to_fmt());

                                return Err(SemanticError::General(
                                    msg,
                                    vec![spanned_ty_expr.span],
                                ));
                            }

                            let inner = self.resolve_type_expr(
                                associated_scope,
                                &generic.args[0],
                                scope_type,
                                LookupPattern::NoRestrictions,
                            )?;

                            let ty = if kind == BuiltinTypeKind::List {
                                Type::BuiltinType(BuiltinType::List(inner))
                            } else {
                                Type::BuiltinType(BuiltinType::Set(inner))
                            };

                            let type_id = TypeId::new(self.compiler.types.len() as u32);

                            // TODO: Technically it's a structure owned by core, but it wasn't
                            // defined as core, but this can't be referenced directly anyways so it
                            // doesn't really make a difference
                            let ty_info = TypeInfo::new(ty, self.compiler.core_mod_id);
                            self.compiler.types.push(ty_info);

                            return Ok(type_id);
                        }
                        BuiltinTypeKind::Tuple => {
                            let mut elements: Vec<TypeId> = Vec::new();

                            for arg in &generic.args {
                                elements.push(self.resolve_type_expr(
                                    associated_scope,
                                    arg,
                                    scope_type,
                                    LookupPattern::NoRestrictions,
                                )?);
                            }

                            let type_id = TypeId::new(self.compiler.types.len() as u32);
                            let tuple = Type::BuiltinType(BuiltinType::Tuple(elements));

                            let ty_info = TypeInfo::new(tuple, self.compiler.core_mod_id);
                            self.compiler.types.push(ty_info);

                            return Ok(type_id);
                        }
                        BuiltinTypeKind::Map => {
                            if generic.args.len() != 2 {
                                let msg = format!("Expected only 2 types within `Map`",);

                                return Err(SemanticError::General(
                                    msg,
                                    vec![spanned_ty_expr.span],
                                ));
                            }

                            // Should it reset to current module if it has a new sesarch started?
                            let key = self.resolve_type_expr(
                                AssociatedScopeKind::Module(self.current_mod),
                                &generic.args[0],
                                scope_type,
                                LookupPattern::NoRestrictions,
                            )?;

                            let val = self.resolve_type_expr(
                                AssociatedScopeKind::Module(self.current_mod),
                                &generic.args[1],
                                scope_type,
                                LookupPattern::NoRestrictions,
                            )?;

                            let map = Type::BuiltinType(BuiltinType::Map(key, val));
                            let map_id = self.compiler.types.len() as u32;

                            let ty_info = TypeInfo::new(map, self.compiler.core_mod_id);
                            self.compiler.types.push(ty_info);

                            return Ok(TypeId::new(map_id));
                        }
                        // Returns nothing since both have the same error handling
                        _ => (),
                    },
                    None => (),
                }

                let err_name = self.interner.search(generic.base.id as usize);

                let msg = format!(
                    "Found identifier \"{err_name}\" before generic parameters, but only `List`, `Set`, `Map`, and `Tuple` are valid data structures"
                );

                Err(SemanticError::General(msg, vec![spanned_ty_expr.span]))
            }
            // This only allows something like, defs.Thing which can go to at most one type deep,
            // but no more. Will need change since something like i32.MAX could be "core.i32.MAX".
            //
            // Maybe not though since that would only be usable in expressions anyways which aren't
            // type expressions
            TypeExpr::Path(sp_path_segs) => {
                // maybe active_mod can be removed?
                let last_scope =
                    self.resolve_static_access(&sp_path_segs, associated_scope, scope_type, true)?;
                // for (i, sp_path_seg) in sp_path_segs.iter().enumerate() {
                //     let interned_id = match &sp_path_seg.kind {
                //         PathSegment::Ident(id) => *id,
                //         PathSegment::Generic(generic) => todo!(),
                //     };
                //
                //     todo!("Hi");
                //     match &sp_path_seg.kind {
                //         PathSegment::Ident(interned_id) => {
                //             if let Some(sym_id) = scopes::get_sym_id(
                //                 self.compiler,
                //                 current_scope,
                //                 *interned_id,
                //                 scope_type,
                //                 LookupPattern::NamespaceOnly,
                //             ) {
                //                 match self.compiler.symbols[sym_id.id as usize].associated_scope {
                //                     Some(new_scope) => current_scope = new_scope,
                //                     // meaning the search is DONE
                //                     None => break,
                //                 }
                //                 // Symbol not found
                //             } else {
                //                 let current_member_name =
                //                     self.interner.search(interned_id.id as usize);
                //
                //                 let prev_member_opt = if i > 0 {
                //                     Some(&sp_path_segs[i - 1])
                //                 } else {
                //                     None
                //                 };
                //
                //                 // Different error message depending on if at least the first
                //                 // member was resolved or not
                //                 let (msg, span) = if let Some(prev) = prev_member_opt {
                //                     let prev_member_name = match &prev.kind {
                //                         PathSegment::Ident(prev_name_id) => {
                //                             self.interner.search(prev_name_id.id as usize)
                //                         }
                //                         PathSegment::Generic(generic) => {
                //                             // Represents "module.Generic<T>.stuff" where the middle
                //                             // generic has the ability to access members
                //                             unimplemented!(
                //                                 "Generics may never exist in this form."
                //                             );
                //                         }
                //                     };
                //
                //                     let msg = format!(
                //                         "Could not find the symbol `{}` in the namespace `{}`",
                //                         current_member_name, prev_member_name
                //                     );
                //
                //                     let merged_span = prev.span.merge(sp_path_seg.span);
                //
                //                     (msg, merged_span)
                //                 } else {
                //                     // Specific scope mention?
                //                     let msg = format!(
                //                         "The symbol `{current_member_name}` was not found in all `{scope_type}` searchable scopes"
                //                     );
                //
                //                     (msg, sp_path_seg.span)
                //                 };
                //
                //                 self.reporter.report_spanned(
                //                     &msg,
                //                     None,
                //                     &[span],
                //                     &module
                //                         .src_metadata
                //                         .as_ref()
                //                         .expect("core should not be resolved"),
                //                 );
                //
                //                 return Err(());
                //             };
                //         }
                //         //TODO:
                //         PathSegment::Generic(_) => {
                //             // Still disallows something like, core.List<i32>.other_thing
                //             if i + 1 != sp_path_segs.len() {
                //                 let msg = "Generics cannot use any form of member access";
                //                 self.reporter.report_spanned(
                //                     msg,
                //                     None,
                //                     &[sp_path_seg.span],
                //                     &module
                //                         .src_metadata
                //                         .as_ref()
                //                         .expect("core should not be resolved"),
                //                 );
                //
                //                 return Err(());
                //             }
                //
                //             break;
                //         } // The path cannot recursively have itself
                //     }
                // }
                //
                let last_segment = &sp_path_segs[sp_path_segs.len() - 1];
                let inline_ty_expr = match &last_segment.kind {
                    PathSegment::Ident(interned_id) => {
                        SpannedTypeExpr::new(TypeExpr::Var(*interned_id), last_segment.span)
                    }
                    PathSegment::Generic(generic) => {
                        //FIXME: EVIL CLONING.
                        //Would need to a compability layer to allow for referenced inners, rather
                        //than only owned.
                        SpannedTypeExpr::new(TypeExpr::Generic(generic.clone()), last_segment.span)
                    }
                };

                self.resolve_type_expr(last_scope, &inline_ty_expr, scope_type, lookup_pattern)
            }
        }
    }
}
