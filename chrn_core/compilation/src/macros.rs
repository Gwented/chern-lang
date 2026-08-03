// The name is abstract change it. !!
/// Handles "for _ in chrn_utils::MAX_LOOPS" boilerplate by confirming that either, it will retrieve
/// a valid type id, or it will panic. This is a macro so that `loop_abort()` can be used at the
/// actual call-site rather than a function call-site.
///
/// If `chrn_utils::MAX_LOOPS` is exceeded will panic, otherwise will return a `Checked<TypeId>`
/// which guarantees that deferred is unreachable, and that the type id is not corrupted.
#[macro_export]
macro_rules! walk_type_id_deferred {
    ($type_arena:expr, $type_id:ident) => {{
        let mut abort = true;
        for _ in 0..chrn_utils::MAX_LOOPS {
            match &$type_arena[$type_id].ty {
                $crate::semantic::hir::hir_concepts::Type::Deferred(inner) => $type_id = *inner,
                _ => {
                    abort = false;
                    break;
                }
            }
        }

        if abort {
            chrn_utils::loop_abort!()
        }

        chrn_utils::utils::containers::CheckedContainer::new($type_id)
    }};
}
