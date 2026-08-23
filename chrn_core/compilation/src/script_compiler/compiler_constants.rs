use chrn_utils::id_types::DirectiveId;
use lang::{
    directives::{Directive, TypeDirective},
    types::builtins::{BuiltinType, BuiltinTypeKind},
};

// -- CORE TYPE CONSTANTS --
//NOTE: I think these can be removed. Maybe. I don't know actually.
//No they can't. But we should put this elsewhere. core_constants
pub const CORE_I8: u32 = 0;
pub const CORE_U8: u32 = 1;
pub const CORE_I16: u32 = 2;
pub const CORE_U16: u32 = 3;
pub const CORE_F16: u32 = 4;
pub const CORE_I32: u32 = 5;
pub const CORE_U32: u32 = 6;
pub const CORE_F32: u32 = 7;
pub const CORE_I64: u32 = 8;
pub const CORE_U64: u32 = 9;
pub const CORE_F64: u32 = 10;
pub const CORE_I128: u32 = 11;
pub const CORE_U128: u32 = 12;
pub const CORE_F128: u32 = 13;
pub const CORE_SIZED: u32 = 14;
pub const CORE_UNSIZED: u32 = 15;
pub const CORE_STR: u32 = 16;
pub const CORE_CHAR: u32 = 17;
pub const CORE_NIL: u32 = 18;
pub const CORE_BOOL: u32 = 19;
pub const CORE_BIGINT: u32 = 20;
pub const CORE_BIGFLOAT: u32 = 21;
pub const CORE_RUNTIME: u32 = 22;
// This particular type has no identifier because it's not a real type beyond being a signifier.
pub const CORE_UNKNOWN: u32 = 23;
// pub const CORE_CHARACTER_MAPPABLE: u32 = 24;

/// Converts built-ins with a compile-time id to `u32`
pub const fn builtin_ty_to_id(ty: BuiltinTypeKind) -> u32 {
    match ty {
        BuiltinTypeKind::I8 => CORE_I8,
        BuiltinTypeKind::U8 => CORE_U8,
        BuiltinTypeKind::I16 => CORE_I16,
        BuiltinTypeKind::U16 => CORE_U16,
        BuiltinTypeKind::F16 => CORE_F16,
        BuiltinTypeKind::I32 => CORE_I32,
        BuiltinTypeKind::U32 => CORE_U32,
        BuiltinTypeKind::F32 => CORE_F32,
        BuiltinTypeKind::I64 => CORE_I64,
        BuiltinTypeKind::U64 => CORE_U64,
        BuiltinTypeKind::F64 => CORE_F64,
        BuiltinTypeKind::I128 => CORE_I128,
        BuiltinTypeKind::U128 => CORE_U128,
        BuiltinTypeKind::F128 => CORE_F128,
        BuiltinTypeKind::Sized => CORE_SIZED,
        BuiltinTypeKind::Unsized => CORE_UNSIZED,
        BuiltinTypeKind::Bool => CORE_BOOL,
        BuiltinTypeKind::Nil => CORE_NIL,
        BuiltinTypeKind::Char => CORE_CHAR,
        BuiltinTypeKind::Str => CORE_STR,
        BuiltinTypeKind::BigInt => CORE_BIGINT,
        BuiltinTypeKind::BigFloat => CORE_BIGFLOAT,
        BuiltinTypeKind::Runtime => CORE_RUNTIME,
        BuiltinTypeKind::List
        | BuiltinTypeKind::Set
        | BuiltinTypeKind::Map
        | BuiltinTypeKind::Tuple => unreachable!(),
    }
}

// --  DIRECTIVE CONSTANTS --

pub const DIRECTIVE_WARN_IDX: usize = 0;
pub const DIRECTIVE_IGNORE_IDX: usize = 1;
pub const DIRECTIVE_SCIENT_IDX: usize = 2;
pub const DIRECTIVE_HEX_IDX: usize = 3;
pub const DIRECTIVE_BIN_IDX: usize = 4;
pub const DIRECTIVE_OCTAL_IDX: usize = 5;
pub const DIRECTIVE_UNICODE_IDX: usize = 6;

/// Maps given directive to it's built-in `DirectiveId`
pub fn directive_to_id(directive: &Directive) -> DirectiveId {
    let idx = match directive {
        Directive::Warn => DIRECTIVE_WARN_IDX,
        Directive::Ignore => DIRECTIVE_IGNORE_IDX,
        Directive::Type(type_directive) => match type_directive {
            TypeDirective::Scient => DIRECTIVE_SCIENT_IDX,
            TypeDirective::Hex => DIRECTIVE_HEX_IDX,
            TypeDirective::Bin => DIRECTIVE_BIN_IDX,
            TypeDirective::Octal => DIRECTIVE_OCTAL_IDX,
            TypeDirective::Unicode => DIRECTIVE_UNICODE_IDX,
        },
    };

    DirectiveId::new(idx as u32)
}

// ---- DIRECTIVE CONSTANTS END ---
