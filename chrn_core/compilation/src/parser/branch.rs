//FIXME: Parse reporting has degraded for configs
//
//TODO: May convert part of the responsibility here into a general context consuming system.
//So, something like ".env_ctx(!!!!!!)?" <- More of these where propagation is actually used to traverse
//up where parsing was conducted.
//
// KEEPING THE TREE. LET. IT. GROW.
// Ok nevermind this is too far

//! Tree representation of all possible branching for the parser.
//! Since tokens are the only source of information this allows for probabilistic help and note
//! messages to be emitted.
use std::fmt::Display;

/// Branches that represent parsing stages so that more decriptive error messages and help messages
/// can be given.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub(super) enum Branch {
    Broken,
    Searching,
    Neutral(NeutralBranch),
    Section(SectionBranch),
    Expr,
    Cond,
    Type,
    ArgList,
    /// #warn, #scient, etc
    Directive,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub(super) enum NeutralBranch {
    Searching,
    Bind,
    Alias,
    Let,
    Import,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub(super) enum SectionBranch {
    Searching,
    Var,
    Nest(NestBranch),
    Complex,
    Override,
}

impl From<NestBranch> for SectionBranch {
    fn from(branch: NestBranch) -> Self {
        SectionBranch::Nest(branch)
    }
}

// Probably not the best idea to continue
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub(super) enum NestBranch {
    NestStart,
    EnumType,
    StructType,
    Expr,
}

impl Display for Branch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Branch::Broken => write!(f, "abort"),
            Branch::Searching => write!(f, "searching [neutral]"),
            Branch::Neutral(neutral_branch) => match neutral_branch {
                NeutralBranch::Searching => write!(f, "searching [neutral]"),
                NeutralBranch::Alias => write!(f, "alias"),
                NeutralBranch::Bind => write!(f, "bind"),
                NeutralBranch::Let => write!(f, "let"),
                NeutralBranch::Import => write!(f, "import"),
            },
            Branch::Section(sect_branch) => match sect_branch {
                SectionBranch::Searching => write!(f, "searching [section]"),
                SectionBranch::Var => write!(f, "var"),
                SectionBranch::Nest(inner) => match inner {
                    NestBranch::NestStart => write!(f, "[nest]"),
                    NestBranch::EnumType => write!(f, "[type]"),
                    NestBranch::StructType => write!(f, "[type]"),
                    NestBranch::Expr => write!(f, "[expr]"),
                },
                SectionBranch::Complex => write!(f, "complex"),
                SectionBranch::Override => write!(f, "override"),
            },
            Branch::Expr => write!(f, "[expr]"),
            Branch::Type => write!(f, "[type]"),
            Branch::Cond => write!(f, "[conditions]"),
            Branch::ArgList => write!(f, "[args]"),
            Branch::Directive => write!(f, "[type args]"),
        }
    }
}
