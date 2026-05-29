use crate::{
    fmter::{Formattable, Formatted},
    inner_args::InnerArgs,
    types::builtins::BuiltinTypeKind,
};

// This was a horrible idea.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypeConstraint {
    // Multiple(Vec<TypeConstraint>),
    Collection,
    CharacterMappable,
    HasLen,
    Ranged,
    Ordered,
    Comparable,
    Numeric,
    Integer,
    SignedInteger,
    UnsignedInteger,
    Float,
    Bool,
    Str,
    Char,
    Runtime,
    Nil,
}

impl Formattable for TypeConstraint {
    fn to_fmt(&self) -> Formatted {
        match self {
            TypeConstraint::Collection => Formatted::Collection,
            TypeConstraint::CharacterMappable => Formatted::CharacterMappable,
            TypeConstraint::Numeric => Formatted::Numeric,
            TypeConstraint::HasLen => Formatted::HasLen,
            TypeConstraint::Integer => Formatted::Integer,
            TypeConstraint::Float => Formatted::Float,
            TypeConstraint::Bool => Formatted::Bool,
            TypeConstraint::Str => Formatted::Str,
            TypeConstraint::SignedInteger => Formatted::SignedInteger,
            TypeConstraint::UnsignedInteger => Formatted::UnsignedInteger,
            TypeConstraint::Char => Formatted::Char,
            TypeConstraint::Runtime => Formatted::Runtime,
            TypeConstraint::Ranged => Formatted::Ranged,
            TypeConstraint::Comparable => Formatted::Comparable,
            TypeConstraint::Ordered => Formatted::Ordered,
            TypeConstraint::Nil => Formatted::Nil,
        }
    }
}

///
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TypeDomainFlags {
    pub flags: u64,
}

impl TypeDomainFlags {
    pub fn new(flags: u64) -> TypeDomainFlags {
        TypeDomainFlags { flags }
    }

    pub fn to_type_constraints_vec(self) -> Vec<TypeConstraint> {
        let mut constraints = Vec::new();
        let mut flags = self.flags;

        while flags != 0 {
            let bit = flags & (!flags + 1);

            let idx = self::to_idx(bit);
            constraints.push(TYPE_CONSTRAINTS_ARRAY[idx]);

            flags ^= bit;
        }

        constraints
    }
}

///
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TypeConstraintFlags {
    pub flags: u64,
}

impl TypeConstraintFlags {
    pub fn new(flags: u64) -> TypeConstraintFlags {
        TypeConstraintFlags { flags }
    }

    pub fn runtime() -> TypeConstraintFlags {
        TypeConstraintFlags { flags: RUNTIME }
    }

    /// Turns bit-flags into a `TypeConstraint` enum vector by going through each of it's active
    /// bits.
    pub fn to_type_constraint_vec(self) -> Vec<TypeConstraint> {
        let mut constraints = Vec::new();
        let mut flags = self.flags;

        while flags != 0 {
            let bit = flags & (!flags + 1);

            let idx = self::to_idx(bit);
            constraints.push(TYPE_CONSTRAINTS_ARRAY[idx]);

            flags ^= bit;
        }

        constraints
    }

    pub fn contains_specific(self, ty_constraint: TypeConstraint) -> bool {
        let other_constraints = TypeConstraintFlags::new(ty_constraint.to_u64());

        self.contains(other_constraints);
        todo!()
    }

    /// Returns true if any concrete member of `other` falls within the domain of `self`.
    /// Unlike `contains`, this expands both sides into their concrete domains before testing.
    pub fn contains(self, other: TypeConstraintFlags) -> bool {
        let self_domain = self.get_domain();
        let other_domain = other.get_domain();
        let filtered_domains = TypeDomainFlags::new(self_domain.flags & other_domain.flags);

        filtered_domains.flags != 0
    }

    /// Get's the domain of `self` according to the const value mapping present.
    pub fn get_domain(self) -> TypeDomainFlags {
        let mut self_flags = self.flags;

        //TEST:
        if self_flags == ALL_DOMAINS {
            return TypeDomainFlags::new(ALL_DOMAINS);
        }

        let mut main_domain = ALL_DOMAINS;

        while self_flags != 0 {
            let bit = self_flags & (!self_flags + 1);

            let idx = self::to_idx(bit);
            let domain = TYPE_CONSTRAINTS_ARRAY[idx].to_u64_domain();
            main_domain &= domain;

            self_flags ^= bit;
        }

        TypeDomainFlags::new(main_domain)
    }

    // Should it be mutating itself though?
    /// Attempts to find the more restrictive of `self` and `other`.
    ///
    /// - If `self`'s concrete domain is a **subset** of `other`'s domain, `self` is more specific
    ///   returns `Some(self)`.
    /// - If `other`'s concrete domain is a **subset** of `self`'s domain, `other` is more specific
    ///   → returns `Some(other)`.
    /// - If the domains partially overlap (neither is a subset of the other), returns
    ///   `Some(other)` so callers can continue narrowing with the new constraint.
    /// - Returns `None` if the two constraints are disjoint (no overlap at all).
    pub fn try_lower_to(self, other: TypeConstraintFlags) -> Option<TypeConstraintFlags> {
        if self == other {
            return None;
        }

        let self_domain = self.get_domain();
        let other_domain = other.get_domain();

        let intersection = self_domain.flags & other_domain.flags;

        if intersection == 0 {
            // No lowering is possible.
            return None;
        }

        // If self's domain is fully contained in other's domain, self is the more restrictive
        // constraint.
        if intersection == self_domain.flags {
            return Some(self);
        }

        // If other's domain is fully contained in self's domain, other is the more restrictive
        // constraint.
        if intersection == other_domain.flags {
            return Some(other);
        }

        // Neither is a subset of the other so self is kept.
        Some(self)
    }

    pub fn try_raise_to(self, other: TypeConstraintFlags) -> Option<TypeConstraintFlags> {
        todo!()
        // if self != other && self.contains(other) {
        //     return Some(other);
        // }
        //
        // None
    }
}

pub fn to_idx(flag: u64) -> usize {
    match flag {
        SIGNED_INTEGER => 0,
        UNSIGNED_INTEGER => 1,
        FLOAT => 2,
        BOOL => 3,
        STR => 4,
        CHAR => 5,
        RUNTIME => 6,
        COMPARABLE => 7,
        CHARACTER_MAPPABLE => 8,
        HAS_LEN => 9,
        INTEGER => 10,
        NUMERIC => 11,
        RANGED => 12,
        COLLECTION => 13,
        ORDERED => 14,
        NIL => 15,
        _ => unreachable!(),
    }
}

// impl Formattable for TypeConstraintFlags {
//     fn to_fmt(&self) -> Formatted {
//         dbg!(self);
//         match self.flags {
//             COLLECTION => Formatted::Collection,
//             CHARACTER_MAPPABLE => Formatted::CharacterMappable,
//             HAS_LEN => Formatted::HasLen,
//             NUMERIC => Formatted::Numeric,
//             INTEGER => Formatted::Integer,
//             SIGNED_INTEGER => Formatted::SignedInteger,
//             FLOAT => Formatted::Float,
//             BOOL => Formatted::Bool,
//             STR => Formatted::Str,
//             CHAR => Formatted::Char,
//             UNSIGNED_INTEGER => Formatted::UnsignedInteger,
//             ANY => Formatted::Any,
//             RANGED => Formatted::Ranged,
//             COMPARABLE => Formatted::Comparable,
//             ORDERED => Formatted::Ordered,
//             _ => unreachable!("Constraint assignment failed"),
//         }
//     }
// }

// Constraints alone
pub const SIGNED_INTEGER: u64 = 1 << 0; // 0 
pub const UNSIGNED_INTEGER: u64 = 1 << 1; // 1
pub const FLOAT: u64 = 1 << 2;
pub const BOOL: u64 = 1 << 3;
pub const STR: u64 = 1 << 4;
pub const CHAR: u64 = 1 << 5;
pub const RUNTIME: u64 = 1 << 6;
pub const COMPARABLE: u64 = 1 << 7;
pub const CHARACTER_MAPPABLE: u64 = 1 << 8;
pub const HAS_LEN: u64 = 1 << 9;
pub const INTEGER: u64 = 1 << 10;
pub const NUMERIC: u64 = 1 << 11;
pub const RANGED: u64 = 1 << 12;
pub const COLLECTION: u64 = 1 << 13;
pub const ORDERED: u64 = 1 << 14;
pub const NIL: u64 = 1 << 15;

// Domains of constraints

pub const INTEGER_DOMAIN: u64 = INTEGER | SIGNED_INTEGER | UNSIGNED_INTEGER;
pub const NUMERIC_DOMAIN: u64 = NUMERIC | INTEGER_DOMAIN | FLOAT;

pub const CHARACTER_MAPPABLE_DOMAIN: u64 = CHARACTER_MAPPABLE | STR | CHAR;

pub const ORDERED_DOMAIN: u64 = ORDERED | NUMERIC_DOMAIN;

pub const COMPARABLE_DOMAIN: u64 = COMPARABLE | NUMERIC_DOMAIN | CHARACTER_MAPPABLE_DOMAIN | BOOL;

pub const HAS_LEN_DOMAIN: u64 = HAS_LEN | CHARACTER_MAPPABLE_DOMAIN | COLLECTION_DOMAIN;
pub const COLLECTION_DOMAIN: u64 = COLLECTION | STR;

pub const RANGED_DOMAIN: u64 =
    RANGED | NUMERIC_DOMAIN | COLLECTION_DOMAIN | CHARACTER_MAPPABLE_DOMAIN;

pub const ALL_DOMAINS: u64 = STR
    | CHAR
    | SIGNED_INTEGER
    | HAS_LEN
    | UNSIGNED_INTEGER
    | FLOAT
    | BOOL
    | RUNTIME
    | INTEGER
    | NUMERIC
    | RANGED
    | CHARACTER_MAPPABLE
    | COLLECTION
    | COMPARABLE
    | ORDERED
    | NIL;

pub static TYPE_CONSTRAINTS_ARRAY: [TypeConstraint; 16] = [
    TypeConstraint::SignedInteger,
    TypeConstraint::UnsignedInteger,
    TypeConstraint::Float,
    TypeConstraint::Bool,
    TypeConstraint::Str,
    TypeConstraint::Char,
    TypeConstraint::Runtime,
    TypeConstraint::Comparable,
    TypeConstraint::CharacterMappable,
    TypeConstraint::HasLen,
    TypeConstraint::Integer,
    TypeConstraint::Numeric,
    TypeConstraint::Ranged,
    TypeConstraint::Collection,
    TypeConstraint::Ordered,
    TypeConstraint::Nil,
];

impl TypeConstraint {
    /// Will return non-recursive check of compatibility
    pub fn supports_builtin_ty(&self, builtin: BuiltinTypeKind) -> bool {
        todo!()
        // if (self.to_u64() & builtin.type_constraints(false)) != 0 {
        //     return true;
        // }
        //
        // false
        // match self {
        //     TypeConstraint::CharacterMappable => builtin.is_character_mappable(),
        //     TypeConstraint::Numeric => builtin.is_numeric(),
        //     TypeConstraint::Integer => builtin.is_integer(),
        //     TypeConstraint::SignedInteger => builtin.is_signed_integer(),
        //     TypeConstraint::Float => builtin.is_float(),
        //     TypeConstraint::Bool => builtin == BuiltinTypeKind::Bool,
        //     TypeConstraint::Str => builtin == BuiltinTypeKind::Str,
        //     TypeConstraint::Char => builtin == BuiltinTypeKind::Char,
        //     TypeConstraint::Collection => builtin.is_collection(),
        //     TypeConstraint::Ordered => builtin.is_ordered(),
        //     TypeConstraint::HasLen => builtin.has_len(),
        //     TypeConstraint::UnsignedInteger => builtin.is_unsigned_integer(),
        //     TypeConstraint::Ranged => builtin.is_ranged(),
        //     TypeConstraint::Comparable => builtin.is_comparable(),
        //     // TypeConstraint::Multiple(type_constraints) => {
        //     //     for constraint in type_constraints {
        //     //         if constraint.supports_builtin_ty(builtin) {
        //     //             return true;
        //     //         }
        //     //     }
        //     //
        //     //     false
        //     // }
        //     TypeConstraint::Any => true,
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
            TypeConstraint::Runtime => RUNTIME,
            TypeConstraint::Ranged => RANGED,
            TypeConstraint::Comparable => COMPARABLE,
            TypeConstraint::Ordered => ORDERED,
            TypeConstraint::Nil => NIL,
        }
    }

    pub fn to_u64_domain(&self) -> u64 {
        match self {
            TypeConstraint::Collection => COLLECTION_DOMAIN,
            TypeConstraint::CharacterMappable => CHARACTER_MAPPABLE_DOMAIN,
            TypeConstraint::HasLen => HAS_LEN_DOMAIN,
            TypeConstraint::Numeric => NUMERIC_DOMAIN,
            TypeConstraint::Integer => INTEGER_DOMAIN,
            TypeConstraint::SignedInteger => SIGNED_INTEGER,
            TypeConstraint::Float => FLOAT,
            TypeConstraint::Bool => BOOL,
            TypeConstraint::Str => STR,
            TypeConstraint::Char => CHAR,
            TypeConstraint::UnsignedInteger => UNSIGNED_INTEGER,
            TypeConstraint::Runtime => RUNTIME,
            TypeConstraint::Ranged => RANGED_DOMAIN,
            TypeConstraint::Comparable => COMPARABLE_DOMAIN,
            TypeConstraint::Ordered => ORDERED_DOMAIN,
            TypeConstraint::Nil => NIL,
        }
    }
}

// No
impl TryFrom<u64> for TypeConstraint {
    type Error = ();

    fn try_from(val: u64) -> Result<Self, Self::Error> {
        match val {
            COLLECTION => Ok(TypeConstraint::Collection),
            CHARACTER_MAPPABLE => Ok(TypeConstraint::CharacterMappable),
            HAS_LEN => Ok(TypeConstraint::HasLen),
            NUMERIC => Ok(TypeConstraint::Numeric),
            INTEGER => Ok(TypeConstraint::Integer),
            SIGNED_INTEGER => Ok(TypeConstraint::SignedInteger),
            FLOAT => Ok(TypeConstraint::Float),
            BOOL => Ok(TypeConstraint::Bool),
            STR => Ok(TypeConstraint::Str),
            CHAR => Ok(TypeConstraint::Char),
            UNSIGNED_INTEGER => Ok(TypeConstraint::UnsignedInteger),
            RUNTIME => Ok(TypeConstraint::Runtime),
            RANGED => Ok(TypeConstraint::Ranged),
            COMPARABLE => Ok(TypeConstraint::Comparable),
            ORDERED => Ok(TypeConstraint::Ordered),
            NIL => Ok(TypeConstraint::Nil),
            _ => unreachable!("try_from failed to turn u64 constraint into enum `TypeConstraint`"),
        }
    }
}
