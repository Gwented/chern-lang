use chrn_utils::{id_types::InternedId, intern};

use crate::{
    fmter::{Formattable, Formatted},
    types::{
        builtins::BuiltinType,
        type_constraints::{self, TypeConstraintFlags},
    },
};

/// If a new argument is added ensure this is updated
pub static BUILTIN_DIRECTIVE_STRS: [&str; 6] = ["warn", "scient", "hex", "bin", "octal", "ignore"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// General directives not specific to anything
pub enum Directive {
    Warn,
    Ignore,
    Type(TypeDirective),
}

impl From<TypeDirective> for Directive {
    fn from(val: TypeDirective) -> Self {
        Directive::Type(val)
    }
}

impl Directive {
    pub fn has_restrictions(self) -> bool {
        match self {
            Directive::Warn | Directive::Ignore => false,
            Directive::Type(type_directive) => type_directive.has_restrictions(),
        }
    }
    pub fn supports_builtin_type(self, builtin_type: &BuiltinType) -> bool {
        match self {
            Directive::Warn | Directive::Ignore => true,
            Directive::Type(type_directive) => type_directive.supports_builtin_type(builtin_type),
        }
    }

    pub fn type_constraints(self) -> TypeConstraintFlags {
        let flags = match self {
            Directive::Warn | Directive::Ignore => {
                TypeConstraintFlags::new(type_constraints::ALL_DOMAINS)
            }
            Directive::Type(type_directive) => type_directive.type_constraints(),
        };

        flags
    }

    pub fn try_from_interned_str(interned_id: InternedId) -> Option<Directive> {
        match interned_id.id {
            intern::INTERNED_WARN => Some(Directive::Warn),
            intern::INTERNED_IGNORE => Some(Directive::Ignore),
            // Ok this looks confusing
            _ => Some(TypeDirective::try_from_interned_str(interned_id)?.into()),
        }
    }
}

impl Formattable for Directive {
    fn to_fmt(&self) -> crate::fmter::Formatted {
        match self {
            Directive::Warn => Formatted::DirectiveWarn,
            Directive::Ignore => Formatted::DirectiveIgnore,
            Directive::Type(type_directive) => type_directive.to_fmt(),
        }
    }
}

// May make it split to where there's compiler directive, type directive, but keepign it as this
// for now
/// Directives specific to types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypeDirective {
    Scient,
    Hex,
    Bin,
    Octal,
}

impl TypeDirective {
    // has_restrictions?
    /// Returns true if the given argument is applicable to every type, such as `#warn`, otherwise
    /// returns false
    pub fn has_restrictions(self) -> bool {
        match self {
            TypeDirective::Scient
            | TypeDirective::Hex
            | TypeDirective::Bin
            | TypeDirective::Octal => true,
        }
    }

    /// This MUST be used after ensuring the type is a primitive, not a data structure.
    // Maybe this is a good time to use kind
    pub fn supports_builtin_type(self, builtin_type: &BuiltinType) -> bool {
        match self {
            TypeDirective::Scient
            | TypeDirective::Hex
            | TypeDirective::Bin
            | TypeDirective::Octal => {
                match builtin_type {
                    BuiltinType::I8
                    | BuiltinType::U8
                    | BuiltinType::I16
                    | BuiltinType::U16
                    | BuiltinType::F16
                    | BuiltinType::I32
                    | BuiltinType::U32
                    | BuiltinType::F32
                    | BuiltinType::I64
                    | BuiltinType::U64
                    | BuiltinType::F64
                    | BuiltinType::I128
                    | BuiltinType::U128
                    | BuiltinType::F128
                    | BuiltinType::Sized
                    | BuiltinType::BigInt
                    | BuiltinType::BigFloat
                    | BuiltinType::Unsized
                    //NOTE: Checks this at runtime
                    |BuiltinType::Runtime => true,
                    // Maybe this means that it shouldn't be a method, it should be a function that
                    // has access to their inner, which can do the rolving. Rolving.
                    //
                    // This is unreachable because when arguments are resolved, it requires the
                    // data structures to be recursively resolved into a builtin type
                    BuiltinType::List(_)
                    |BuiltinType::Set(_)
                    |BuiltinType::Map(_, _) => unreachable!("ConstraintResolver broke"),
                    _ => false,
                }
            }
        }
    }

    // pub fn supports_type_constraint(&self, constraint_flags: TypeConstraintFlags) -> bool {
    //     todo!()
    // match self {
    //     InnerArgs::Scientific | InnerArgs::Hex | InnerArgs::Binary | InnerArgs::Octal => {
    //         match constraint_flags.flags {
    //             TypeConstraint::Collection | TypeConstraint::HasLen if is_rec => true,
    //             TypeConstraint::Numeric
    //             | TypeConstraint::Integer
    //             | TypeConstraint::SignedInteger
    //             | TypeConstraint::UnsignedInteger
    //             | TypeConstraint::Float
    //             | TypeConstraint::Ordered
    //             | TypeConstraint::Any => true,
    //             TypeConstraint::Bool
    //             | TypeConstraint::Collection
    //             | TypeConstraint::HasLen
    //             | TypeConstraint::CharacterMappable
    //             | TypeConstraint::Char
    //             | TypeConstraint::Ranged
    //             | TypeConstraint::Comparable
    //             | TypeConstraint::Str => false,
    //         }
    //     }
    //     InnerArgs::Ignore | InnerArgs::Warn => true,
    // }
    // }

    pub fn type_constraints(self) -> TypeConstraintFlags {
        let flags = match self {
            TypeDirective::Scient
            | TypeDirective::Hex
            | TypeDirective::Bin
            | TypeDirective::Octal => type_constraints::NUMERIC,
        };

        TypeConstraintFlags::new(flags)
    }

    pub fn try_from_interned_str(interned_id: InternedId) -> Option<TypeDirective> {
        match interned_id.id {
            intern::INTERNED_SCIENT => Some(TypeDirective::Scient),
            intern::INTERNED_HEX => Some(TypeDirective::Hex),
            intern::INTERNED_BIN => Some(TypeDirective::Bin),
            intern::INTERNED_OCTAL => Some(TypeDirective::Octal),
            _ => None,
        }
    }
}

impl Formattable for TypeDirective {
    fn to_fmt(&self) -> Formatted {
        match self {
            TypeDirective::Scient => Formatted::DirectiveScient,
            TypeDirective::Hex => Formatted::DirectiveHex,
            TypeDirective::Bin => Formatted::DirectiveBin,
            TypeDirective::Octal => Formatted::DirectiveOctal,
        }
    }
}
