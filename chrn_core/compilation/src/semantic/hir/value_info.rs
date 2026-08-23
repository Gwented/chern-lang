// TODO: Should probably be in compilation
// Was about to say this again..
// And again

use chrn_utils::id_types::{ExprId, TypeId};
use lang::values::Value;

// This is supposed to represent something like, let x = 4, where 4 may or may not have a constant
// value, 4 is the expression, and it's type is whatever is inferred
/// Metadata over `Value`
#[derive(Debug, Clone)]
pub struct ValueInfo {
    pub type_id: TypeId,
    pub expr_id: ExprId,
    pub const_val: Option<Value>,
}

impl ValueInfo {
    pub fn new(type_id: TypeId, expr_id: ExprId, const_val: Option<Value>) -> ValueInfo {
        ValueInfo {
            type_id,
            expr_id,
            const_val,
        }
    }
}
