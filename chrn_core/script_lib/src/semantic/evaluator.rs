use chrn_utils::values::Value;

use crate::{
    parser::ast::{BinaryOp, UnaryOp},
    semantic::error::SemanticError,
};

pub fn is_compatible_unary(op: UnaryOp, operand: &Value) -> bool {
    match op {
        UnaryOp::Not => match operand {
            Value::Bool(_) => true,
            Value::I64(_)
            | Value::F64(_)
            | Value::Char(_)
            | Value::Tuple(_)
            | Value::InternedStr(_)
            | Value::RuntimeStr(_)
            | Value::Unknown => false,
        },
        UnaryOp::Negate => match operand {
            Value::I64(_) | Value::F64(_) => true,
            _ => false,
        },
    }
}

pub fn is_compatible_binary(lhs: &Value, op: BinaryOp, rhs: &Value) -> bool {
    match lhs {
        Value::I64(_) => match op {
            BinaryOp::Add
            | BinaryOp::Sub
            | BinaryOp::Mult
            | BinaryOp::Div
            | BinaryOp::Greater
            | BinaryOp::Less
            | BinaryOp::GreaterOrEq
            | BinaryOp::LessOrEq
            | BinaryOp::Mod
            | BinaryOp::EqTo
            | BinaryOp::BitOr
            | BinaryOp::BitAnd
            | BinaryOp::BitNot
            | BinaryOp::BitRightShift
            | BinaryOp::BitLeftShift
            | BinaryOp::BitXor
            | BinaryOp::NotEq => match rhs {
                Value::I64(_) => true,
                _ => false,
            },
            BinaryOp::And | BinaryOp::Or => false,
        },
        Value::F64(_) => match op {
            BinaryOp::Add
            | BinaryOp::Sub
            | BinaryOp::Mult
            | BinaryOp::Div
            | BinaryOp::Greater
            | BinaryOp::Less
            | BinaryOp::GreaterOrEq
            | BinaryOp::LessOrEq
            | BinaryOp::Mod
            | BinaryOp::EqTo
            | BinaryOp::NotEq => match rhs {
                Value::F64(_) => true,
                _ => false,
            },
            _ => false,
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
        Value::InternedStr(_) => match op {
            BinaryOp::EqTo => match rhs {
                Value::InternedStr(_) => true,
                _ => false,
            },
            BinaryOp::NotEq => match rhs {
                Value::InternedStr(_) => true,
                _ => false,
            },
            // Not right now
            // BinaryOp::Add => todo!(),
            // BinaryOp::Greater => todo!(),
            // BinaryOp::Less => todo!(),
            // BinaryOp::GreaterOrEq => todo!(),
            // BinaryOp::LessOrEq => todo!(),
            _ => false,
        },
        Value::Tuple(_) | Value::Unknown => false,
        // Only semantic has access to these functions due a HIR being used for serial as opposed
        // to Expr so this compile time step cannot touch runtime
        Value::RuntimeStr(_) => unreachable!("Impossible at compile time"),
    }
}

pub fn apply_unary_op(op: UnaryOp, operand: &Value) -> Result<Value, SemanticError> {
    match op {
        UnaryOp::Not => match operand {
            Value::Bool(v) => Ok(Value::Bool(!v)),
            Value::F64(_)
            | Value::I64(_)
            | Value::Char(_)
            | Value::Tuple(_)
            | Value::InternedStr(_)
            | Value::RuntimeStr(_)
            | Value::Unknown => unreachable!(),
        },
        UnaryOp::Negate => match operand {
            Value::I64(v) => Ok(Value::I64(-v)),
            Value::F64(v) => Ok(Value::F64(-v)),
            _ => unreachable!(),
        },
    }
}

// Not my best work
/// Applies operation assuming that lhs and rhs were checked for compatibility
//TODO: BIGFLOAT
//Maybe have all error handling happen here?
pub fn apply_binary_op(lhs: &Value, op: BinaryOp, rhs: &Value) -> Result<Value, SemanticError> {
    match op {
        BinaryOp::Add => match lhs {
            Value::I64(lhs_inner) => match rhs {
                Value::I64(rhs_inner) => Ok(Value::I64(lhs_inner + rhs_inner)),
                _ => unreachable!(),
            },
            Value::F64(lhs_inner) => match rhs {
                Value::F64(rhs_inner) => Ok(Value::F64(lhs_inner + rhs_inner)),
                _ => unreachable!(),
            },
            // In case this is forgotten to be updated
            Value::Bool(_)
            | Value::Char(_)
            | Value::Tuple(_)
            | Value::InternedStr(_)
            | Value::RuntimeStr(_)
            | Value::Unknown => unreachable!(),
        },
        BinaryOp::Sub => match lhs {
            Value::I64(lhs_inner) => match rhs {
                Value::I64(rhs_inner) => Ok(Value::I64(lhs_inner - rhs_inner)),
                _ => unreachable!(),
            },
            Value::F64(lhs_inner) => match rhs {
                Value::F64(rhs_inner) => Ok(Value::F64(lhs_inner - rhs_inner)),
                _ => unreachable!(),
            },
            _ => unreachable!(),
        },
        BinaryOp::Mult => match lhs {
            Value::I64(lhs_inner) => match rhs {
                Value::I64(rhs_inner) => Ok(Value::I64(lhs_inner * rhs_inner)),
                _ => unreachable!(),
            },
            Value::F64(lhs_inner) => match rhs {
                Value::F64(rhs_inner) => Ok(Value::F64(lhs_inner * rhs_inner)),
                _ => unreachable!(),
            },
            _ => unreachable!(),
        },
        BinaryOp::Div => match lhs {
            Value::I64(lhs_inner) => match rhs {
                Value::I64(rhs_inner) => {
                    if *rhs_inner == 0 {
                        todo!("Center a div");
                    }

                    Ok(Value::I64(lhs_inner / rhs_inner))
                }
                _ => unreachable!(),
            },
            Value::F64(lhs_inner) => match rhs {
                Value::F64(rhs_inner) => {
                    // No span
                    if *rhs_inner == 0.0 {
                        panic!("Center a div");
                    }

                    Ok(Value::F64(lhs_inner / rhs_inner))
                }
                _ => unreachable!(),
            },
            _ => unreachable!(),
        },
        BinaryOp::Greater => match lhs {
            Value::I64(lhs_inner) => match rhs {
                Value::I64(rhs_inner) => Ok(Value::Bool(lhs_inner > rhs_inner)),
                _ => unreachable!(),
            },
            Value::F64(lhs_inner) => match rhs {
                Value::F64(rhs_inner) => Ok(Value::Bool(lhs_inner > rhs_inner)),
                _ => unreachable!(),
            },
            Value::Char(lhs_inner) => match rhs {
                Value::Char(rhs_inner) => Ok(Value::Bool(lhs_inner > rhs_inner)),
                _ => unreachable!(),
            },
            // Value::CompileStr(name_id) => todo!(),
            // Value::RuntimeStr(_) => todo!(),
            _ => unreachable!(),
        },
        BinaryOp::Less => match lhs {
            Value::I64(lhs_inner) => match rhs {
                Value::I64(rhs_inner) => Ok(Value::Bool(lhs_inner < rhs_inner)),
                _ => unreachable!(),
            },
            Value::F64(lhs_inner) => match rhs {
                Value::F64(rhs_inner) => Ok(Value::Bool(lhs_inner < rhs_inner)),
                _ => unreachable!(),
            },
            Value::Char(lhs_inner) => match rhs {
                Value::Char(rhs_inner) => Ok(Value::Bool(lhs_inner < rhs_inner)),
                _ => unreachable!(),
            },
            _ => unreachable!(),
        },
        BinaryOp::GreaterOrEq => match lhs {
            Value::I64(lhs_inner) => match rhs {
                Value::I64(rhs_inner) => Ok(Value::Bool(lhs_inner >= rhs_inner)),
                _ => unreachable!(),
            },
            Value::F64(lhs_inner) => match rhs {
                Value::F64(rhs_inner) => Ok(Value::Bool(lhs_inner >= rhs_inner)),
                _ => unreachable!(),
            },
            Value::Char(lhs_inner) => match rhs {
                Value::Char(rhs_inner) => Ok(Value::Bool(lhs_inner >= rhs_inner)),
                _ => unreachable!(),
            },
            _ => unreachable!(),
        },
        BinaryOp::LessOrEq => match lhs {
            Value::I64(lhs_inner) => match rhs {
                Value::I64(rhs_inner) => Ok(Value::Bool(lhs_inner <= rhs_inner)),
                _ => unreachable!(),
            },
            Value::F64(lhs_inner) => match rhs {
                Value::F64(rhs_inner) => Ok(Value::Bool(lhs_inner <= rhs_inner)),
                _ => unreachable!(),
            },
            Value::Char(lhs_inner) => match rhs {
                Value::Char(rhs_inner) => Ok(Value::Bool(lhs_inner <= rhs_inner)),
                _ => unreachable!(),
            },
            _ => unreachable!(),
        },
        BinaryOp::Mod => match lhs {
            Value::I64(lhs_inner) => match rhs {
                Value::I64(rhs_inner) => Ok(Value::I64(lhs_inner % rhs_inner)),
                _ => unreachable!(),
            },
            Value::F64(lhs_inner) => match rhs {
                Value::F64(rhs_inner) => Ok(Value::F64(lhs_inner % rhs_inner)),
                _ => unreachable!(),
            },
            _ => unreachable!(),
        },
        BinaryOp::And => match lhs {
            Value::Bool(lhs_inner) => match rhs {
                Value::Bool(rhs_inner) => Ok(Value::Bool(*lhs_inner && *rhs_inner)),
                _ => unreachable!(),
            },
            _ => unreachable!(),
        },
        BinaryOp::Or => match lhs {
            Value::Bool(lhs_inner) => match rhs {
                Value::Bool(rhs_inner) => Ok(Value::Bool(*lhs_inner || *rhs_inner)),
                _ => unreachable!(),
            },
            _ => unreachable!(),
        },
        BinaryOp::EqTo => match lhs {
            Value::I64(lhs_inner) => match rhs {
                Value::I64(rhs_inner) => Ok(Value::Bool(lhs_inner == rhs_inner)),
                _ => unreachable!(),
            },
            Value::F64(lhs_inner) => match rhs {
                Value::F64(rhs_inner) => Ok(Value::Bool(lhs_inner == rhs_inner)),
                _ => unreachable!(),
            },
            Value::Bool(lhs_inner) => match rhs {
                Value::Bool(rhs_inner) => Ok(Value::Bool(lhs_inner == rhs_inner)),
                // Value::InternedStr(rhs_inner) => {
                //     if rhs_inner.id == Keyword::True as u32 {
                //         return Ok(Value::Bool(*lhs_inner == true));
                //     } else if rhs_inner.id == Keyword::False as u32 {
                //         return Ok(Value::Bool(*lhs_inner == false));
                //     }
                //
                //     unreachable!()
                // }
                _ => unreachable!(),
            },
            Value::Char(lhs_inner) => match rhs {
                Value::Char(rhs_inner) => Ok(Value::Bool(lhs_inner == rhs_inner)),
                _ => unreachable!(),
            },
            // Value::InternedStr(lhs_inner) => match rhs {
            //     Value::InternedStr(rhs_inner) => Ok(Value::Bool(lhs_inner == rhs_inner)),
            //     Value::Bool(rhs_inner) => {
            //         if lhs_inner.id == Keyword::True as u32 {
            //             return Ok(Value::Bool(true == *rhs_inner));
            //         } else if lhs_inner.id == Keyword::False as u32 {
            //             return Ok(Value::Bool(false == *rhs_inner));
            //         }
            //
            //         unreachable!()
            //     }
            //     _ => unreachable!(),
            // },
            _ => unreachable!(),
        },
        BinaryOp::NotEq => match lhs {
            Value::I64(lhs_inner) => match rhs {
                Value::I64(rhs_inner) => Ok(Value::Bool(lhs_inner != rhs_inner)),
                _ => unreachable!(),
            },
            Value::F64(lhs_inner) => match rhs {
                Value::F64(rhs_inner) => Ok(Value::Bool(lhs_inner != rhs_inner)),
                _ => unreachable!(),
            },
            Value::Bool(lhs_inner) => match rhs {
                Value::Bool(rhs_inner) => Ok(Value::Bool(lhs_inner != rhs_inner)),
                // Value::InternedStr(rhs_inner) => {
                //     if rhs_inner.id == Keyword::True as u32 {
                //         return Ok(Value::Bool(*lhs_inner != true));
                //     } else if rhs_inner.id == Keyword::False as u32 {
                //         return Ok(Value::Bool(*lhs_inner != false));
                //     }
                //
                //     unreachable!()
                // }
                _ => unreachable!(),
            },
            Value::Char(lhs_inner) => match rhs {
                Value::Char(rhs_inner) => Ok(Value::Bool(lhs_inner != rhs_inner)),
                _ => unreachable!(),
            },
            Value::InternedStr(lhs_inner) => match rhs {
                Value::InternedStr(rhs_inner) => Ok(Value::Bool(lhs_inner != rhs_inner)),
                // Value::Bool(rhs_inner) => {
                //     if lhs_inner.id == Keyword::True as u32 {
                //         return Ok(Value::Bool(true != *rhs_inner));
                //     } else if lhs_inner.id == Keyword::False as u32 {
                //         return Ok(Value::Bool(false != *rhs_inner));
                //     }
                //
                //     unreachable!()
                // }
                _ => unreachable!(),
            },
            _ => unreachable!(),
        },
        BinaryOp::BitOr => todo!(),
        BinaryOp::BitAnd => todo!(),
        BinaryOp::BitNot => todo!(),
        BinaryOp::BitRightShift => todo!(),
        BinaryOp::BitLeftShift => todo!(),
        BinaryOp::BitXor => todo!(),
    }
}

// pub fn apply_binary_op(lhs: &Value, op: BinaryOp, rhs: &Value, spans: Vec<Span>) -> Result<Value, SemanticError> {
//     todo!();
// }
