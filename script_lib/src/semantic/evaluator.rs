use chern_core::values::Value;

use crate::{parser::ast::BinaryOp, semantic::error::SemanticError};

pub fn eval_vals() -> Value {
    todo!();
}

pub fn is_compatible(lhs: &Value, op: BinaryOp, rhs: &Value) -> bool {
    match lhs {
        Value::I128(_) => match op {
            _ => match rhs {
                Value::I128(_) => true,
                _ => false,
            },
        },
        Value::F64(_) => match op {
            _ => match rhs {
                Value::I128(_) => true,
                _ => false,
            },
        },
        Value::Bool(_) => {
            if op.is_bool_op() && rhs.is_bool() {
                true
            } else {
                false
            }
        }
        // Not right now
        Value::Char(_) => false,
        // Not right now
        Value::InternedStr(_) => false,
        // Not right now
        Value::RuntimeStr(_) => false,
        Value::Tuple(_) => false,
        Value::Unknown => false,
    }
}

/// Applies operation assuming that lhs and rhs were checked for compatibility
pub fn apply_binary_op(lhs: &Value, op: BinaryOp, rhs: &Value) -> Result<Value, SemanticError> {
    match op {
        BinaryOp::Add => {
            todo!();
            if lhs.is_bool() || rhs.is_bool() {}
        }
        BinaryOp::Sub => todo!(),
        BinaryOp::Mult => todo!(),
        BinaryOp::Divide => todo!(),
        BinaryOp::Greater => todo!(),
        BinaryOp::Less => todo!(),
        BinaryOp::GreaterOrEq => todo!(),
        BinaryOp::LessOrEq => todo!(),
        BinaryOp::Mod => todo!(),
        BinaryOp::And => todo!(),
        BinaryOp::Or => todo!(),
        BinaryOp::EqTo => todo!(),
        BinaryOp::NotEq => todo!(),
    }

    todo!();
}
