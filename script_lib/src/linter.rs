use common::intern::Intern;

use crate::parser::symbols::{Symbol, SymbolTable};

// #[macro_export]
// macro_rules! print_all {
//     ($($x:ident),* ) => {
//         $()*
//     };
// }

//TEST: THIS WILL BE MOVED
//STILL being moved but should this be a struct since it's stateful?
// pub fn print_all(sym_table: &SymbolTable, interner: &Intern) {
//     let indent = 4;
//     let spaces = " ".repeat(indent);
//
//     for symbol in sym_table.symbols().values() {
//         match symbol {
//             Symbol::Bind(bind) => {
//                 let name = interner.search(bind.name_id.id as usize);
//                 println!("Bind [\n{spaces}\"{name}\"");
//                 println!("],");
//             }
//             Symbol::Def(type_def_id) => {
//                 let type_def = sym_table.extract_typedef(*type_def_id);
//
//                 let name = interner.search(type_def.sym_id.id as usize);
//
//                 println!("TypeDef [\n{spaces}{name}");
//
//                 print_type(sym_table, indent * 2, interner);
//
//                 println!("],");
//             }
//             Symbol::Func(func_def_id) => {
//                 let func_def = sym_table.extract_func(*func_def_id);
//
//                 let name = interner.search(func_def.name_id.id as usize);
//
//                 println!("FunctionDef [\n{spaces}{name}");
//
//                 print_type(sym_table, indent, interner);
//
//                 println!("],");
//                 todo!("How did a function get here?");
//             }
//         }
//     }
//     println!("]");
// }
//
// fn print_type(sym_table: &SymbolTable, indent: usize, interner: &Intern) {
//     let spaces = " ".repeat(indent);
// }
