/// Given a len, determines if it should use a plural s
// JUST TESTING MACROS HERE
#[macro_export]
macro_rules! s_ifier {
    ($len:expr) => {
        if $len == 1 { "s" } else { "" }
    };
}
