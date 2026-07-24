pub mod args;
pub mod config;
mod detect;
pub mod dispatcher;
mod env_vars;
mod files;
mod macros;
mod presentation;
mod renderer;

// Argument to nullify this should exist
/// Max diagnostics that can be held by the reporter
const MAX_DIAGNOSTICS: usize = 80;

#[cfg(test)]
mod tests {
    use crate::detect;

    // Tests related to ensuring any extensions of "chrn-*" is properly parsed
    #[test]
    fn bare_name_with_prefix_is_recognized() {
        let arg = "chrn-fmt";
        assert_eq!(detect::subcommand_from_bin_name(&arg), Some("fmt"));
    }

    #[test]
    fn absolute_path_with_prefix_is_recognized() {
        let arg = "/usr/local/bin/chrn-fmt";
        assert_eq!(detect::subcommand_from_bin_name(&arg), Some("fmt"));
    }

    #[test]
    fn relative_path_with_prefix_is_recognized() {
        let arg = "./chrn-fmt";
        assert_eq!(detect::subcommand_from_bin_name(&arg), Some("fmt"));
    }

    #[test]
    fn bare_name_without_prefix_is_ignored() {
        let arg = "chrn";
        assert_eq!(detect::subcommand_from_bin_name(&arg), None);
    }

    // This test seems useless Rust enforces this
    #[test]
    fn non_utf8_filename_is_ignored() {
        // On Unix this is a valid path; the function should not panic.
        let arg = "chrn-\u{FFFD}";
        // We don't care which result we get as long as the call doesn't panic.
        _ = detect::subcommand_from_bin_name(&arg);
    }
}
