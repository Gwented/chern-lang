// I think we lost and we need a `common` crate
/// Given a len, determines if it should use a plural s
// JUST TESTING MACROS HERE
#[macro_export]
macro_rules! s_suffix {
    ($len:expr) => {
        if $len == 1 { "" } else { "s" }
    };
}

// Since the issue for this existing is fixed it doesn't HAVE to be a macro but, !
/// Allows for variadic and optional behavior when deciding how to print diagnostics
/// without duplicate functions or intrusive parameters.
#[macro_export]
macro_rules! print_diags {
    ($diags:expr) => {
        for diag in $diags {
            eprintln!("{diag}");
        }
    };
}
