// Will be moved eventually. Maybe.

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
