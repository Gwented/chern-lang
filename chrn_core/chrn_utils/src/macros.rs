// Will be moved eventually. Maybe.
/// Given a len, determines if it should use a plural s
#[macro_export]
macro_rules! s_suffix {
    ($len:expr) => {
        if $len == 1 { "" } else { "s" }
    };
}

// Incredible name
/// Given a size in bytes, formats it into a human-readable string with the unit suffix to at most a
/// GB
#[macro_export]
macro_rules! format_byte_size {
    ($size:expr) => {{
        let size = $size as f64;
        const KB: f64 = 1024.0;
        const MB: f64 = 1024.0 * 1024.0;
        const GB: f64 = 1024.0 * 1024.0 * 1024.0;
        let (val, unit) = match size {
            0.0..KB => (size, "B"),
            KB..MB => (size / KB, "Kib"),
            MB..GB => (size / MB, "Mib"),
            _ => (size / GB, "Gib"),
        };
        if val == val.trunc() {
            format!("{} {}", val as u64, unit)
        } else {
            format!("{:.1} {}", val, unit)
        }
    }};
}

/// Convenience macro which prints the current file of the error and the max loops that were
/// exceeded.
///
/// Intended to reduce recursive issues and help examine said types of issues better.
#[macro_export]
macro_rules! loop_abort {
    () => {
        panic!(
            "Would be overflow\nFile = \"{}\"\nReached [`MAX_LOOPS`: {}] during recursive descent",
            file!(),
            $crate::MAX_LOOPS
        );
    };
}
