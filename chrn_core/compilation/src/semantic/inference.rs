use chrn_utils::id_types::TypeId;
use lang::values::Value;

use crate::{
    parser::ast::ast_concepts::BinaryOp,
    script_compiler::{self, ScriptCompiler},
};

pub(crate) fn infer_type_from_val(compiler: &ScriptCompiler, val: &Value) -> Option<TypeId> {
    match val {
        Value::I64(_) => Some(TypeId::new(script_compiler::CORE_I64)),
        Value::F64(_) => Some(TypeId::new(script_compiler::CORE_F64)),
        Value::Bool(_) => Some(TypeId::new(script_compiler::CORE_BOOL)),
        Value::Char(_) => Some(TypeId::new(script_compiler::CORE_CHAR)),
        Value::Func(func_sym) => {
            let func_def = compiler.get_func(*func_sym);
            Some(func_def.ret_type)
        }
        Value::InternedStr(_) => Some(TypeId::new(script_compiler::CORE_STR)),
        Value::Array(elements) => {
            // Would this be possible?
            if elements.is_empty() {
                return None;
            }

            // Recursively calling so the known element re-uses matching logic
            infer_type_from_val(compiler, &elements[0])
        }
        // Both of these are not possible as of right now from an operation
        // since there are no runtime values RIGHT NOW, and unknown is not a comptaible
        // binary op so it can't acually be produced.
        // Tuples also are not used outside of expressing type constraints.
        //
        // Value::RuntimeStr(_) => TypeId::new(script_compiler::CORE_STR),
        // Value::Tuple(_) => TypeId::new(script_compiler::CORE_TUPLE),
        // Value::Unknown => TypeId::new(script_compiler::TYPE_UNKNOWN_IDX),
        Value::Tuple(_) | Value::RuntimeStr(_) => unreachable!(),
        Value::Unknown => None,
    }
}

pub(crate) fn infer_type_from_binary_op(
    lhs_type_id: TypeId,
    rhs_type_id: TypeId,
    lhs_is_unknown: bool,
    op: BinaryOp,
    rhs_is_unknown: bool,
) -> Option<TypeId> {
    match op {
        // Maybe this can still point to unknown?
        BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mult | BinaryOp::Div | BinaryOp::Mod => {
            if lhs_is_unknown && rhs_is_unknown {
                None
            } else if rhs_is_unknown {
                Some(lhs_type_id)
            } else {
                Some(rhs_type_id)
            }
        }
        BinaryOp::Greater
        | BinaryOp::Less
        | BinaryOp::GreaterOrEq
        | BinaryOp::And
        | BinaryOp::Or
        | BinaryOp::EqTo
        | BinaryOp::NotEq
        | BinaryOp::LessOrEq => Some(TypeId::new(script_compiler::CORE_BOOL)),
        // Bitwise doesn't exist yet
        //WARN: Endo
        BinaryOp::BitOr
        | BinaryOp::BitAnd
        | BinaryOp::BitRightShift
        | BinaryOp::BitLeftShift
        | BinaryOp::BitXor => Some(TypeId::new(script_compiler::CORE_I64)),
    }
}
