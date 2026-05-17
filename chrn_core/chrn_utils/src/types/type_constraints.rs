use common::fmter::{Formattable, Formatted};

use crate::types::builtins::BuiltinTypeKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypeConstraint {
    // Multiple(Vec<TypeConstraint>),
    Collection,
    CharacterMappable,
    HasLen,
    Numeric,
    Integer,
    SignedInteger,
    UnsignedInteger,
    Float,
    Bool,
    Str,
    Char,
    Any,
}

impl Formattable for TypeConstraint {
    fn to_fmt(&self) -> common::fmter::Formatted {
        match self {
            TypeConstraint::Collection => Formatted::TypeConstraintCollection,
            TypeConstraint::CharacterMappable => Formatted::TypeConstraintCharacterMappable,
            TypeConstraint::Numeric => Formatted::TypeConstraintNumeric,
            TypeConstraint::HasLen => Formatted::TypeConstraintHasLen,
            TypeConstraint::Integer => Formatted::Integer,
            TypeConstraint::Float => Formatted::Float,
            TypeConstraint::Bool => Formatted::Bool,
            TypeConstraint::Str => Formatted::Str,
            TypeConstraint::SignedInteger => Formatted::SignedInteger,
            TypeConstraint::UnsignedInteger => Formatted::UnsignedInteger,
            TypeConstraint::Char => Formatted::Char,
            TypeConstraint::Any => Formatted::Any,
        }
    }
}

pub const CHARACTER_MAPPABLE: u64 = 1 << 1;
pub const SIGNED_INTEGER: u64 = 1 << 2;
pub const UNSIGNED_INTEGER: u64 = 1 << 3;
pub const FLOAT: u64 = 1 << 4;
pub const BOOL: u64 = 1 << 5;
pub const STR: u64 = 1 << 6;
pub const CHAR: u64 = 1 << 7;
pub const HAS_LEN: u64 = 1 << 8;
pub const ANY: u64 = 1 << 9;
pub const NUMERIC: u64 = INTEGER | FLOAT;
pub const INTEGER: u64 = SIGNED_INTEGER | UNSIGNED_INTEGER;
// Sure it doesn't include data structures explicilty but does that matter here?
pub const COLLECTION: u64 = STR;

impl TypeConstraint {
    /// Will return non-recursive check of compatibility
    pub fn supports_builtin_ty(&self, builtin: BuiltinTypeKind) -> bool {
        match self {
            TypeConstraint::CharacterMappable => builtin.is_character_mappable(),
            TypeConstraint::Numeric => builtin.is_numeric(),
            TypeConstraint::Integer => builtin.is_integer(),
            TypeConstraint::SignedInteger => builtin.is_signed_integer(),
            TypeConstraint::Float => builtin.is_float(),
            TypeConstraint::Bool => builtin == BuiltinTypeKind::Bool,
            TypeConstraint::Str => builtin == BuiltinTypeKind::Str,
            TypeConstraint::Char => builtin == BuiltinTypeKind::Char,
            TypeConstraint::Collection => builtin.is_collection(),
            // TypeConstraint::Multiple(type_constraints) => {
            //     for constraint in type_constraints {
            //         if constraint.supports_builtin_ty(builtin) {
            //             return true;
            //         }
            //     }
            //
            //     false
            // }
            TypeConstraint::HasLen => builtin.has_len(),
            TypeConstraint::UnsignedInteger => builtin.is_unsigned_integer(),
            TypeConstraint::Any => true,
        }
    }

    /// Checks if `self` is within the set of `other`
    pub fn alignes_with(&self, other: TypeConstraint) -> bool {
        (self.to_u64() & other.to_u64()) != 0
    }

    /// If any member of the given type constraint falls under the same as `self`, that constraint
    /// is returned in the lowered form
    pub fn try_lower_to(&self, other: TypeConstraint) -> Option<TypeConstraint> {
        if (self.to_u64() & other.to_u64()) != 0 {
            Some(other)
        } else {
            None
        }
        // match self {
        //     TypeConstraint::Collection => match other {
        //         TypeConstraint::Collection
        //         | TypeConstraint::CharacterMappable
        //         | TypeConstraint::Str
        //         | TypeConstraint::HasLen => Some(other),
        //         TypeConstraint::Numeric
        //         | TypeConstraint::Integer
        //         | TypeConstraint::SignedInteger
        //         | TypeConstraint::UnsignedInteger
        //         | TypeConstraint::Float
        //         | TypeConstraint::Bool
        //         // Any is the highest type in the hierarchy so it probably can't be lowered or
        //         // raised
        //         | TypeConstraint::Any => None,
        //         TypeConstraint::Char => todo!(),
        //     },
        //     TypeConstraint::CharacterMappable => match other {
        //         TypeConstraint::CharacterMappable | TypeConstraint::Str | TypeConstraint::Char => {
        //             Some(other)
        //         }
        //         TypeConstraint::Collection | TypeConstraint::HasLen => todo!(),
        //         TypeConstraint::Numeric => todo!(),
        //         TypeConstraint::Integer => todo!(),
        //         TypeConstraint::SignedInteger => todo!(),
        //         TypeConstraint::UnsignedInteger => todo!(),
        //         TypeConstraint::Float => todo!(),
        //         TypeConstraint::Bool => todo!(),
        //         TypeConstraint::Any => todo!(),
        //     },
        //     TypeConstraint::HasLen => todo!(),
        //     TypeConstraint::Numeric => todo!(),
        //     TypeConstraint::Integer => todo!(),
        //     TypeConstraint::SignedInteger => todo!(),
        //     TypeConstraint::UnsignedInteger => todo!(),
        //     TypeConstraint::Float => todo!(),
        //     TypeConstraint::Bool => todo!(),
        //     TypeConstraint::Str => todo!(),
        //     TypeConstraint::Any => todo!(),
        //     TypeConstraint::Char => todo!(),
        // }
    }

    pub fn to_u64(&self) -> u64 {
        match self {
            TypeConstraint::Collection => COLLECTION,
            TypeConstraint::CharacterMappable => CHARACTER_MAPPABLE,
            TypeConstraint::HasLen => HAS_LEN,
            TypeConstraint::Numeric => NUMERIC,
            TypeConstraint::Integer => INTEGER,
            TypeConstraint::SignedInteger => SIGNED_INTEGER,
            TypeConstraint::Float => FLOAT,
            TypeConstraint::Bool => BOOL,
            TypeConstraint::Str => STR,
            TypeConstraint::Char => CHAR,
            TypeConstraint::UnsignedInteger => UNSIGNED_INTEGER,
            TypeConstraint::Any => ANY,
        }
    }
}

// Nat
// Real
// Complex
// Prime
// TEST:
