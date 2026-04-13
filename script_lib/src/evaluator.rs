use chern_core::values::Value;

use crate::parser::ast::BinaryOp;

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
