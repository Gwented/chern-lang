use chrn_utils::id_types::InternedId;

#[derive(Debug)]
pub struct DumpSettings {
    pub mod_opts: ModuleOptions,
    pub output_kind: DumpOutputKind,
}

impl DumpSettings {
    pub fn new(mod_opts: ModuleOptions, output_kind: DumpOutputKind) -> DumpSettings {
        DumpSettings {
            mod_opts,
            output_kind,
        }
    }
}

#[derive(Debug)]
pub enum ModuleOptions {
    EntryPoint,
    Only(Vec<String>),
    Skip(Vec<String>),
    All,
}

// Maybe this kind suffix trend is a little redundant
#[derive(Debug)]
pub enum DumpOutputKind {
    Cli,
    // Yup
    Asm,
}
