pub mod args;
pub mod config;
mod detect;
pub mod dispatcher;
mod files;
mod macros;
mod renderer;

#[cfg(test)]
mod tests {
    #[cfg(test)]
    mod tests {
        use crate::detect;

        #[test]
        fn bare_name_with_prefix_is_recognized() {
            let args = vec!["chrn-fmt".to_string()];
            assert_eq!(detect::subcommand_from_bin_name(&args), Some("fmt"));
        }

        #[test]
        fn absolute_path_with_prefix_is_recognized() {
            let args = vec!["/usr/local/bin/chrn-fmt".to_string()];
            assert_eq!(detect::subcommand_from_bin_name(&args), Some("fmt"));
        }

        #[test]
        fn relative_path_with_prefix_is_recognized() {
            let args = vec!["./chrn-fmt".to_string()];
            assert_eq!(detect::subcommand_from_bin_name(&args), Some("fmt"));
        }

        #[test]
        fn bare_name_without_prefix_is_ignored() {
            let args = vec!["chrn".to_string()];
            assert_eq!(detect::subcommand_from_bin_name(&args), None);
        }

        #[test]
        fn empty_args_is_ignored() {
            let args: Vec<String> = vec![];
            assert_eq!(detect::subcommand_from_bin_name(&args), None);
        }

        #[test]
        fn non_utf8_filename_is_ignored() {
            // On Unix this is a valid path; the function should not panic.
            let args = vec!["chrn-\u{FFFD}".to_string()];
            // We don't care which result we get as long as the call doesn't panic.
            let _ = detect::subcommand_from_bin_name(&args);
        }
    }
}
