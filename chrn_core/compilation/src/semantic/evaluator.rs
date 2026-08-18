use chrn_utils::{intern::Intern, utils::containers::SpannedContainerRef};
use lang::values::Value;

use crate::parser::ast::ast_concepts::{BinaryOp, UnaryOp};

pub enum UnaryOpResult {
    Output(Value),
    Invalid,
}

pub enum BinaryOpResult {
    Output(Value),
    Invalid,
    DivideByZero,
}

// Is this the type checker's?
/// Evaluates if the given unary operation is possible given language rules
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
            | Value::Func(_)
            | Value::Array(_)
            | Value::Unknown => false,
        },
        UnaryOp::Negate => match operand {
            Value::I64(_) | Value::F64(_) => true,
            _ => false,
        },
        UnaryOp::BitNot => match operand {
            Value::I64(_) => true,
            _ => false,
        },
    }
}

/// Evaluates if the given binary operation is possible given language rules
///
/// Mirrors `apply_binary_op`: an operand pair accepted here has a matching arm there, and one
/// rejected here has none. Both sides must be updated together.
pub fn is_compatible_binary(lhs: &Value, op: BinaryOp, rhs: &Value) -> bool {
    if let Value::RuntimeStr(_) = lhs {
        unreachable!("Impossible to reach at compile time")
    }

    match op {
        BinaryOp::Add => matches!(
            (lhs, rhs),
            (Value::I64(_), Value::I64(_))
                | (Value::F64(_), Value::F64(_))
                | (Value::InternedStr(_), Value::InternedStr(_))
        ),
        BinaryOp::Sub | BinaryOp::Mult | BinaryOp::Div | BinaryOp::Mod => matches!(
            (lhs, rhs),
            (Value::I64(_), Value::I64(_)) | (Value::F64(_), Value::F64(_))
        ),
        BinaryOp::Greater | BinaryOp::Less | BinaryOp::GreaterOrEq | BinaryOp::LessOrEq => {
            matches!(
                (lhs, rhs),
                (Value::I64(_), Value::I64(_))
                    | (Value::F64(_), Value::F64(_))
                    | (Value::Char(_), Value::Char(_))
                    | (Value::InternedStr(_), Value::InternedStr(_))
            )
        }
        BinaryOp::And | BinaryOp::Or => matches!((lhs, rhs), (Value::Bool(_), Value::Bool(_))),
        BinaryOp::EqTo | BinaryOp::NotEq => matches!(
            (lhs, rhs),
            (Value::I64(_), Value::I64(_))
                | (Value::F64(_), Value::F64(_))
                | (Value::Bool(_), Value::Bool(_))
                | (Value::Char(_), Value::Char(_))
                | (Value::InternedStr(_), Value::InternedStr(_))
        ),
        BinaryOp::BitOr
        | BinaryOp::BitAnd
        | BinaryOp::BitRightShift
        | BinaryOp::BitLeftShift
        | BinaryOp::BitXor => matches!((lhs, rhs), (Value::I64(_), Value::I64(_))),
    }
}

pub fn apply_unary_op(op: UnaryOp, sp_operand: SpannedContainerRef<Value>) -> UnaryOpResult {
    let operand = sp_operand.inner;

    let res = match op {
        UnaryOp::Not => match operand {
            Value::Bool(v) => Some(Value::Bool(!v)),
            _ => None,
        },
        UnaryOp::Negate => match operand {
            Value::I64(v) => Some(Value::I64(-v)),
            Value::F64(v) => Some(Value::F64(-v)),
            _ => None,
        },
        UnaryOp::BitNot => match operand {
            Value::I64(v) => Some(Value::I64(!v)),
            _ => None,
        },
    };

    match res {
        Some(val) => UnaryOpResult::Output(val),
        None => UnaryOpResult::Invalid,
    }
}

/// Applies operation assuming that lhs and rhs were checked for compatibility
//TODO: BIGFLOAT
pub fn apply_binary_op(
    sp_lhs: SpannedContainerRef<Value>,
    op: BinaryOp,
    sp_rhs: SpannedContainerRef<Value>,
    interner: &mut Intern,
) -> BinaryOpResult {
    let lhs = sp_lhs.inner;
    let rhs = sp_rhs.inner;

    //TODO: Avoid overflow/underflow
    let res = match op {
        BinaryOp::Add => match lhs {
            Value::I64(lhs_inner) => match rhs {
                Value::I64(rhs_inner) => Some(Value::I64(lhs_inner + rhs_inner)),
                _ => None,
            },
            Value::F64(lhs_inner) => match rhs {
                Value::F64(rhs_inner) => Some(Value::F64(lhs_inner + rhs_inner)),
                _ => None,
            },
            Value::InternedStr(lhs_inner) => match rhs {
                Value::InternedStr(rhs_inner) => {
                    let l_str = interner.search(*lhs_inner);
                    let r_str = interner.search(*rhs_inner);
                    let new_str = l_str.to_string() + r_str;
                    let new_interned_id = interner.intern(&new_str);
                    Some(Value::InternedStr(new_interned_id))
                }
                _ => None,
            },
            _ => None,
        },
        BinaryOp::Sub => match lhs {
            Value::I64(lhs_inner) => match rhs {
                Value::I64(rhs_inner) => Some(Value::I64(lhs_inner - rhs_inner)),
                _ => None,
            },
            Value::F64(lhs_inner) => match rhs {
                Value::F64(rhs_inner) => Some(Value::F64(lhs_inner - rhs_inner)),
                _ => None,
            },
            _ => None,
        },
        BinaryOp::Mult => match lhs {
            Value::I64(lhs_inner) => match rhs {
                Value::I64(rhs_inner) => Some(Value::I64(lhs_inner * rhs_inner)),
                _ => None,
            },
            Value::F64(lhs_inner) => match rhs {
                Value::F64(rhs_inner) => Some(Value::F64(lhs_inner * rhs_inner)),
                _ => None,
            },
            _ => None,
        },
        BinaryOp::Div => match lhs {
            Value::I64(lhs_inner) => match rhs {
                Value::I64(rhs_inner) => {
                    if *rhs_inner == 0 {
                        return BinaryOpResult::DivideByZero;
                    }

                    Some(Value::I64(lhs_inner / rhs_inner))
                }
                _ => None,
            },
            Value::F64(lhs_inner) => match rhs {
                Value::F64(rhs_inner) => {
                    if *rhs_inner == 0.0 {
                        return BinaryOpResult::DivideByZero;
                    }

                    Some(Value::F64(lhs_inner / rhs_inner))
                }
                _ => None,
            },
            _ => None,
        },
        BinaryOp::Greater => match lhs {
            Value::I64(lhs_inner) => match rhs {
                Value::I64(rhs_inner) => Some(Value::Bool(lhs_inner > rhs_inner)),
                _ => None,
            },
            Value::F64(lhs_inner) => match rhs {
                Value::F64(rhs_inner) => Some(Value::Bool(lhs_inner > rhs_inner)),
                _ => None,
            },
            Value::Char(lhs_inner) => match rhs {
                Value::Char(rhs_inner) => Some(Value::Bool(lhs_inner > rhs_inner)),
                _ => None,
            },
            Value::InternedStr(lhs_inner) => match rhs {
                Value::InternedStr(rhs_inner) => {
                    let l_str = interner.search(*lhs_inner);
                    let r_str = interner.search(*rhs_inner);
                    Some(Value::Bool(l_str > r_str))
                }
                _ => None,
            },
            _ => None,
        },
        BinaryOp::Less => match lhs {
            Value::I64(lhs_inner) => match rhs {
                Value::I64(rhs_inner) => Some(Value::Bool(lhs_inner < rhs_inner)),
                _ => None,
            },
            Value::F64(lhs_inner) => match rhs {
                Value::F64(rhs_inner) => Some(Value::Bool(lhs_inner < rhs_inner)),
                _ => None,
            },
            Value::Char(lhs_inner) => match rhs {
                Value::Char(rhs_inner) => Some(Value::Bool(lhs_inner < rhs_inner)),
                _ => None,
            },
            Value::InternedStr(lhs_inner) => match rhs {
                Value::InternedStr(rhs_inner) => {
                    let l_str = interner.search(*lhs_inner);
                    let r_str = interner.search(*rhs_inner);
                    Some(Value::Bool(l_str < r_str))
                }
                _ => None,
            },
            _ => None,
        },
        BinaryOp::GreaterOrEq => match lhs {
            Value::I64(lhs_inner) => match rhs {
                Value::I64(rhs_inner) => Some(Value::Bool(lhs_inner >= rhs_inner)),
                _ => None,
            },
            Value::F64(lhs_inner) => match rhs {
                Value::F64(rhs_inner) => Some(Value::Bool(lhs_inner >= rhs_inner)),
                _ => None,
            },
            Value::Char(lhs_inner) => match rhs {
                Value::Char(rhs_inner) => Some(Value::Bool(lhs_inner >= rhs_inner)),
                _ => None,
            },
            Value::InternedStr(lhs_inner) => match rhs {
                Value::InternedStr(rhs_inner) => {
                    let l_str = interner.search(*lhs_inner);
                    let r_str = interner.search(*rhs_inner);
                    Some(Value::Bool(l_str >= r_str))
                }
                _ => None,
            },
            _ => None,
        },
        BinaryOp::LessOrEq => match lhs {
            Value::I64(lhs_inner) => match rhs {
                Value::I64(rhs_inner) => Some(Value::Bool(lhs_inner <= rhs_inner)),
                _ => None,
            },
            Value::F64(lhs_inner) => match rhs {
                Value::F64(rhs_inner) => Some(Value::Bool(lhs_inner <= rhs_inner)),
                _ => None,
            },
            Value::Char(lhs_inner) => match rhs {
                Value::Char(rhs_inner) => Some(Value::Bool(lhs_inner <= rhs_inner)),
                _ => None,
            },
            Value::InternedStr(lhs_inner) => match rhs {
                Value::InternedStr(rhs_inner) => {
                    let l_str = interner.search(*lhs_inner);
                    let r_str = interner.search(*rhs_inner);
                    Some(Value::Bool(l_str <= r_str))
                }
                _ => None,
            },
            _ => None,
        },
        BinaryOp::Mod => match lhs {
            Value::I64(lhs_inner) => match rhs {
                Value::I64(rhs_inner) => Some(Value::I64(lhs_inner % rhs_inner)),
                _ => None,
            },
            Value::F64(lhs_inner) => match rhs {
                Value::F64(rhs_inner) => Some(Value::F64(lhs_inner % rhs_inner)),
                _ => None,
            },
            _ => None,
        },
        BinaryOp::And => match lhs {
            Value::Bool(lhs_inner) => match rhs {
                Value::Bool(rhs_inner) => Some(Value::Bool(*lhs_inner && *rhs_inner)),
                _ => None,
            },
            _ => None,
        },
        BinaryOp::Or => match lhs {
            Value::Bool(lhs_inner) => match rhs {
                Value::Bool(rhs_inner) => Some(Value::Bool(*lhs_inner || *rhs_inner)),
                _ => None,
            },
            _ => None,
        },
        BinaryOp::EqTo => match lhs {
            Value::I64(lhs_inner) => match rhs {
                Value::I64(rhs_inner) => Some(Value::Bool(lhs_inner == rhs_inner)),
                _ => None,
            },
            Value::F64(lhs_inner) => match rhs {
                Value::F64(rhs_inner) => Some(Value::Bool(lhs_inner == rhs_inner)),
                _ => None,
            },
            Value::Bool(lhs_inner) => match rhs {
                Value::Bool(rhs_inner) => Some(Value::Bool(lhs_inner == rhs_inner)),
                _ => None,
            },
            Value::Char(lhs_inner) => match rhs {
                Value::Char(rhs_inner) => Some(Value::Bool(lhs_inner == rhs_inner)),
                _ => None,
            },
            Value::InternedStr(lhs_inner) => match rhs {
                Value::InternedStr(rhs_inner) => Some(Value::Bool(lhs_inner == rhs_inner)),
                _ => None,
            },
            _ => None,
        },
        BinaryOp::NotEq => match lhs {
            Value::I64(lhs_inner) => match rhs {
                Value::I64(rhs_inner) => Some(Value::Bool(lhs_inner != rhs_inner)),
                _ => None,
            },
            Value::F64(lhs_inner) => match rhs {
                Value::F64(rhs_inner) => Some(Value::Bool(lhs_inner != rhs_inner)),
                _ => None,
            },
            Value::Bool(lhs_inner) => match rhs {
                Value::Bool(rhs_inner) => Some(Value::Bool(lhs_inner != rhs_inner)),
                _ => None,
            },
            Value::Char(lhs_inner) => match rhs {
                Value::Char(rhs_inner) => Some(Value::Bool(lhs_inner != rhs_inner)),
                _ => None,
            },
            Value::InternedStr(lhs_inner) => match rhs {
                Value::InternedStr(rhs_inner) => Some(Value::Bool(lhs_inner != rhs_inner)),
                _ => None,
            },
            _ => None,
        },
        BinaryOp::BitOr => match lhs {
            Value::I64(lhs_inner) => match rhs {
                Value::I64(rhs_inner) => Some(Value::I64(lhs_inner | rhs_inner)),
                _ => None,
            },
            _ => None,
        },
        BinaryOp::BitAnd => match lhs {
            Value::I64(lhs_inner) => match rhs {
                Value::I64(rhs_inner) => Some(Value::I64(lhs_inner & rhs_inner)),
                _ => None,
            },
            _ => None,
        },
        BinaryOp::BitRightShift => match lhs {
            Value::I64(lhs_inner) => match rhs {
                Value::I64(rhs_inner) => Some(Value::I64(lhs_inner >> rhs_inner)),
                _ => None,
            },
            _ => None,
        },
        BinaryOp::BitLeftShift => match lhs {
            Value::I64(lhs_inner) => match rhs {
                Value::I64(rhs_inner) => Some(Value::I64(lhs_inner << rhs_inner)),
                _ => None,
            },
            _ => None,
        },
        BinaryOp::BitXor => match lhs {
            Value::I64(lhs_inner) => match rhs {
                Value::I64(rhs_inner) => Some(Value::I64(lhs_inner ^ rhs_inner)),
                _ => None,
            },
            _ => None,
        },
    };

    match res {
        Some(val) => BinaryOpResult::Output(val),
        None => BinaryOpResult::Invalid,
    }
}
