//TODO: IMPORT CHECKING SYMBOLS.
pub mod type_context;

use std::collections::HashSet;

use chrn_utils::id_types::{
    AstId, ExprId, InternedId, ModuleId, ScopeId, SymbolId, TypeId, ValueId,
};
use chrn_utils::intern::Intern;
use chrn_utils::types::builtins::{BuiltinType, BuiltinTypeKind};
use chrn_utils::values::{Value, ValueInfo};
use common::chrn_settings::ChrnSettings;
use common::fmter::{Formattable, Formatted};
use common::{reporter::diagnostic::Diagnostic, span::Span};

use crate::parser::ast::{AbstractVar, BinaryOp, Expr, SpannedExpr};
use crate::script_compiler::{self, ScriptCompiler};
use crate::semantic::error::{MathError, SemanticError};
use crate::semantic::representation::{
    ExprHir, Param, PossibleMember, ResolvedExpr, Symbol, SymbolKind,
};
use crate::semantic::scopes::{LookupPattern, ScopeType};
use crate::semantic::type_resolver::type_context::{PendingExpr, PendingSymbol, TypeContext};
use crate::semantic::{evaluator, scopes};

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

        // This can maybe be removed if the cam_remove logic is changed to be more meaningful but
        // works for now
        let mut last_resolved_count = 0;
        let mut current_resolved_count = 0;
        while self.ty_ctx.needs_check {
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
                        // Not sure about this yet
                        if can_remove {
                            removable_syms.insert(*sym_id);
                            current_resolved_count += 1;
                        }
                    }
                    // Not sure if anything more can be done here since the diagnostic is already
                    // made
                    Err(_) => (),
                };
            }

            self.ty_ctx.sym_queue.extend(pending_syms);

            //TEST: Not confident in how well this works
            // The intent of this is to check if the symbol itself was resolved during the
            // traverse_expr innately but did not send a signal
            for (sym_id, pending_sym) in self.ty_ctx.sym_queue.iter_mut() {
                if pending_sym.is_resolved {
                    continue;
                }

                let type_id = match self.compiler.symbols[sym_id.id as usize].kind {
                    SymbolKind::Type(type_id) => type_id,
                    SymbolKind::Val(val_id) => self.compiler.values[val_id.id as usize].type_id,
                    SymbolKind::Unknown => TypeId::new(script_compiler::TYPE_UNKNOWN_IDX),
                };

                // The reason for some index checks, some deep checks, is because core should
                // probably not be using set indices upon loading, but parts still depend on said
                // index being given, so all possible not index specific checking is being used.
                let ty = &self.compiler.types[type_id.id as usize].ty;
                if let Type::Unknown = ty {
                    continue;
                }

                pending_sym.is_resolved = true;
                current_resolved_count += 1;
            }

            //TEMP or not I don't know
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

        // if self.current_mod == self.compiler.mods[self.compiler.mods.len() - 2].mod_id {
        //     dbg!(&self.ty_ctx);
        //     for symbol in &self.compiler.symbols {
        //         if self.interner.search(symbol.name_id.id as usize) == "d" {
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
        //                     }
        //                 }
        //                 SymbolKind::Unknown => todo!(),
        //             }
        //             panic!("Done");
        //         }
        //         dbg!(self.interner.search(symbol.name_id.id as usize));
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
        pending_sym: &PendingSymbol,
    ) -> Result<bool, ()> {
        // Tells the caller if the given pending symbol is fully resolved to where it can be
        // removed as a pending symbol
        let mut can_remove = false;
        let mut queue: Vec<ExprId> = Vec::new();

        //Suspicious
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

        // Tracking how many were resolved so it knows whether to remove or not
        let mut resolved_count = 0;

        // Needs to resolve first root
        for root_id in queue.iter().copied() {
            // Still need to repair root expr
            let root_expr = &mut self.compiler.exprs[root_id.id as usize];
            match self.compiler.symbols[resolved_sym_id.id as usize].kind {
                SymbolKind::Val(val_id) => {
                    // Brain starting working now it works
                    let val_info = &self.compiler.values[val_id.id as usize];
                    let type_id = val_info.type_id;
                    let const_val_opt = val_info.const_val.clone();

                    // Not sure if clone can be avoided here since mutating val_id so that it
                    // points to the current expr would mutate the symbol it was gotten from,
                    // which would mangle the resolved symbol itself even though we just want the
                    // const value if present.
                    root_expr.type_id = type_id;
                    let inner_val = &mut self.compiler.values[root_expr.val_id.id as usize];
                    inner_val.type_id = type_id;
                    inner_val.const_val = const_val_opt;
                }
                // Types are known without dependency tracking.
                // SymbolKind::Unknown is always turned into an expr_id
                SymbolKind::Type(_) | SymbolKind::Unknown => unreachable!("Not possible"),
            }

            if let Some(user) = root_expr.user {
                match self.traverse_expr(user) {
                    Ok(true) => resolved_count += 1,
                    // WARN: This case is not hit yet
                    Ok(false) => (),
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
                resolved_count += 1;
                break;
            }
        }

        // If all pending expressions were pushed into the queue and the entire queue was resolved then
        // can remove
        if queue.len() == pending_sym.pending_exprs.len() && resolved_count == queue.len() {
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
    fn traverse_expr(&mut self, current_expr_id: ExprId) -> Result<bool, SemanticError> {
        match &self.compiler.exprs[current_expr_id.id as usize].expr_hir {
            ExprHir::Val(val_id) => {
                let val_info = &self.compiler.values[val_id.id as usize];

                let type_id = val_info.type_id;
                let const_val_opt = val_info.const_val.clone();

                let expr = &mut self.compiler.exprs[current_expr_id.id as usize];
                expr.type_id = type_id;

                let inner_val = &mut self.compiler.values[expr.val_id.id as usize];
                inner_val.type_id = type_id;
                inner_val.const_val = const_val_opt;
                dbg!(inner_val);
                todo!("Make sure this is ok")
            }
            // Not sure if this is reachable since other than the root, there can't another
            // singular variable seen since, "let z = x y" is non-existen syntactically
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
                    if !evaluator::is_compatible_unary(*op, const_val) {
                        return Err(MathError::UnaryOpMismatch(
                            const_val.kind().to_fmt(),
                            op.to_fmt(),
                            vec![operand_expr.span],
                        ))?;
                    } else {
                        Some(evaluator::apply_unary_op(*op, const_val)?)
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
                        if !evaluator::is_compatible_binary(lhs_const, *op, rhs_const) {
                            let full_span = lhs_expr.span.merge(rhs_expr.span);

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
            //TODO:
            ExprHir::Call(expr_id, expr_ids) => todo!(),
            ExprHir::Var(sym_id) => {
                todo!("What is a varrrble")
            }
            // This may not be possibly since it expects a literal which is always const
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

        Ok(true)
    }

    fn resolve_var(&mut self, abs_var: &AbstractVar, ast_id: AstId) -> Result<(), ()> {
        let scope_id = self
            .compiler
            .extract_scope_id(ScopeType::Neutral, self.current_mod);
        let table = &mut self.compiler.get_scope_mut(scope_id).scope.table;

        let sym_id = table.ast_to_sym[&ast_id];

        //NOTE: Pipeline where expressions are always returned, just that some may have
        //unresolved parts, which are put into the queue, not the variable itself.
        let expr_id = match self.register_expr(
            sym_id,
            &abs_var.spanned_expr,
            None,
            self.current_mod,
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
        let symbol = self
            .compiler
            .symbols
            .get_mut(sym_id.id as usize)
            .expect("Exists");
        symbol.kind = SymbolKind::Val(val_id);

        // If the symbol that was just examined is a pending symbol AND it was actually resolved,
        // then it'll be marked as resolved
        if let Some(pending_sym) = self.ty_ctx.sym_queue.get_mut(&sym_id)
            && !is_unknown
        {
            // Two caches
            pending_sym.is_resolved = true;
            self.ty_ctx.needs_check = true;
        }

        Ok(())
    }

    fn resolve_typedef(&mut self, abs_typedef: &AbstractTypeDef, ast_id: AstId) -> Result<(), ()> {
        let type_id = self.resolve_type_expr(
            self.current_mod,
            &abs_typedef.spanned_ty_expr,
            ScopeType::Var,
            LookupPattern::NoRestrictions,
        )?;

        let scope_id = self
            .compiler
            .extract_scope_id(ScopeType::Var, self.current_mod);
        let table = &self.compiler.get_scope(scope_id).scope.table;
        let sym_id = table.ast_to_sym[&ast_id];

        let mut conds: Vec<ExprId> = Vec::new();
        for spanned_expr in &abs_typedef.conds {
            //FIX: Scope type is a little wrong here since it's a condition
            let cond_opt = match self.register_expr(
                sym_id,
                spanned_expr,
                None,
                self.current_mod,
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

        // Checking if there are duplicate name ids within the same struct along with resolution
        for (i, field_typedef) in abs_struct.fields.iter().enumerate() {
            let type_id = self.resolve_type_expr(
                self.current_mod,
                &field_typedef.spanned_ty_expr,
                ScopeType::Nest,
                LookupPattern::NoRestrictions,
            )?;

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
                    self.current_mod,
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
                self.current_mod,
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
                let type_id = self.resolve_type_expr(
                    self.current_mod,
                    &spanned_ty_expr,
                    ScopeType::Nest,
                    LookupPattern::NoRestrictions,
                )?;
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
                    self.current_mod,
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
                self.current_mod,
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

            let type_id = self.resolve_type_expr(
                self.current_mod,
                &abs_param.ty_expr,
                ScopeType::Neutral,
                LookupPattern::NoRestrictions,
            )?;

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
                self.current_mod,
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
        active_mod_id: ModuleId,
        scope_type: ScopeType,
        seen: &mut Vec<SymbolId>,
    ) -> Result<ExprId, SemanticError> {
        match &spanned_expr.expr {
            //TODO: Check ownership in constraints
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
                            SymbolKind::Unknown => todo!(),
                        };

                        self.compiler.exprs.push(expr);

                        return Ok(expr_id);
                    }
                }

                if let Some(found_sym_id) = scopes::get_sym_id(
                    self.compiler,
                    active_mod_id,
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

                    let resolved_expr = match symbol.kind {
                        // I don't think this is needed since types are already known
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
                                // Can only be local right now
                                Type::Constrained(ty_constraint) => todo!(),
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
                        SymbolKind::Unknown => {
                            let expr_id = ExprId::new(self.compiler.exprs.len() as u32);
                            let expr_hir = ExprHir::Var(found_sym_id);
                            let pending_expr = PendingExpr::new(expr_id, parent_sym_id);

                            //NOTE: ONLY THIS POINT SHOULD STORE THE SYMBOL. This is how the
                            //connection is made so that, y = x + 2, goes from x -> x + 2 -> y
                            //after x is resolved.
                            self.ty_ctx.store_pending_expr(found_sym_id, pending_expr);
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
                            )
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
                    active_mod_id,
                    scope_type,
                    seen,
                )?;

                let rhs_id = self.register_expr(
                    parent_sym_id,
                    &*rhs,
                    local_scope_id,
                    active_mod_id,
                    scope_type,
                    seen,
                )?;

                let lhs_expr = &self.compiler.exprs[lhs_id.id as usize];
                let rhs_expr = &self.compiler.exprs[rhs_id.id as usize];

                let lhs_is_unknown = lhs_expr.type_id.id == script_compiler::TYPE_UNKNOWN_IDX;
                let rhs_is_unknown = rhs_expr.type_id.id == script_compiler::TYPE_UNKNOWN_IDX;

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
                let type_id = match &const_val_opt {
                    Some(val) => match val {
                        Value::I64(_) => TypeId::new(script_compiler::CORE_I64),
                        Value::F64(_) => TypeId::new(script_compiler::CORE_F64),
                        Value::Bool(_) => TypeId::new(script_compiler::CORE_BOOL),
                        Value::Char(_) => TypeId::new(script_compiler::CORE_BOOL),
                        Value::Func(_) => TypeId::new(script_compiler::TYPE_UNKNOWN_IDX),
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
                        BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mult | BinaryOp::Div => {
                            if lhs_is_unknown && rhs_is_unknown {
                                TypeId::new(script_compiler::TYPE_UNKNOWN_IDX)
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
                        | BinaryOp::Mod
                        | BinaryOp::LessOrEq => TypeId::new(script_compiler::CORE_BOOL),
                        // Bitwise doesn't exist yet
                        BinaryOp::BitOr => todo!(),
                        BinaryOp::BitAnd => todo!(),
                        BinaryOp::BitNot => todo!(),
                        BinaryOp::BitRightShift => todo!(),
                        BinaryOp::BitLeftShift => todo!(),
                        BinaryOp::BitXor => todo!(),
                    },
                };

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
            //WARN: Maybe turn nameid to expr?
            Expr::Default(ident_expr, spanned_expr) => {
                let expr_id = ExprId::new(self.compiler.exprs.len() as u32);
                let val_id = ValueId::new(self.compiler.values.len() as u32);

                //WARN: SUSPICIOUS
                let default_ident_expr_id = self.register_expr(
                    parent_sym_id,
                    &ident_expr,
                    local_scope_id,
                    active_mod_id,
                    scope_type,
                    seen,
                )?;

                let default_val_expr_id = self.register_expr(
                    parent_sym_id,
                    &spanned_expr,
                    local_scope_id,
                    active_mod_id,
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
                    active_mod_id,
                    scope_type,
                    seen,
                )?;

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
                    active_mod_id,
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
                        active_mod_id,
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
            // Maybe having "::" exist could help..
            // FIX: Need to reduce code re-usage since this
            Expr::MemberAccess(abs_member_access) => {
                match self.resolve_member(
                    parent_sym_id,
                    &abs_member_access.base,
                    local_scope_id,
                    active_mod_id,
                    scope_type,
                    seen,
                )? {
                    PossibleMember::Module(extern_mod_id) => {
                        let extern_mod = &self.compiler.mods[extern_mod_id.id];
                        //NOTE: Maybe privacy should be checked from this resolver?
                        if let Some(extern_sym_id) = scopes::get_sym_id(
                            self.compiler,
                            extern_mod_id,
                            abs_member_access.field,
                            scope_type,
                            LookupPattern::ModuleOnly,
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
                                extern_mod_id,
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
        }
    }

    fn resolve_member(
        &mut self,
        sym_parent: SymbolId,
        member: &SpannedExpr,
        local_scope: Option<ScopeId>,
        active_mod_id: ModuleId,
        scope_type: ScopeType,
        seen: &mut Vec<SymbolId>,
    ) -> Result<PossibleMember, SemanticError> {
        if let Ok(expr_id) = self.register_expr(
            sym_parent,
            member,
            local_scope,
            active_mod_id,
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
                self.current_mod,
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
        active_mod_id: ModuleId,
        spanned_ty_expr: &SpannedTypeExpr,
        scope_type: ScopeType,
        lookup_pattern: LookupPattern,
    ) -> Result<TypeId, ()> {
        match &spanned_ty_expr.ty_expr {
            //FIXME: If an error occurs while self.current_mod = extern_mod, it tries to report the
            //error from the external module instead of the actual module of origin.
            TypeExpr::Var(name_id) => {
                // Searching symbols because otherwise, the type of a variable would be valid
                // which is not a favorable allowable syntax
                match scopes::get_sym_id(
                    self.compiler,
                    active_mod_id,
                    *name_id,
                    scope_type,
                    lookup_pattern,
                ) {
                    Some(sym_id) => match self.compiler.symbols[sym_id.id as usize].kind {
                        SymbolKind::Type(type_id) => {
                            // NOTE: Will probably error later in resolution but fine for now
                            let symbol = &self.compiler.symbols[sym_id.id as usize];
                            if symbol.is_priv && symbol.owner != self.current_mod {
                                let active_mod = &self.compiler.mods[active_mod_id.id];
                                let active_name =
                                    self.interner.search(active_mod.name_id.id as usize);
                                let sym_name = self.interner.search(symbol.name_id.id as usize);

                                let msg = format!(
                                    "The type `{sym_name}` is private within module `{active_name}`"
                                );

                                let mod_origin = &self.compiler.mods[self.current_mod.id];
                                self.reporter.report_spanned(
                                    &msg,
                                    None,
                                    &[spanned_ty_expr.span],
                                    &mod_origin
                                        .src_metadata
                                        .as_ref()
                                        .expect("core should not be resolved"),
                                );

                                return Err(());
                            }

                            return Ok(type_id);
                        }
                        SymbolKind::Val(_) | SymbolKind::Unknown => (),
                    },
                    None => (),
                }

                let mod_origin = &self.compiler.mods[self.current_mod.id];

                let active_mod = &self.compiler.mods[active_mod_id.id];
                let active_name = self.interner.search(active_mod.name_id.id as usize);

                // If we have main, that imports def, that imports other, it tries to search for
                // things in the "other" module even though it's defined in "def".
                //
                // Within "def", it tries to search "other" for everything declared even if "other"
                // is never used.
                let err_name = self.interner.search(name_id.id as usize);
                let err_msg =
                    format!("`{err_name}` is not defined as a type within module `{active_name}`");
                println!("{err_msg}");

                self.reporter.report_spanned(
                    &err_msg,
                    Some(err_name),
                    &[spanned_ty_expr.span],
                    &mod_origin
                        .src_metadata
                        .as_ref()
                        .expect("core should not be resolved"),
                );

                Err(())
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

                                let mod_origin = &self.compiler.mods[self.current_mod.id];
                                self.reporter.report_spanned(
                                    &msg,
                                    None,
                                    &[spanned_ty_expr.span],
                                    &mod_origin
                                        .src_metadata
                                        .as_ref()
                                        .expect("core should not be resolved"),
                                );

                                return Err(());
                            }

                            let inner = self.resolve_type_expr(
                                self.current_mod,
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
                                    self.current_mod,
                                    arg,
                                    scope_type,
                                    LookupPattern::NoRestrictions,
                                )?);
                            }

                            let type_id = TypeId::new(self.compiler.types.len() as u32);
                            let tuple = Type::BuiltinType(BuiltinType::Tuple(elements));

                            let ty_info = TypeInfo::new(tuple, self.compiler.core_mod_id);
                            self.compiler.types.push(ty_info);

                            Ok(type_id)
                        }
                        BuiltinTypeKind::Map => {
                            if generic.args.len() != 2 {
                                let msg = format!("Expected only 2 types within `Map`",);

                                let mod_origin = &self.compiler.mods[self.current_mod.id];
                                self.reporter.report_spanned(
                                    &msg,
                                    None,
                                    &[spanned_ty_expr.span],
                                    &mod_origin
                                        .src_metadata
                                        .as_ref()
                                        .expect("core should not be resolved"),
                                );

                                return Err(());
                            }

                            let key = self.resolve_type_expr(
                                self.current_mod,
                                &generic.args[0],
                                scope_type,
                                LookupPattern::NoRestrictions,
                            )?;

                            let val = self.resolve_type_expr(
                                self.current_mod,
                                &generic.args[1],
                                scope_type,
                                LookupPattern::NoRestrictions,
                            )?;

                            let map = Type::BuiltinType(BuiltinType::Map(key, val));
                            let map_id = self.compiler.types.len() as u32;

                            let ty_info = TypeInfo::new(map, self.compiler.core_mod_id);
                            self.compiler.types.push(ty_info);

                            Ok(TypeId::new(map_id))
                        }
                        // I'm sure this can be done better...
                        _ => {
                            let err_name = self.interner.search(generic.base.id as usize);
                            //WARN: Questionablly phrased error message
                            //This COULD change so this will not be upheld at the parsing stage
                            let err_msg = format!(
                                "Found identifier \"{err_name}\" before generic parameters, but only `List`, `Set`, `Map`, and `Tuple` are valid data structures"
                            );

                            let mod_origin = &self.compiler.mods[self.current_mod.id];
                            self.reporter.report_spanned(
                                &err_msg,
                                Some(err_name),
                                &[spanned_ty_expr.span],
                                &mod_origin
                                    .src_metadata
                                    .as_ref()
                                    .expect("core should not be resolved"),
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

                        let mod_origin = &self.compiler.mods[self.current_mod.id];
                        self.reporter.report_spanned(
                            &err_msg,
                            Some(err_name),
                            &[spanned_ty_expr.span],
                            &mod_origin
                                .src_metadata
                                .as_ref()
                                .expect("core should not be resolved"),
                        );

                        Err(())
                    }
                }
            }
            // This only allows something like, defs.Thing which can go to at most one type deep,
            // but no more. Will need change since something like i32.MAX could be "core.i32.MAX".
            //
            // Maybe not though since that would only be usable in expressions anyways which aren't
            // type expressions
            TypeExpr::Path(spanned_ty_exprs) => {
                // The parser disallows < 2 type pathing to actually exist so indexing should be
                // safe here
                if spanned_ty_exprs.len() != 2 {
                    let msg = format!("Only 1 member access can be used for types");

                    let spans: Vec<Span> = spanned_ty_exprs
                        .iter()
                        .skip(1)
                        .map(|expr| expr.span)
                        .collect();

                    let module = &self.compiler.mods[self.current_mod.id];
                    self.reporter.report_spanned(
                        &msg,
                        None,
                        &spans,
                        &module
                            .src_metadata
                            .as_ref()
                            .expect("core should not be resolved"),
                    );
                }

                let extern_mod = match &spanned_ty_exprs[0].ty_expr {
                    TypeExpr::Var(name_id) => {
                        if let Some(mod_id) = self.compiler.mod_map.get(name_id) {
                            &self.compiler.mods[mod_id.id]
                        } else {
                            let err_name = self.interner.search(name_id.id as usize);
                            let msg = format!("The module `{err_name}` does not exist");

                            let module = &self.compiler.mods[self.current_mod.id];
                            self.reporter.report_spanned(
                                &msg,
                                None,
                                &[spanned_ty_exprs[0].span],
                                &module
                                    .src_metadata
                                    .as_ref()
                                    .expect("core should not be resolved"),
                            );

                            return Err(());
                        }
                    }
                    _ => unreachable!("Parser does not pick this up"),
                };

                // Checking if the current module actually has the module found imported.
                //WARN: Using current_mod instead of active_mod here
                let current_module = &self.compiler.mods[self.current_mod.id];

                if !current_module.contains_import(extern_mod) {
                    let extern_name = self.interner.search(extern_mod.name_id.id as usize);
                    let current_name = self.interner.search(current_module.name_id.id as usize);
                    let msg = format!(
                        "The module `{extern_name}` exists but was not imported by `{current_name}`"
                    );

                    self.reporter.report_spanned(
                        &msg,
                        None,
                        &[spanned_ty_exprs[0].span],
                        &current_module
                            .src_metadata
                            .as_ref()
                            .expect("core should not be resolved"),
                    );

                    return Err(());
                };

                self.resolve_type_expr(
                    extern_mod.mod_id,
                    &spanned_ty_exprs[1],
                    scope_type,
                    LookupPattern::ModuleOnly,
                )
            }
        }
    }
}
