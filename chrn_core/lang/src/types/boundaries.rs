//TODO: Adjust behavior as needed for the type system since some oddities from the original
//hallucinated idea are still in place since.
use bitflags::bitflags;

use crate::fmter::Formatted;

// Even though an internal implementation of this existed before-hand, the subject to error and pro
// of learning bitflags outweighs the )@$)#835j435jl yes
bitflags! {
    /// Constraints that describe what built-in types a value may have.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct TypeBoundaryFlags: u64 {
        const SIGNED_INTEGER = 1 << 0;
        const UNSIGNED_INTEGER = 1 << 1;
        const FLOAT = 1 << 2;
        const BOOL = 1 << 3;
        const STR = 1 << 4;
        const CHAR = 1 << 5;
        const RUNTIME = 1 << 6;
        const COMPARABLE = 1 << 7;
        const CHARACTER_MAPPABLE = 1 << 8;
        const HAS_LEN = 1 << 9;
        const INTEGER = 1 << 10;
        const NUMERIC = 1 << 11;
        const RANGED = 1 << 12;
        const COLLECTION = 1 << 13;
        const ORDERED = 1 << 14;
        const NIL = 1 << 15;
    }
}

bitflags! {
    /// The set of concrete built-in types that satisfy a collection of
    /// `TypeBoundaryFlags`. This is a separate type from `TypeBoundaryFlags`
    /// so that constraint logic and domain logic cannot be mixed up by accident.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct TypeDomainFlags: u64 {
        const SIGNED_INTEGER = 1 << 0;
        const UNSIGNED_INTEGER = 1 << 1;
        const FLOAT = 1 << 2;
        const BOOL = 1 << 3;
        const STR = 1 << 4;
        const CHAR = 1 << 5;
        const RUNTIME = 1 << 6;
        const COMPARABLE = 1 << 7;
        const CHARACTER_MAPPABLE = 1 << 8;
        const HAS_LEN = 1 << 9;
        const INTEGER = 1 << 10;
        const NUMERIC = 1 << 11;
        const RANGED = 1 << 12;
        const COLLECTION = 1 << 13;
        const ORDERED = 1 << 14;
        const NIL = 1 << 15;
    }
}

impl TypeDomainFlags {
    pub const INTEGER_DOMAIN: TypeDomainFlags = TypeDomainFlags::from_bits_retain(
        TypeDomainFlags::INTEGER.bits()
            | TypeDomainFlags::SIGNED_INTEGER.bits()
            | TypeDomainFlags::UNSIGNED_INTEGER.bits(),
    );

    pub const NUMERIC_DOMAIN: TypeDomainFlags = TypeDomainFlags::from_bits_retain(
        TypeDomainFlags::NUMERIC.bits()
            | TypeDomainFlags::INTEGER_DOMAIN.bits()
            | TypeDomainFlags::FLOAT.bits(),
    );

    pub const CHARACTER_MAPPABLE_DOMAIN: TypeDomainFlags = TypeDomainFlags::from_bits_retain(
        TypeDomainFlags::CHARACTER_MAPPABLE.bits()
            | TypeDomainFlags::STR.bits()
            | TypeDomainFlags::CHAR.bits(),
    );

    pub const ORDERED_DOMAIN: TypeDomainFlags = TypeDomainFlags::from_bits_retain(
        TypeDomainFlags::ORDERED.bits() | TypeDomainFlags::NUMERIC_DOMAIN.bits(),
    );

    pub const COMPARABLE_DOMAIN: TypeDomainFlags = TypeDomainFlags::from_bits_retain(
        TypeDomainFlags::COMPARABLE.bits()
            | TypeDomainFlags::NUMERIC_DOMAIN.bits()
            | TypeDomainFlags::CHARACTER_MAPPABLE_DOMAIN.bits()
            | TypeDomainFlags::BOOL.bits(),
    );

    pub const COLLECTION_DOMAIN: TypeDomainFlags = TypeDomainFlags::from_bits_retain(
        TypeDomainFlags::COLLECTION.bits() | TypeDomainFlags::STR.bits(),
    );

    pub const HAS_LEN_DOMAIN: TypeDomainFlags = TypeDomainFlags::from_bits_retain(
        TypeDomainFlags::HAS_LEN.bits()
            | TypeDomainFlags::CHARACTER_MAPPABLE_DOMAIN.bits()
            | TypeDomainFlags::COLLECTION_DOMAIN.bits(),
    );

    pub const RANGED_DOMAIN: TypeDomainFlags = TypeDomainFlags::from_bits_retain(
        TypeDomainFlags::RANGED.bits()
            | TypeDomainFlags::NUMERIC_DOMAIN.bits()
            | TypeDomainFlags::COLLECTION_DOMAIN.bits()
            | TypeDomainFlags::CHARACTER_MAPPABLE_DOMAIN.bits(),
    );
}

impl TypeBoundaryFlags {
    pub const fn runtime() -> TypeBoundaryFlags {
        TypeBoundaryFlags::RUNTIME
    }

    /// Expand these constraints into the set of concrete built-in types that
    /// can satisfy all of them.
    pub fn domain(self) -> TypeDomainFlags {
        if self == TypeBoundaryFlags::all() {
            return TypeDomainFlags::all();
        }

        let mut result = TypeDomainFlags::all();
        for flag in self.iter() {
            result &= flag.single_domain();
        }
        result
    }

    /// Returns true if any concrete type satisfies both sets of constraints.
    pub fn overlaps(self, other: TypeBoundaryFlags) -> bool {
        self.domain().intersects(other.domain())
    }

    /// Returns the more restrictive of `self` and `other` when one domain is a
    /// subset of the other. If the domains are equal, partially overlap, or are
    /// disjoint, returns `None`.
    pub fn try_lower_to(self, other: TypeBoundaryFlags) -> Option<TypeBoundaryFlags> {
        if self == other {
            return None;
        }

        let self_domain = self.domain();
        let other_domain = other.domain();
        let intersection = self_domain & other_domain;

        if intersection.is_empty() {
            return None;
        }

        if intersection == self_domain {
            return Some(self);
        }

        if intersection == other_domain {
            return Some(other);
        }

        Some(self)
    }

    /// Returns the less restrictive of `self` and `other` when one domain is a
    /// superset of the other. If the domains are equal, partially overlap, or are
    /// disjoint, returns `None`.
    pub fn try_raise_to(self, other: TypeBoundaryFlags) -> Option<TypeBoundaryFlags> {
        if self == other {
            return None;
        }

        let self_domain = self.domain();
        let other_domain = other.domain();

        if self_domain.is_empty() || other_domain.is_empty() {
            return None;
        }

        if self_domain.contains(other_domain) {
            return Some(self);
        }

        if other_domain.contains(self_domain) {
            return Some(other);
        }

        None
    }

    /// Convert each set flag into `Formatted`
    pub fn to_fmt(self) -> Vec<Formatted> {
        self.iter()
            .map(|flag| match flag {
                TypeBoundaryFlags::SIGNED_INTEGER => Formatted::SignedInteger,
                TypeBoundaryFlags::UNSIGNED_INTEGER => Formatted::UnsignedInteger,
                TypeBoundaryFlags::FLOAT => Formatted::Float,
                TypeBoundaryFlags::BOOL => Formatted::Bool,
                TypeBoundaryFlags::STR => Formatted::Str,
                TypeBoundaryFlags::CHAR => Formatted::Char,
                TypeBoundaryFlags::RUNTIME => Formatted::Runtime,
                TypeBoundaryFlags::COMPARABLE => Formatted::Comparable,
                TypeBoundaryFlags::CHARACTER_MAPPABLE => Formatted::CharacterMappable,
                TypeBoundaryFlags::HAS_LEN => Formatted::HasLen,
                TypeBoundaryFlags::INTEGER => Formatted::Integer,
                TypeBoundaryFlags::NUMERIC => Formatted::Numeric,
                TypeBoundaryFlags::RANGED => Formatted::Ranged,
                TypeBoundaryFlags::COLLECTION => Formatted::Collection,
                TypeBoundaryFlags::ORDERED => Formatted::Ordered,
                TypeBoundaryFlags::NIL => Formatted::Nil,
                _ => Formatted::Unknown,
            })
            .collect()
    }

    fn single_domain(self) -> TypeDomainFlags {
        match self {
            TypeBoundaryFlags::SIGNED_INTEGER => TypeDomainFlags::SIGNED_INTEGER,
            TypeBoundaryFlags::UNSIGNED_INTEGER => TypeDomainFlags::UNSIGNED_INTEGER,
            TypeBoundaryFlags::FLOAT => TypeDomainFlags::FLOAT,
            TypeBoundaryFlags::BOOL => TypeDomainFlags::BOOL,
            TypeBoundaryFlags::STR => TypeDomainFlags::STR,
            TypeBoundaryFlags::CHAR => TypeDomainFlags::CHAR,
            TypeBoundaryFlags::RUNTIME => TypeDomainFlags::RUNTIME,
            TypeBoundaryFlags::COMPARABLE => TypeDomainFlags::COMPARABLE_DOMAIN,
            TypeBoundaryFlags::CHARACTER_MAPPABLE => TypeDomainFlags::CHARACTER_MAPPABLE_DOMAIN,
            TypeBoundaryFlags::HAS_LEN => TypeDomainFlags::HAS_LEN_DOMAIN,
            TypeBoundaryFlags::INTEGER => TypeDomainFlags::INTEGER_DOMAIN,
            TypeBoundaryFlags::NUMERIC => TypeDomainFlags::NUMERIC_DOMAIN,
            TypeBoundaryFlags::RANGED => TypeDomainFlags::RANGED_DOMAIN,
            TypeBoundaryFlags::COLLECTION => TypeDomainFlags::COLLECTION_DOMAIN,
            TypeBoundaryFlags::ORDERED => TypeDomainFlags::ORDERED_DOMAIN,
            TypeBoundaryFlags::NIL => TypeDomainFlags::NIL,
            _ => TypeDomainFlags::empty(),
        }
    }
}

//TODO: Tests being in the same module is nice when it's small because it's local but then that's
//bloating actual code implementation, but separating into modules seems like a bit much for this
//and permissions may be lost depending on the access level of the behavior that needs to be tested.
//Maybe lib.rs for those types of tests, but try to delegate to one test module that's actually a
//directory to avoid the wall of tests.
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn domain_of_single_flags() {
        assert_eq!(
            TypeBoundaryFlags::SIGNED_INTEGER.domain(),
            TypeDomainFlags::SIGNED_INTEGER
        );
        assert_eq!(
            TypeBoundaryFlags::UNSIGNED_INTEGER.domain(),
            TypeDomainFlags::UNSIGNED_INTEGER
        );
        assert_eq!(TypeBoundaryFlags::FLOAT.domain(), TypeDomainFlags::FLOAT);
        assert_eq!(TypeBoundaryFlags::BOOL.domain(), TypeDomainFlags::BOOL);
        assert_eq!(TypeBoundaryFlags::STR.domain(), TypeDomainFlags::STR);
        assert_eq!(TypeBoundaryFlags::CHAR.domain(), TypeDomainFlags::CHAR);
        assert_eq!(
            TypeBoundaryFlags::RUNTIME.domain(),
            TypeDomainFlags::RUNTIME
        );
        assert_eq!(TypeBoundaryFlags::NIL.domain(), TypeDomainFlags::NIL);

        assert_eq!(
            TypeBoundaryFlags::INTEGER.domain(),
            TypeDomainFlags::INTEGER_DOMAIN
        );
        assert_eq!(
            TypeBoundaryFlags::NUMERIC.domain(),
            TypeDomainFlags::NUMERIC_DOMAIN
        );
        assert_eq!(
            TypeBoundaryFlags::COLLECTION.domain(),
            TypeDomainFlags::COLLECTION_DOMAIN
        );
        assert_eq!(
            TypeBoundaryFlags::CHARACTER_MAPPABLE.domain(),
            TypeDomainFlags::CHARACTER_MAPPABLE_DOMAIN
        );
        assert_eq!(
            TypeBoundaryFlags::HAS_LEN.domain(),
            TypeDomainFlags::HAS_LEN_DOMAIN
        );
        assert_eq!(
            TypeBoundaryFlags::RANGED.domain(),
            TypeDomainFlags::RANGED_DOMAIN
        );
        assert_eq!(
            TypeBoundaryFlags::COMPARABLE.domain(),
            TypeDomainFlags::COMPARABLE_DOMAIN
        );
        assert_eq!(
            TypeBoundaryFlags::ORDERED.domain(),
            TypeDomainFlags::ORDERED_DOMAIN
        );
    }

    #[test]
    fn domain_of_compound_flags_is_intersection() {
        let numeric_and_integer = TypeBoundaryFlags::NUMERIC | TypeBoundaryFlags::INTEGER;
        assert_eq!(
            numeric_and_integer.domain(),
            TypeDomainFlags::INTEGER_DOMAIN
        );

        let signed_and_integer = TypeBoundaryFlags::SIGNED_INTEGER | TypeBoundaryFlags::INTEGER;
        assert_eq!(signed_and_integer.domain(), TypeDomainFlags::SIGNED_INTEGER);
    }

    #[test]
    fn all_domain_is_full() {
        assert_eq!(TypeBoundaryFlags::all().domain(), TypeDomainFlags::all());
        assert!(TypeBoundaryFlags::all().overlaps(TypeBoundaryFlags::NIL));
    }

    #[test]
    fn overlaps() {
        assert!(TypeBoundaryFlags::NUMERIC.overlaps(TypeBoundaryFlags::INTEGER));
        assert!(TypeBoundaryFlags::INTEGER.overlaps(TypeBoundaryFlags::SIGNED_INTEGER));
        assert!(TypeBoundaryFlags::NUMERIC.overlaps(TypeBoundaryFlags::COMPARABLE));
        assert!(TypeBoundaryFlags::STR.overlaps(TypeBoundaryFlags::HAS_LEN));
        assert!(TypeBoundaryFlags::COLLECTION.overlaps(TypeBoundaryFlags::STR));

        assert!(!TypeBoundaryFlags::INTEGER.overlaps(TypeBoundaryFlags::FLOAT));
        assert!(!TypeBoundaryFlags::BOOL.overlaps(TypeBoundaryFlags::NUMERIC));
        assert!(!TypeBoundaryFlags::RUNTIME.overlaps(TypeBoundaryFlags::NIL));

        assert!(TypeBoundaryFlags::all().overlaps(TypeBoundaryFlags::RUNTIME));
        // An empty constraint set is treated as the wildcard/top, so it overlaps everything.
        assert!(TypeBoundaryFlags::empty().overlaps(TypeBoundaryFlags::NUMERIC));
    }

    #[test]
    fn try_lower_to() {
        assert_eq!(
            TypeBoundaryFlags::INTEGER.try_lower_to(TypeBoundaryFlags::SIGNED_INTEGER),
            Some(TypeBoundaryFlags::SIGNED_INTEGER)
        );
        assert_eq!(
            TypeBoundaryFlags::SIGNED_INTEGER.try_lower_to(TypeBoundaryFlags::INTEGER),
            Some(TypeBoundaryFlags::SIGNED_INTEGER)
        );
        assert_eq!(
            TypeBoundaryFlags::NUMERIC.try_lower_to(TypeBoundaryFlags::FLOAT),
            Some(TypeBoundaryFlags::FLOAT)
        );
        assert_eq!(
            TypeBoundaryFlags::NUMERIC.try_lower_to(TypeBoundaryFlags::INTEGER),
            Some(TypeBoundaryFlags::INTEGER)
        );

        assert_eq!(
            TypeBoundaryFlags::INTEGER.try_lower_to(TypeBoundaryFlags::FLOAT),
            None
        );
        assert_eq!(
            TypeBoundaryFlags::INTEGER.try_lower_to(TypeBoundaryFlags::INTEGER),
            None
        );
    }

    #[test]
    fn try_raise_to() {
        assert_eq!(
            TypeBoundaryFlags::INTEGER.try_raise_to(TypeBoundaryFlags::SIGNED_INTEGER),
            Some(TypeBoundaryFlags::INTEGER)
        );
        assert_eq!(
            TypeBoundaryFlags::SIGNED_INTEGER.try_raise_to(TypeBoundaryFlags::INTEGER),
            Some(TypeBoundaryFlags::INTEGER)
        );
        assert_eq!(
            TypeBoundaryFlags::FLOAT.try_raise_to(TypeBoundaryFlags::NUMERIC),
            Some(TypeBoundaryFlags::NUMERIC)
        );
        assert_eq!(
            TypeBoundaryFlags::INTEGER.try_raise_to(TypeBoundaryFlags::NUMERIC),
            Some(TypeBoundaryFlags::NUMERIC)
        );

        assert_eq!(
            TypeBoundaryFlags::INTEGER.try_raise_to(TypeBoundaryFlags::FLOAT),
            None
        );
        assert_eq!(
            TypeBoundaryFlags::INTEGER.try_raise_to(TypeBoundaryFlags::INTEGER),
            None
        );
    }

    #[test]
    fn to_formatted_order_and_contents() {
        let flags = TypeBoundaryFlags::INTEGER | TypeBoundaryFlags::SIGNED_INTEGER;
        assert_eq!(
            flags.to_fmt(),
            vec![Formatted::SignedInteger, Formatted::Integer]
        );

        assert_eq!(
            TypeBoundaryFlags::NUMERIC.to_fmt(),
            vec![Formatted::Numeric]
        );
    }
}
