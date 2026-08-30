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
/// Would turn `1024` into `1 KiB` and so on
#[macro_export]
macro_rules! byte_formatter {
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
            format!("{} {}", val, unit)
        } else {
            format!("{:.1} {}", val, unit)
        }
    }};
}
