use std::fmt::Display;

#[derive(Debug)]
// I'm new to thinking. Anyone have some beginner thoughts?
pub struct Diagnostic {
    //FIX:
    pub(crate) msg: String,
    pub(crate) branch: Branch,
    // Maybe help
    // pub(crate) help: Option<String>
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub(crate) enum Branch {
    Broken,
    Searching,
    Bind,
    Var,
    VarType,
    VarCond,
    VarFuncArgs,
    VarTypeArgs,
    Nest,
    NestType,
    ComplexRules,
}

impl Display for Branch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Branch::Broken => write!(f, "abort"),
            Branch::Searching => write!(f, "searching..."),
            Branch::Bind => write!(f, "bind"),
            Branch::Var => write!(f, "var"),
            Branch::VarType => write!(f, "[type]"),
            Branch::VarCond => write!(f, "[conditions]"),
            Branch::VarFuncArgs => write!(f, "[args]"),
            Branch::VarTypeArgs => write!(f, "[args]"),
            Branch::Nest => write!(f, "nest"),
            Branch::NestType => write!(f, "[type]"),
            Branch::ComplexRules => write!(f, "complex_rules"),
        }
    }
}

impl Diagnostic {
    pub(crate) fn new(msg: String, branch: Branch) -> Diagnostic {
        Diagnostic { msg, branch }
    }
}
