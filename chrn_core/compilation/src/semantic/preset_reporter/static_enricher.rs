// Naming is a bit confusing
//! Adds help and notes to a `SourceDiagnosticBuilder` in a composable manner that allows for
//! specific checks. Is "static" because it's based off static information rather than the dynamic
//! collection of data `engine.rs` participates in.
//!
//! Example: If we have " "che" + "rn" ", it can say that string concatenation is not available
//! rather than emit a basic error that says it can't be applied.

use chrn_utils::source_map::source_diagnostic::SourceDiagnosticBuilder;
use lang::values::ValueKind;

use crate::parser::ast::ast_concepts::{BinaryOp, UnaryOp};

// These seem a bit intrunsive..
// Agent or not one could easily infer that not being applied is only usable for bool, along with
// many of the other notes

pub(super) fn enrich_binary_op(
    builder: SourceDiagnosticBuilder,
    lhs: ValueKind,
    op: BinaryOp,
    rhs: ValueKind,
) -> SourceDiagnosticBuilder {
    match (lhs, op, rhs) {
        //TODO: Still make a function for this
        // (ValueKind::InternedStr, BinaryOp::Add, ValueKind::InternedStr) => {
        //     builder.add_note("String concatenation is not supported".into())
        // }
        //TODO: make a function
        (ValueKind::InternedStr, BinaryOp::Mult, ValueKind::InternedStr) => {
            builder.add_note("String repetition is not supported")
        }
        (ValueKind::Bool, _, _) | (_, _, ValueKind::Bool) if op.is_arithmetic_op() => {
            builder.add_note("`bool` is not an integer internally")
        }
        _ => builder,
    }
}

pub(super) fn enrich_unary_op(
    builder: SourceDiagnosticBuilder,
    op: UnaryOp,
    operand: ValueKind,
) -> SourceDiagnosticBuilder {
    match (op, operand) {
        (UnaryOp::Not, ValueKind::I64) => {
            builder.add_note("If this was intended to be `BITNOT` use `~` 🦀")
        }
        _ => builder,
    }
}
