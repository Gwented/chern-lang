use std::fmt::Display;

#[derive(Debug)]
// I'm new to thinking. Anyone have some beginner thoughts?
pub(super) struct Diagnostic {
    //FIX:
    pub(super) msg: String,
    pub(super) branch: Branch,
    // Maybe help
    // pub(crate) help: Option<String>
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub(super) enum Branch {
    Broken,
    //TODO: Actually use neutral for something
    Neutral,
    Alias,
    Import,
    Searching,
    Expr,
    Bind,
    Var,
    VarType,
    Cond,
    FuncArgs,
    TypeArgs,
    Nest,
    NestType,
    NestEnum,
    Complex,
    Override,
}

impl Display for Branch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Branch::Broken => write!(f, "abort"),
            Branch::Neutral => write!(f, "neutral"),
            Branch::Import => write!(f, "import"),
            Branch::Alias => write!(f, "alias"),
            Branch::Searching => write!(f, "searching"),
            Branch::Bind => write!(f, "bind"),
            Branch::Expr => write!(f, "[expression]"),
            Branch::Var => write!(f, "var"),
            Branch::VarType => write!(f, "[type]"),
            Branch::Cond => write!(f, "[conditions]"),
            Branch::FuncArgs => write!(f, "[args]"),
            Branch::TypeArgs => write!(f, "[args]"),
            Branch::Nest => write!(f, "nest"),
            Branch::NestType => write!(f, "[type]"),
            Branch::NestEnum => write!(f, "[enum]"),
            Branch::Complex => write!(f, "complex_rules"),
            Branch::Override => write!(f, "override"),
        }
    }
}

impl Diagnostic {
    pub(super) fn new(msg: String, branch: Branch) -> Diagnostic {
        Diagnostic { msg, branch }
    }
}
