// Is engine the right word here?
//! Engine that executes a set of options to allow for composable help and note messages given the
//! context procedurally.

//TODO: General presets for engines to run to reduce boiler-plate

use chrn_utils::{
    id_types::TypeId, intern::Intern, s_suffix,
    source_map::source_diagnostic::SourceDiagnosticBuilder,
};

use crate::{lookup::member_lookup, script_compiler::ScriptCompiler};

/// Engine options
#[derive(Debug)]
pub(super) enum EngineOption {
    /// TypeId of parent to list the `AvailableKind` of
    ListAvailable(TypeId, AvailableKind),
}

#[derive(Debug)]
pub(super) enum AvailableKind {
    Member,
    // Maybe stuff like function arguments should NOT be coupled with this since that seems like it
    // would want a differently structured message
    Args,
}

// impl Display for AvailableKind {
//     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//         let out = match self {
//             AvailableKind::Member => "member",
//             AvailableKind::Args => "arg",
//         };
//         write!(f, "{out}")
//     }
// }

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
    opts: &[EngineOption],
) -> SourceDiagnosticBuilder {
    for opt in opts {
        match opt {
            EngineOption::ListAvailable(parent_type_id, available_kind) => {
                // Shouuld search available fields and similar name fields
                //
                // What about, if one member is similar enough, only suggest, otherwise just print
                // all fields
                // Also maybe limit the amount that can be printed at a time
                let mut available_str = String::new();

                match available_kind {
                    AvailableKind::Member => {
                        let available_members =
                            member_lookup::collect_members(compiler, *parent_type_id);

                        if available_members.is_empty() {
                            continue;
                        }

                        let s_suffix = s_suffix!(available_members.len());
                        available_str.push_str(&format!("member{s_suffix}: "));

                        for (i, member_id) in available_members.iter().enumerate() {
                            let member_name =
                                interner.search(compiler.members[*member_id].name_id());
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
                builder = builder.add_help(format!("Available {available_str}"));
            }
        }
    }
    builder
}
