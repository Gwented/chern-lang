use chrn_utils::id_types::{InternedId, PathId, ScopeId};

#[derive(Debug)]
pub(crate) struct ModuleDumpNode {
    pub(crate) name: InternedId,
    pub(crate) path: PathId,
    pub(crate) imports: Vec<InternedId>,
    pub(crate) scopes: Vec<ScopeId>,
}

#[derive(Debug)]
pub(crate) struct PrintNode {
    pub(crate) name: String,
    pub(crate) fields: PrintField,
    pub(crate) children: Vec<PrintNode>,
}

impl PrintNode {
    pub(crate) fn new(name: String, fields: PrintField, children: Vec<PrintNode>) -> PrintNode {
        PrintNode {
            name,
            fields,
            children,
        }
    }
}

#[derive(Debug)]
pub(crate) struct PrintField {
    name: String,
    value: PrintValue,
}

impl PrintField {
    pub fn new(name: String, value: PrintValue) -> PrintField {
        PrintField { name, value }
    }
}

#[derive(Debug)]
pub(crate) enum PrintValue {
    String(String),
    Integer(i64),
    Bool(bool),
    List(Vec<PrintValue>),
    Map(Box<PrintValue>, Box<PrintValue>),
}
