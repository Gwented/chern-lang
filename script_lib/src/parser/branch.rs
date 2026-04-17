use std::fmt::Display;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub(super) enum Branch {
    Broken,
    Searching,
    Neutral(NeutralBranch),
    Section(SectionBranch),
    Expr,
    Cond,
    Type,
    FuncArgs,
    /// #warn, #scient, etc parsing
    TypeArgs,
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
            Branch::Searching => write!(f, "searching [neutral]"),
            Branch::Neutral(neutral_branch) => match neutral_branch {
                NeutralBranch::Searching => write!(f, "searching [neutral]"),
                NeutralBranch::Alias => write!(f, "alias"),
                NeutralBranch::Bind => write!(f, "bind"),
                NeutralBranch::Let => write!(f, "const"),
                NeutralBranch::Import => write!(f, "import"),
            },
            Branch::Section(sect_branch) => match sect_branch {
                SectionBranch::Searching => write!(f, "searching [section]"),
                SectionBranch::Var => write!(f, "var"),
                SectionBranch::Nest => write!(f, "nest"),
                SectionBranch::NestType => write!(f, "[type]"),
                SectionBranch::NestEnum => write!(f, "[enum]"),
                SectionBranch::Complex => write!(f, "complex_rules"),
                SectionBranch::Override => write!(f, "override"),
            },
            Branch::Expr => write!(f, "[expr]"),
            Branch::Type => write!(f, "[type]"),
            Branch::Cond => write!(f, "[conditions]"),
            Branch::FuncArgs => write!(f, "[args]"),
            Branch::TypeArgs => write!(f, "[type args]"),
        }
    }
}
