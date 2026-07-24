Config loader rejects any script block/file above 32KB in [config_loader](./lang/src/config_loader.rs)

Module amount can't exceed MAX_MODULES in [chrn_utils](./chrn_utils/src/lib.rs)

Max diagnostics are controlled by external tooling decisions through `Budget` usage in [source_diagnostic](./chrn_utils/src/source_map/source_diagnostic.rs)
Where possible, loops use `loop_abort!`, maybe convert this into a real error if this was more so a geniune attempt at overloading the system? Right now it just assumes this was an internal bug.
