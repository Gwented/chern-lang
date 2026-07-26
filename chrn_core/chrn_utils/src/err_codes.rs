// Is this a utils decision or a compilation issue?
//!

// Maybe procedural. Not sure what we're doing :(

/// !!!!
pub const MAX_ERR_CODE_WIDTH: u16 = 4;

// No comment on this.

// Reserving 0 on purpose
// pub const ERR_CODE_DEF_WITHOUT_END: u16 = 0001;
// pub const ERR_CODE_EXCEEDED_MAX_MODS: u16 = 0002;
// pub const ERR_CODE_SCHEMA_BOUNDARY_MISMATCH: u16 = 0003;
// pub const ERR_CODE_SCHEMA_NO_BOUNDARIES_IN_VALUE: u16 = 0003;
// pub const ERR_CODE_SCHEMA_SAME_TYPE_AS_USER_MISMATCH: u16 = 0003;

// This is a soruce of truth value is shared
const CONFIG_LOAD_ERR: isize = 0001;
// chrn utils lib.rs constants as well as external tooling controlled constants
const COMPILER_SAFETY_LIMITS: isize = 0002;
// NOTE: These were too granular but still important to remember to make the doc actually refer
// to these.
//
// /// Schema option was expecting a particular boundary, but got a type with no boundaries.
// /// This is when the value's boundaries are `None`.
// SchemaNoBoundariesInValue = 0004,
// / Schema option was expecting an alignment between the config and the option's values, but a
// / mismatch occurred.
// /
// /// For "struct Thing { x: i32 }" with complex "Thing=>x { default_val = "3" }" expected the
// /// same type as the user `x` which is `i32`, but it instead got `str`
// SchemaSameTypeAsUserMismatch = 0005,
// /// Schema option was expecting a particular boundary but got a type that is incapable of
// /// holding boundaries. This focuses on the not supporting part, not the fact that the
// /// boundaries may be `None`.
// SchemaValueCannotSupportBoundaries = 0006,
// /// Identifier given as an option does not exist for particular schema
// schemaunknownoptionname = 0007,
const SCHEMA_OPTION_ERR: isize = 0003;
// What certain scopes can and can't search
// module privacy
// What can and can't be private
// Namespaces
const SCOPE_ERR: isize = 0004;
// Account for:
// Showing all known directives
// How their boundaries work
// "Vague" directive
// Circular directive
const DIRECTIVE_ERR: isize = 0005;
// module privacy
// section privacy
// What can and can't be private
// Namespaces
const PRIVACY_ERR: isize = 0006;
// Cannot declare generics
// Only List, Map, Tuple, and Set exist
// Cannot use "Generic<T>::namespace"
const GENERICS_ERR: isize = 0007;
// `nest` and `var` prefix as well as default scope searching behavior
// Config roots and config members
// How far `complex` and `override` nesting levels can go
// Embedding `override` in `complex`
// Recursive configs
const CONFIG_DECL_ERR: isize = 0008;
// Main cannot use aliases (unelss its added i guess)
// Lliases with imports that have invalid file names can oopt fo aliasesss
const IMPORT_ERR: isize = 0009;
//FIX: ConfigLoad and ConfigSchema have confusingly similar names. Should just be more distinct.

// Maybe these should lead to general docs instead of being so granular, where possible. ?
/// ITS JUST THE WAY WE'RE WAIRED
pub enum ErrorCode {
    // This is so enums remain aligned with the source of truth and error on same numeric value
    /// Config loader originating errors
    ConfigLoadErr = CONFIG_LOAD_ERR,
    /// An error emitted because of internal compiler guarantees, not the user's fault
    CompilerSafetyLimits = COMPILER_SAFETY_LIMITS,
    /// Error is from an option failing
    SchemaOptionErr = SCHEMA_OPTION_ERR,
    /// Scope error of any kind. Should lead to scope semantics.
    ScopeErr = SCOPE_ERR,
    /// Any error with directives
    DirectiveErr = DIRECTIVE_ERR,
    /// Any privacy error
    PrivacyErr = PRIVACY_ERR,
    /// Any generic error
    GenericsErr = GENERICS_ERR,
    /// Error specifically regarding how the config was declared, not schema verification
    ConfigDeclErr = CONFIG_DECL_ERR,
    ImportErr = IMPORT_ERR,
}

impl ErrorCode {
    /// Error code of `self`
    pub fn code(self) -> u16 {
        (match self {
            ErrorCode::ConfigLoadErr => CONFIG_LOAD_ERR,
            ErrorCode::CompilerSafetyLimits => COMPILER_SAFETY_LIMITS,
            ErrorCode::SchemaOptionErr => SCHEMA_OPTION_ERR,
            ErrorCode::ScopeErr => SCOPE_ERR,
            ErrorCode::DirectiveErr => DIRECTIVE_ERR,
            ErrorCode::PrivacyErr => PRIVACY_ERR,
            ErrorCode::GenericsErr => GENERICS_ERR,
            ErrorCode::ConfigDeclErr => CONFIG_DECL_ERR,
            ErrorCode::ImportErr => IMPORT_ERR,
        }) as u16
    }
}

// pub fn err_code_fmter(code: u16) -> String {
//     // debug_assert!(code < );
//     // Nothing to see here
//     let width = get_code_width(code);
//     let needed_padding = MAX_ERR_CODE_WIDTH - width;
//
//     // Avoid unneccessary format! call :(
//     let zero_padding = "0".repeat(needed_padding as usize);
//     format!("E{zero_padding}{code}")
// }

// Doesn't use same helper as module line_mapping to avoid conversion since the function is fairly simple
/// Is the preferred function for getting number widths to avoid allocating strings just for number sizes
// pub fn get_code_width(num: u16) -> u16 {
//     let mut size = 0;
//     let mut i = num;
//
//     while i != 0 {
//         i /= 10;
//         size += 1;
//     }
//
//     size
// }

// Suspicious...
#[cfg(test)]
mod tests {}
