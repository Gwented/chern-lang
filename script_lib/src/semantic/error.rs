use common::{
    builtins::BuiltinTypeKind,
    symbols::{InnerArgs, Span, SpannedInnerArgs, TypedId},
};

#[derive(Debug)]
// Lifetimes
pub(super) enum SemanticError {
    // TypeMismatch,
    // Interesting names
    VagueArg(InnerArgs, Span),
    UnsupportedArg(SpannedInnerArgs, BuiltinTypeKind),
}

#[derive(Debug)]
pub(super) struct Diagnostic {
    //FIX:
    pub(super) msg: String,
    // Maybe help
    // pub(crate) help: Option<String>
}

impl Diagnostic {
    // TODO: May change both to &str
    pub(super) fn new(msg: String) -> Diagnostic {
        Diagnostic { msg }
    }
}
