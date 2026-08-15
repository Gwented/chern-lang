pub mod algo;
pub mod chrn_classifier;
pub mod config_schemas;
pub mod directives;
pub mod keywords;
pub mod lang_config;
pub mod types;
pub mod values;

// I think this is appropriate placement?
// It IS a general language level rule, but at the same time what if the implementation was
// different? Maybe move this.
/// Max depth for config reach for `chrn`, excluding `override` section expansion
pub const CFG_MAX_COMPLEX_NEST_LEVEL: u8 = 2;

#[cfg(test)]
mod tests {
    // #[test]
    // fn specific_name() {
    //     panic!();
    // }
}
