## ADDING A NEW DIRECTIVE
- Must add interned string of the identifier for it & updated INTERNER_PRELOAD_SIZE if needed
- Must add it to `load_directives()` if it's not user defined (#lang(RUST) would be user defined, #bin would be compiler defined hence so should be added to `load_directives()`)
- Must add index to script compiler's pre-registered indices if it isn't user-defined
- Must add to static directives array
- Update `try_from_interned_idr` call
- Probably should update docs
