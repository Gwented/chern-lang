use std::env;
use std::path::{Path, PathBuf};

/// Prefix every external `chrn` subcommand is expected to expose itself under.
const CHRN_BIN_PREFIX: &str = "chrn-";

/// Bitmask covering the owner, group, and other executable permission bits
/// in a Unix file mode. Used to test whether at least one of the three is
/// set on a candidate `chrn-<subcommand>`.
#[cfg(unix)]
const UNIX_EXEC_BITS: u32 = 0o111;

/// If the binary was invoked under a `chrn-<subcommand>` alias (for example
/// `chrn-fmt` or `/usr/local/bin/chrn-fmt`), returns the `<subcommand>` part.
///
/// The file name is taken from the leading path component of `args[0]` so the
/// alias is recognized whether the binary was invoked by bare name, relative
/// path, or absolute path.
pub fn subcommand_from_bin_name(args: &[String]) -> Option<&str> {
    let bin_name = args.first()?;
    let file_name = Path::new(bin_name).file_name()?.to_str()?;
    file_name.strip_prefix(CHRN_BIN_PREFIX)
}

/// Looks up `chrn-<subcommand>` in every directory listed in the `PATH`
/// environment variable. On Windows, also looks for the `chrn-<subcommand>.exe`
/// variant to account for the executable extension.
///
/// On Unix-like systems the candidate must also have at least one executable
/// bit set, so a non-executable file accidentally named `chrn-<x>` in `PATH`
/// is not handed off to the OS only to fail.
pub fn find_external_binary(subcommand: &str) -> Option<PathBuf> {
    if subcommand.is_empty() {
        return None;
    }
    let bin_name = format!("{CHRN_BIN_PREFIX}{subcommand}");
    env::var_os("PATH").and_then(|paths| {
        env::split_paths(&paths).find_map(|dir| {
            let candidate = dir.join(&bin_name);
            if is_runnable(&candidate) {
                return Some(candidate);
            }
            #[cfg(windows)]
            {
                let exe_candidate = dir.join(format!("{bin_name}.exe"));
                if exe_candidate.is_file() {
                    return Some(exe_candidate);
                }
            }
            None
        })
    })
}

/// returns `true` when `path` is a regular file the OS will let us execute.
fn is_runnable(path: &Path) -> bool {
    if !path.is_file() {
        return false;
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        path.metadata()
            .map(|m| m.permissions().mode() & UNIX_EXEC_BITS != 0)
            .unwrap_or(false)
    }
    #[cfg(not(unix))]
    {
        true
    }
}
