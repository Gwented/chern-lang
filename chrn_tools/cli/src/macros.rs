// Since the issue for this existing is fixed it doesn't HAVE to be a macro but, !
// So remove the macro?
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
