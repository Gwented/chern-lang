use common::{intern::Intern, symbols::SymbolTable};

use crate::parser::{
    ast::{AbstractBind, AbstractEnum, AbstractFunc, AbstractStruct, AbstractType, Item},
    error::Diagnostic,
};

pub struct Analyzer<'a> {
    ast: &'a Vec<Item>,
    interner: &'a Intern,
    sym_table: SymbolTable,
    err_vec: Vec<Diagnostic>,
}

impl Analyzer<'_> {
    pub fn new<'a>(ast: &'a Vec<Item>, interner: &'a Intern) -> Analyzer<'a> {
        let sym_table = SymbolTable::new();

        Analyzer {
            ast,
            interner,
            sym_table,
            err_vec: Vec::new(),
        }
    }
}
