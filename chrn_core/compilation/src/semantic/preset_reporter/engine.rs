// Is engine the right word here?
//! Engine that executes a set of options to allow for composable help and note messages given the
//! context procedurally.

//TODO: General presets for engines to run to reduce boiler-plate

use chrn_utils::{
    intern::Intern, s_suffix, source_map::source_diagnostic::SourceDiagnosticBuilder,
};

use crate::{
    lookup::member_lookup,
    script_compiler::ScriptCompiler,
    semantic::preset_reporter::engine_concepts::{
        AvailableKind, EngineOption, EngineOptionBase, ListAvailable,
    },
};

// produce..., enrich...
/// Since builders need to take ownership of `self` this must return the same diagnostic after
/// transformations.
///
/// * compiler: Allows for engines to search for the semantic data from options.
/// * interner: Allows for data carrying to remain small by using the already present interner.
/// * builder: Passed by ownership, mutated, and returned since builders pass in `self` implicitly.
/// * opts: Options to edit `builder`.
///
/// NOTE: This may not alter `builder` at all depending on if no data is producible from the data given.
/// For example, a list available members option may be given, but if no members are present nothing
/// was altered.
pub fn enrich(
    compiler: &ScriptCompiler,
    interner: &Intern,
    mut builder: SourceDiagnosticBuilder,
    opt_bases: &[EngineOptionBase],
) -> SourceDiagnosticBuilder {
    //This looks. Suspicious.
    for base in opt_bases {
        // Is set to some so the loop can operate off the same option types instead of delegating a
        // special one just for the first option.
        let mut next_base = Some(base);

        // Equivalent to node.next traversal while !failed && on_failure is Some
        while let Some(opt) = next_base {
            let (new_builder, ok) = match &base.opt {
                EngineOption::ListAvailable(list_available) => {
                    exec_list_available(compiler, interner, builder, list_available)
                }
            };

            // Isn't this...like...expensive?
            // Moving? Several times?
            builder = new_builder;

            if ok {
                break;
            }
            // Re-looping if a failure option exists
            next_base = opt.on_failure.as_deref();
        }
    }
    builder
}

/// Returns builder and whether or not there was a no-op
///
/// Returns `true` if succeeded, `false` on failure
pub(super) fn exec_list_available(
    compiler: &ScriptCompiler,
    interner: &Intern,
    builder: SourceDiagnosticBuilder,
    list_available: &ListAvailable,
) -> (SourceDiagnosticBuilder, bool) {
    // Shouuld search available fields and similar name fields
    //
    // What about, if one member is similar enough, only suggest, otherwise just print
    // all fields
    // Also maybe limit the amount that can be printed at a time
    let mut available_str = String::new();

    match list_available.kind {
        AvailableKind::Member => {
            let available_members =
                member_lookup::collect_members(compiler, *&list_available.type_id);

            if available_members.is_empty() {
                return (builder, false);
            }

            let s_suffix = s_suffix!(available_members.len());
            available_str.push_str(&format!("member{s_suffix}: "));

            for (i, member_id) in available_members.iter().enumerate() {
                let member_name = interner.search(compiler.sym_members[*member_id].name_id());
                available_str.push_str(&format!("`{member_name}`"));
                if i + 1 < available_members.len() {
                    available_str.push_str(", ");
                }
            }
        }
        AvailableKind::Args => {
            todo!()
        }
    };

    // Expecting "Available {kind}: {content}"
    (builder.add_help(format!("Available {available_str}")), true)
}
