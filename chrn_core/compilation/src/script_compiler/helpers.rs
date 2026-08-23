//! General purpose of this module and all sub-modules is to have composable datasets which
//! allows for procedural insertion.
pub(super) mod compiler_helpers;
pub(crate) mod core_helpers;
pub(super) mod extern_helpers;
// `core_helpers` is `pub(crate)` and its datasets are typed in terms of these, so the module is
// crate-visible too rather than leaking types out of a private module
pub(crate) mod instantiation_symbols;
