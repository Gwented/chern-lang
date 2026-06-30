use crate::{
    fmter::{Formattable, Formatted},
    types::builtins::BuiltinTypeKind,
};

// This was a horrible idea.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypeBoundary {
    // Multiple(Vec<TypeBoundary>),
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

impl Formattable for TypeBoundary {
    fn to_fmt(&self) -> Formatted {
        match self {
            TypeBoundary::Collection => Formatted::Collection,
            TypeBoundary::CharacterMappable => Formatted::CharacterMappable,
            TypeBoundary::Numeric => Formatted::Numeric,
            TypeBoundary::HasLen => Formatted::HasLen,
            TypeBoundary::Integer => Formatted::Integer,
            TypeBoundary::Float => Formatted::Float,
            TypeBoundary::Bool => Formatted::Bool,
            TypeBoundary::Str => Formatted::Str,
            TypeBoundary::SignedInteger => Formatted::SignedInteger,
            TypeBoundary::UnsignedInteger => Formatted::UnsignedInteger,
            TypeBoundary::Char => Formatted::Char,
            TypeBoundary::Runtime => Formatted::Runtime,
            TypeBoundary::Ranged => Formatted::Ranged,
            TypeBoundary::Comparable => Formatted::Comparable,
            TypeBoundary::Ordered => Formatted::Ordered,
            TypeBoundary::Nil => Formatted::Nil,
        }
    }
}

///
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TypeDomainFlags {
    pub flags: u64,
}

impl TypeDomainFlags {
    pub const fn new(flags: u64) -> TypeDomainFlags {
        TypeDomainFlags { flags }
    }

    pub fn to_type_constraints_vec(self) -> Vec<TypeBoundary> {
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

//TODO: Make bitflags
//I'm so scared
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TypeBoundaryFlags {
    pub flags: u64,
}

impl TypeBoundaryFlags {
    pub const fn new(flags: u64) -> TypeBoundaryFlags {
        TypeBoundaryFlags { flags }
    }

    pub const fn runtime() -> TypeBoundaryFlags {
        TypeBoundaryFlags { flags: RUNTIME }
    }

    /// Turns bit-flags into a `TypeBoundary` enum vector by going through each of it's active
    /// bits.
    pub fn to_type_constraint_vec(self) -> Vec<TypeBoundary> {
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

    pub fn contains_specific(self, ty_constraint: TypeBoundary) -> bool {
        let other_constraints = TypeBoundaryFlags::new(ty_constraint.to_u64());

        self.contains(other_constraints);
        todo!()
    }

    /// Returns true if any concrete member of `other` falls within the domain of `self`.
    /// Unlike `contains`, this expands both sides into their concrete domains before testing.
    pub fn contains(self, other: TypeBoundaryFlags) -> bool {
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
    pub fn try_lower_to(self, other: TypeBoundaryFlags) -> Option<TypeBoundaryFlags> {
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

    pub fn try_raise_to(self, other: TypeBoundaryFlags) -> Option<TypeBoundaryFlags> {
        todo!()
        // if self != other && self.contains(other) {
        //     return Some(other);
        // }
        //
        // None
    }
}

pub const fn to_idx(flag: u64) -> usize {
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

// impl Formattable for TypeBoundaryFlags {
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

pub static TYPE_CONSTRAINTS_ARRAY: [TypeBoundary; 16] = [
    TypeBoundary::SignedInteger,
    TypeBoundary::UnsignedInteger,
    TypeBoundary::Float,
    TypeBoundary::Bool,
    TypeBoundary::Str,
    TypeBoundary::Char,
    TypeBoundary::Runtime,
    TypeBoundary::Comparable,
    TypeBoundary::CharacterMappable,
    TypeBoundary::HasLen,
    TypeBoundary::Integer,
    TypeBoundary::Numeric,
    TypeBoundary::Ranged,
    TypeBoundary::Collection,
    TypeBoundary::Ordered,
    TypeBoundary::Nil,
];

impl TypeBoundary {
    /// Will return non-recursive check of compatibility
    pub fn supports_builtin_ty(&self, builtin: BuiltinTypeKind) -> bool {
        todo!()
        // if (self.to_u64() & builtin.type_constraints(false)) != 0 {
        //     return true;
        // }
        //
        // false
        // match self {
        //     TypeBoundary::CharacterMappable => builtin.is_character_mappable(),
        //     TypeBoundary::Numeric => builtin.is_numeric(),
        //     TypeBoundary::Integer => builtin.is_integer(),
        //     TypeBoundary::SignedInteger => builtin.is_signed_integer(),
        //     TypeBoundary::Float => builtin.is_float(),
        //     TypeBoundary::Bool => builtin == BuiltinTypeKind::Bool,
        //     TypeBoundary::Str => builtin == BuiltinTypeKind::Str,
        //     TypeBoundary::Char => builtin == BuiltinTypeKind::Char,
        //     TypeBoundary::Collection => builtin.is_collection(),
        //     TypeBoundary::Ordered => builtin.is_ordered(),
        //     TypeBoundary::HasLen => builtin.has_len(),
        //     TypeBoundary::UnsignedInteger => builtin.is_unsigned_integer(),
        //     TypeBoundary::Ranged => builtin.is_ranged(),
        //     TypeBoundary::Comparable => builtin.is_comparable(),
        //     // TypeBoundary::Multiple(type_constraints) => {
        //     //     for constraint in type_constraints {
        //     //         if constraint.supports_builtin_ty(builtin) {
        //     //             return true;
        //     //         }
        //     //     }
        //     //
        //     //     false
        //     // }
        //     TypeBoundary::Any => true,
        // }
    }

    pub const fn to_u64(&self) -> u64 {
        match self {
            TypeBoundary::Collection => COLLECTION,
            TypeBoundary::CharacterMappable => CHARACTER_MAPPABLE,
            TypeBoundary::HasLen => HAS_LEN,
            TypeBoundary::Numeric => NUMERIC,
            TypeBoundary::Integer => INTEGER,
            TypeBoundary::SignedInteger => SIGNED_INTEGER,
            TypeBoundary::Float => FLOAT,
            TypeBoundary::Bool => BOOL,
            TypeBoundary::Str => STR,
            TypeBoundary::Char => CHAR,
            TypeBoundary::UnsignedInteger => UNSIGNED_INTEGER,
            TypeBoundary::Runtime => RUNTIME,
            TypeBoundary::Ranged => RANGED,
            TypeBoundary::Comparable => COMPARABLE,
            TypeBoundary::Ordered => ORDERED,
            TypeBoundary::Nil => NIL,
        }
    }

    // pub const fn from_u64(self, val: u64) -> TypeBoundary {
    //     match val {
    //         COLLECTION => TypeBoundary::Collection,
    //         CHARACTER_MAPPABLE => TypeBoundary::CharacterMappable,
    //         HAS_LEN => TypeBoundary::HasLen,
    //         NUMERIC => TypeBoundary::Numeric,
    //         INTEGER => TypeBoundary::Integer,
    //         SIGNED_INTEGER => TypeBoundary::SignedInteger,
    //         FLOAT => TypeBoundary::Float,
    //         BOOL => TypeBoundary::Bool,
    //         STR => TypeBoundary::Str,
    //         CHAR => TypeBoundary::Char,
    //         UNSIGNED_INTEGER => TypeBoundary::UnsignedInteger,
    //         RUNTIME => TypeBoundary::Runtime,
    //         RANGED => TypeBoundary::Ranged,
    //         COMPARABLE => TypeBoundary::Comparable,
    //         ORDERED => TypeBoundary::Ordered,
    //         NIL => TypeBoundary::Nil,
    //         _ => unreachable!("try_from failed to turn u64 constraint into enum `TypeBoundary`"),
    //     }
    // }

    pub const fn to_u64_domain(&self) -> u64 {
        match self {
            TypeBoundary::Collection => COLLECTION_DOMAIN,
            TypeBoundary::CharacterMappable => CHARACTER_MAPPABLE_DOMAIN,
            TypeBoundary::HasLen => HAS_LEN_DOMAIN,
            TypeBoundary::Numeric => NUMERIC_DOMAIN,
            TypeBoundary::Integer => INTEGER_DOMAIN,
            TypeBoundary::SignedInteger => SIGNED_INTEGER,
            TypeBoundary::Float => FLOAT,
            TypeBoundary::Bool => BOOL,
            TypeBoundary::Str => STR,
            TypeBoundary::Char => CHAR,
            TypeBoundary::UnsignedInteger => UNSIGNED_INTEGER,
            TypeBoundary::Runtime => RUNTIME,
            TypeBoundary::Ranged => RANGED_DOMAIN,
            TypeBoundary::Comparable => COMPARABLE_DOMAIN,
            TypeBoundary::Ordered => ORDERED_DOMAIN,
            TypeBoundary::Nil => NIL,
        }
    }
}

// No
impl TryFrom<u64> for TypeBoundary {
    type Error = ();

    fn try_from(val: u64) -> Result<Self, Self::Error> {
        match val {
            COLLECTION => Ok(TypeBoundary::Collection),
            CHARACTER_MAPPABLE => Ok(TypeBoundary::CharacterMappable),
            HAS_LEN => Ok(TypeBoundary::HasLen),
            NUMERIC => Ok(TypeBoundary::Numeric),
            INTEGER => Ok(TypeBoundary::Integer),
            SIGNED_INTEGER => Ok(TypeBoundary::SignedInteger),
            FLOAT => Ok(TypeBoundary::Float),
            BOOL => Ok(TypeBoundary::Bool),
            STR => Ok(TypeBoundary::Str),
            CHAR => Ok(TypeBoundary::Char),
            UNSIGNED_INTEGER => Ok(TypeBoundary::UnsignedInteger),
            RUNTIME => Ok(TypeBoundary::Runtime),
            RANGED => Ok(TypeBoundary::Ranged),
            COMPARABLE => Ok(TypeBoundary::Comparable),
            ORDERED => Ok(TypeBoundary::Ordered),
            NIL => Ok(TypeBoundary::Nil),
            _ => unreachable!("try_from failed to turn u64 constraint into enum `TypeBoundary`"),
        }
    }
}
