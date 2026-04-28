// Should this be pub(crate)?
mod algo;
pub mod conditions;
pub mod config_loader;
pub mod hir;
mod iyo;
pub mod lexer;
pub mod linter;
pub mod modules;
pub mod parser;
pub mod script_compiler;
pub mod semantic;
pub mod token;

// #[cfg(test)]
// mod tests {
//     // -- Helpers --
//     /// Creates fake strings for the amounts given
//     fn mock_interner(str_amt: usize, path_amt: usize) -> Intern {
//         let mut interner = Intern::init();
//
//         for idx in 0..str_amt {
//             let s = format!("dummyname{idx}");
//             interner.intern(&s);
//         }
//
//         for idx in 0..path_amt {
//             let p = format!("dummyimport{idx}");
//             let p = Path::new(&p);
//             interner.intern_path(&p);
//         }
//
//         interner
//     }
//
//     fn mock_single_module_compiler(text: &str) -> (Intern, ChernSettings, ScriptCompiler) {
//         let interner = mock_interner(0, 1);
//         let settings = ChernSettings::default();
//
//         let metadata = ChernConfigLoader::new(Path::new(""), text.as_bytes(), &settings)
//             .load_config()
//             .unwrap();
//
//         let module = Module::new(
//             Default::default(),
//             Default::default(),
//             Default::default(),
//             Default::default(),
//             metadata,
//         );
//
//         let compiler = ScriptCompiler::new(None, HashMap::default(), vec![module]);
//
//         (interner, settings, compiler)
//     }
//
//     fn mock_import(
//         name: &str,
//         path_name: &str,
//         alias_id: Option<&str>,
//         interner: &mut Intern,
//     ) -> Import {
//         Import::new(
//             InternedId::new(interner.intern(name)),
//             PathId::new(interner.intern_path(&Path::new(path_name))),
//             Default::default(),
//             alias_id.map(|a| InternedId::new(interner.intern(&a))),
//         )
//     }
//
//     fn mock_single_module(
//         name: &str,
//         path_name: &str,
//         imports: Vec<Import>,
//         mod_id: usize,
//         text: &str,
//         interner: &mut Intern,
//     ) -> Module {
//         let settings = ChernSettings::default();
//         let metadata = ChernConfigLoader::new(Path::new(""), text.as_bytes(), &settings)
//             .load_config()
//             .unwrap();
//
//         Module::new(
//             InternedId::new(interner.intern(name)),
//             PathId::new(interner.intern_path(Path::new(path_name))),
//             ModuleId::new(mod_id),
//             imports,
//             metadata,
//         )
//     }
//
//     fn mock_multiple_module_compiler(
//         modules: Vec<Module>,
//     ) -> (Intern, ChernSettings, ScriptCompiler) {
//         let interner = mock_interner(0, modules.len());
//         let settings = ChernSettings::default();
//
//         let mut mod_map = HashMap::new();
//
//         for module in &modules {
//             mod_map.insert(module.name_id, module.mod_id);
//         }
//
//         for i in 0..modules.len() {
//             for import in modules.iter().flat_map(|m| &m.imports) {
//                 if let Some(alias_id) = import.alias_id {
//                     mod_map.insert(alias_id, ModuleId::new(i));
//                 }
//             }
//         }
//
//         let compiler = ScriptCompiler::new(None, mod_map, modules);
//
//         (interner, settings, compiler)
//     }
//     // Builder?
//     //fn setup_multiple_modules(text: &str, ) -> (Intern, ChernSettings, ScriptCompiler) {}
//
//     use std::{collections::HashMap, path::Path};
//
//     use chrn_utils::{
//         id_types::{InternedId, ModuleId, PathId},
//         intern::Intern,
//         keywords,
//         values::Value,
//     };
//     use common::chern_settings::ChernSettings;
//
//     use crate::{
//         config_loader::ChernConfigLoader,
//         lexer::Lexer,
//         modules::{Import, Module},
//         parser::{self, ast::AstInfo},
//         script_compiler::{ScriptCompiler, VALUE_FALSE_POS, VALUE_TRUE_POS},
//         semantic::{
//             constraint_resolver::{ConstraintResolver, value_context::ValueContext},
//             name_resolver::NamespaceResolver,
//             scopes::ScopeType,
//             type_resolver::TypeResolver,
//         },
//         token::{Notation, Token},
//     };
//
//     #[test]
//     fn lex_tok_test() {
//         let text = r#"bind "./some/path""#;
//
//         let metadata =
//             ChernConfigLoader::new(Path::new(""), text.as_bytes(), &ChernSettings::new(false))
//                 .load_config()
//                 .unwrap();
//
//         let mut interner = Intern::init();
//
//         let toks = Lexer::new(&metadata.src_bytes, metadata.script_start).tokenize(&mut interner);
//
//         assert_eq!(
//             None, metadata.serial_start,
//             "start_offset without `@def` failed"
//         );
//         assert_eq!(3, toks.len(), "Token length exceeded 3 in lex_tok_test");
//     }
//
//     #[test]
//     fn lex_tok_test_rev() {
//         // Properly closed @def and @end
//         let correct = r#"@defbind "./some/path"@end"#;
//
//         let opt = ChernConfigLoader::new(
//             Path::new(""),
//             correct.as_bytes(),
//             &ChernSettings::new(false),
//         )
//         .load_config();
//
//         assert_eq!(true, opt.is_ok());
//
//         // Improper @def without an @end
//         // This type of error is more likely to break the diagnostic reporting but is fixed for
//         // now.
//         let wrong = r#"@defbind "./some/path""#;
//
//         let opt =
//             ChernConfigLoader::new(Path::new(""), wrong.as_bytes(), &ChernSettings::new(false))
//                 .load_config();
//
//         assert_eq!(true, opt.is_err());
//     }
//
//     //utf8 broke
//
//     #[test]
//     fn char_literal_test() {
//         // Valid single character
//         let text = "'a'";
//         let metadata =
//             ChernConfigLoader::new(Path::new(""), text.as_bytes(), &ChernSettings::new(false))
//                 .load_config()
//                 .unwrap();
//         let mut interner = Intern::init();
//         let toks = Lexer::new(&metadata.src_bytes, metadata.script_start).tokenize(&mut interner);
//
//         assert_eq!(2, toks.len());
//         assert!(
//             matches!(toks[0].tok, Token::Char(_),),
//             "Expected char token, got {:?}",
//             toks[0].tok
//         );
//
//         // Valid escaped character
//         let text = "'\\n'";
//         let metadata =
//             ChernConfigLoader::new(Path::new(""), text.as_bytes(), &ChernSettings::new(false))
//                 .load_config()
//                 .unwrap();
//
//         let mut interner = Intern::init();
//         let toks = Lexer::new(&metadata.src_bytes, metadata.script_start).tokenize(&mut interner);
//
//         assert_eq!(2, toks.len());
//         assert!(
//             matches!(toks[0].tok, Token::Char(_),),
//             "Expected char token, got {:?}",
//             toks[0].tok
//         );
//
//         // Valid hex escape
//         let text = "'\\x2F'";
//         let metadata =
//             ChernConfigLoader::new(Path::new(""), text.as_bytes(), &ChernSettings::new(false))
//                 .load_config()
//                 .unwrap();
//         let mut interner = Intern::init();
//         let toks = Lexer::new(&metadata.src_bytes, metadata.script_start).tokenize(&mut interner);
//
//         assert_eq!(2, toks.len());
//         assert!(
//             matches!(toks[0].tok, Token::Char(_),),
//             "Expected char token, got {:?}",
//             toks[0].tok
//         );
//
//         // Invalid character
//         let text = "'aa'";
//         let metadata =
//             ChernConfigLoader::new(Path::new(""), text.as_bytes(), &ChernSettings::new(false))
//                 .load_config()
//                 .unwrap();
//         let mut interner = Intern::init();
//         let toks = Lexer::new(&metadata.src_bytes, metadata.script_start).tokenize(&mut interner);
//
//         assert_eq!(2, toks.len());
//         assert!(
//             matches!(toks[0].tok, Token::Illegal(_),),
//             "Expected Illegal token, got {:?}",
//             toks[0].tok
//         );
//
//         // Invalid hex escape
//         let text = "'\\x2'";
//         let metadata =
//             ChernConfigLoader::new(Path::new(""), text.as_bytes(), &ChernSettings::new(false))
//                 .load_config()
//                 .unwrap();
//         let mut interner = Intern::init();
//         let toks = Lexer::new(&metadata.src_bytes, metadata.script_start).tokenize(&mut interner);
//
//         assert_eq!(2, toks.len());
//         assert!(
//             matches!(toks[0].tok, Token::Illegal(_),),
//             "Expected Illegal token, got {:?}",
//             toks[0].tok
//         );
//
//         // I can't actually read hex
//         // Invalid hex digits
//         let text = "'\\x255'";
//         let metadata =
//             ChernConfigLoader::new(Path::new(""), text.as_bytes(), &ChernSettings::new(false))
//                 .load_config()
//                 .unwrap();
//         let mut interner = Intern::init();
//         let toks = Lexer::new(&metadata.src_bytes, metadata.script_start).tokenize(&mut interner);
//
//         assert_eq!(2, toks.len());
//         assert!(
//             matches!(toks[0].tok, Token::Illegal(_),),
//             "Expected Illegal token, got {:?}",
//             toks[0].tok
//         );
//
//         // Unknown escape
//         let text = "'\\q'";
//         let metadata =
//             ChernConfigLoader::new(Path::new(""), text.as_bytes(), &ChernSettings::new(false))
//                 .load_config()
//                 .unwrap();
//         let mut interner = Intern::init();
//         let toks = Lexer::new(&metadata.src_bytes, metadata.script_start).tokenize(&mut interner);
//
//         assert_eq!(2, toks.len());
//         assert!(
//             matches!(toks[0].tok, Token::Illegal(_),),
//             "Expected Illegal token, got {:?}",
//             toks[0].tok
//         );
//
//         // Out of range escape
//         let text = "'\\x1Y'";
//         let metadata =
//             ChernConfigLoader::new(Path::new(""), text.as_bytes(), &ChernSettings::new(false))
//                 .load_config()
//                 .unwrap();
//         let mut interner = Intern::init();
//         let toks = Lexer::new(&metadata.src_bytes, metadata.script_start).tokenize(&mut interner);
//
//         assert_eq!(2, toks.len());
//         assert!(
//             matches!(toks[0].tok, Token::Illegal(_),),
//             "Expected Illegal token, got {:?}",
//             toks[0].tok
//         );
//     }
//
//     #[test]
//     fn multi_line_comment_test() {
//         // Properly closed multi-line comment
//         let correct = "
//             /* /* */ */
//         "
//         .as_bytes();
//
//         // Unclosed multi-line comment
//         let wrong = "
//             /* /* */
//         "
//         .as_bytes();
//
//         let correct = ChernConfigLoader::new(Path::new(""), correct, &ChernSettings::new(false))
//             .load_config();
//         let wrong =
//             ChernConfigLoader::new(Path::new(""), wrong, &ChernSettings::new(false)).load_config();
//
//         assert_eq!(true, correct.is_ok());
//         assert_eq!(true, wrong.is_err());
//     }
//
//     // beautiful name
//     #[test]
//     fn start_and_serial_offset_test() {
//         let text = format!("adwh@def var-> int: i32 @endhi");
//
//         let metadata =
//             ChernConfigLoader::new(Path::new(""), text.as_bytes(), &ChernSettings::new(false))
//                 .load_config()
//                 .unwrap();
//         dbg!(metadata.serial_start);
//
//         assert_eq!(&text[4..], &text[metadata.script_start..]);
//         assert_eq!("hi", &text[metadata.serial_start.unwrap()..]);
//         assert_eq!(28, metadata.serial_start.unwrap());
//     }
//
//     #[test]
//     fn lex_notation_test() {
//         // Hex Test (Hex Text (Hex Test))
//         let text = "0xff";
//         let metadata =
//             ChernConfigLoader::new(Path::new(""), text.as_bytes(), &ChernSettings::new(false))
//                 .load_config()
//                 .unwrap();
//         let mut interner = Intern::init();
//         let toks = Lexer::new(&metadata.src_bytes, metadata.script_start).tokenize(&mut interner);
//
//         assert_eq!(2, toks.len());
//         match toks[0].tok {
//             Token::Integer(id, Notation::Hex) => {
//                 assert_eq!("255", interner.search(id as usize));
//             }
//             _ => panic!("Expected Integer with Hex, found {:?}", toks[0].tok),
//         }
//
//         // Binary
//         let text = "0b1010";
//         let metadata =
//             ChernConfigLoader::new(Path::new(""), text.as_bytes(), &ChernSettings::new(false))
//                 .load_config()
//                 .unwrap();
//         let mut interner = Intern::init();
//         let toks = Lexer::new(&metadata.src_bytes, metadata.script_start).tokenize(&mut interner);
//
//         assert_eq!(2, toks.len());
//         match toks[0].tok {
//             Token::Integer(id, Notation::Bin) => {
//                 assert_eq!("10", interner.search(id as usize));
//             }
//             _ => panic!("Expected Integer with Binary, found {:?}", toks[0].tok),
//         }
//
//         // Octal
//         let text = "0o77";
//         let metadata =
//             ChernConfigLoader::new(Path::new(""), text.as_bytes(), &ChernSettings::new(false))
//                 .load_config()
//                 .unwrap();
//         let mut interner = Intern::init();
//         let toks = Lexer::new(&metadata.src_bytes, metadata.script_start).tokenize(&mut interner);
//
//         assert_eq!(2, toks.len());
//         match toks[0].tok {
//             Token::Integer(id, Notation::Octal) => {
//                 assert_eq!("63", interner.search(id as usize));
//             }
//             _ => panic!("Expected Integer with Octal, found {:?}", toks[0].tok),
//         }
//
//         // Decimal
//         let text = "42";
//         let metadata =
//             ChernConfigLoader::new(Path::new(""), text.as_bytes(), &ChernSettings::new(false))
//                 .load_config()
//                 .unwrap();
//
//         let mut interner = Intern::init();
//         let toks = Lexer::new(&metadata.src_bytes, metadata.script_start).tokenize(&mut interner);
//
//         assert_eq!(2, toks.len());
//         match toks[0].tok {
//             Token::Integer(id, Notation::Decimal) => {
//                 assert_eq!("42", interner.search(id as usize));
//             }
//             _ => panic!("Expected Integer of Decimal, found {:?}", toks[0].tok),
//         }
//
//         // Float with decimal
//         let text = "3.14";
//         let metadata =
//             ChernConfigLoader::new(Path::new(""), text.as_bytes(), &ChernSettings::new(false))
//                 .load_config()
//                 .unwrap();
//         let mut interner = Intern::init();
//         let toks = Lexer::new(&metadata.src_bytes, metadata.script_start).tokenize(&mut interner);
//
//         assert_eq!(2, toks.len());
//         match toks[0].tok {
//             Token::Float(id, Notation::Decimal) => {
//                 assert_eq!("3.14", interner.search(id as usize));
//             }
//             _ => panic!("Expected Float with Decimal, found {:?}", toks[0].tok),
//         }
//
//         // Positive Scientific Notation
//         let text = "1e+23";
//         let metadata =
//             ChernConfigLoader::new(Path::new(""), text.as_bytes(), &ChernSettings::new(false))
//                 .load_config()
//                 .unwrap();
//         let mut interner = Intern::init();
//         let toks = Lexer::new(&metadata.src_bytes, metadata.script_start).tokenize(&mut interner);
//
//         assert_eq!(2, toks.len());
//         match toks[0].tok {
//             Token::Float(id, Notation::Decimal) => {
//                 assert_eq!("1e+23", interner.search(id as usize));
//             }
//             _ => panic!("Expected Float with Decimal, found {:?}", toks[0].tok),
//         }
//
//         // Negative Scientific Notation
//         let text = "1e-23";
//         let metadata =
//             ChernConfigLoader::new(Path::new(""), text.as_bytes(), &ChernSettings::new(false))
//                 .load_config()
//                 .unwrap();
//         let mut interner = Intern::init();
//         let toks = Lexer::new(&metadata.src_bytes, metadata.script_start).tokenize(&mut interner);
//
//         assert_eq!(2, toks.len());
//         match toks[0].tok {
//             Token::Float(id, Notation::Decimal) => {
//                 assert_eq!("1e-23", interner.search(id as usize));
//             }
//             _ => panic!("Expected Float with Decimal, found {:?}", toks[0].tok),
//         }
//
//         // Underscored Numbers
//         let text = "1_000_000";
//         let metadata =
//             ChernConfigLoader::new(Path::new(""), text.as_bytes(), &ChernSettings::new(false))
//                 .load_config()
//                 .unwrap();
//         let mut interner = Intern::init();
//         let toks = Lexer::new(&metadata.src_bytes, metadata.script_start).tokenize(&mut interner);
//
//         assert_eq!(2, toks.len());
//         match toks[0].tok {
//             Token::Integer(id, Notation::Decimal) => {
//                 assert_eq!("1000000", interner.search(id as usize));
//             }
//             _ => panic!("Expected Integer with Decimal, found {:?}", toks[0].tok),
//         }
//
//         // Underscored Hex
//         let text = "0x_ff_ff";
//         let metadata =
//             ChernConfigLoader::new(Path::new(""), text.as_bytes(), &ChernSettings::new(false))
//                 .load_config()
//                 .unwrap();
//         let mut interner = Intern::init();
//         let toks = Lexer::new(&metadata.src_bytes, metadata.script_start).tokenize(&mut interner);
//
//         assert_eq!(2, toks.len());
//         match toks[0].tok {
//             Token::Integer(id, Notation::Hex) => {
//                 assert_eq!("65535", interner.search(id as usize));
//             }
//             _ => panic!("Expected Integer with Hex, found {:?}", toks[0].tok),
//         }
//     }
//
//     #[test]
//     fn nameresolver_duplicate_simple_test() {
//         // -- NEUTRAL --
//         let wrong = "
//             let DUPLICATE = 3
//             let DUPLICATE = \"Hi\"
//             ";
//
//         let (mut interner, settings, mut compiler) = mock_single_module_compiler(wrong);
//
//         let module = &compiler.mods[0];
//
//         let toks = Lexer::new(&module.metadata.src_bytes, module.metadata.script_start)
//             .tokenize(&mut interner);
//
//         let ast_info = parser::parse(&settings, &module, &toks, &mut interner).unwrap();
//
//         // Calls `reporter` internally but the path is fake so this fails
//         let res = NamespaceResolver::new(
//             &settings,
//             &ast_info,
//             &interner,
//             module.mod_id,
//             &mut compiler,
//         )
//         .resolve();
//
//         assert_eq!(res.is_err(), true);
//
//         let correct = "
//                 let ORIGINAL = 2 + 2
//                 let NEW = \"Hallo\"
//             ";
//
//         let (mut interner, settings, mut compiler) = mock_single_module_compiler(correct);
//
//         let module = &compiler.mods[0];
//
//         let toks = Lexer::new(&module.metadata.src_bytes, module.metadata.script_start)
//             .tokenize(&mut interner);
//
//         let ast_info = parser::parse(&settings, &module, &toks, &mut interner).unwrap();
//
//         // Calls `reporter` internally but the path is fake so this fails
//         let res = NamespaceResolver::new(
//             &settings,
//             &ast_info,
//             &interner,
//             module.mod_id,
//             &mut compiler,
//         )
//         .resolve();
//
//         assert_eq!(res.is_ok(), true);
//
//         // -- VAR --
//         let wrong = "
//             var->
//                 duplicate: i32
//                 duplicate: i8
//             ";
//
//         // Doing this first since if modules were identified during the parsing stage any
//         // syntax error within another module would not be reportable since the parser failed.
//         let (mut interner, settings, mut compiler) = mock_single_module_compiler(wrong);
//
//         let module = &compiler.mods[0];
//
//         let toks = Lexer::new(&module.metadata.src_bytes, module.metadata.script_start)
//             .tokenize(&mut interner);
//
//         let ast_info = parser::parse(&settings, &module, &toks, &mut interner).unwrap();
//
//         // Calls `reporter` internally but the path is fake so this fails
//         let res = NamespaceResolver::new(
//             &settings,
//             &ast_info,
//             &interner,
//             module.mod_id,
//             &mut compiler,
//         )
//         .resolve();
//
//         assert_eq!(res.is_err(), true);
//
//         let correct = "
//             var->
//                 original: u32
//                 new: i8
//             ";
//
//         let (mut interner, settings, mut compiler) = mock_single_module_compiler(correct);
//
//         let module = &compiler.mods[0];
//
//         let toks = Lexer::new(&module.metadata.src_bytes, module.metadata.script_start)
//             .tokenize(&mut interner);
//
//         let ast_info = parser::parse(&settings, &module, &toks, &mut interner).unwrap();
//
//         // Calls `reporter` internally but the path is fake so this fails
//         let res = NamespaceResolver::new(
//             &settings,
//             &ast_info,
//             &interner,
//             module.mod_id,
//             &mut compiler,
//         )
//         .resolve();
//
//         assert_eq!(res.is_ok(), true);
//
//         // -- NEST --
//         let mut interner = mock_interner(0, 1);
//         let settings = ChernSettings::default();
//
//         let wrong = "
//             nest->
//                 struct Duplicate {}
//                 struct Duplicate {}
//             ";
//
//         let (mut interner, settings, mut compiler) = mock_single_module_compiler(wrong);
//
//         let module = &compiler.mods[0];
//
//         let toks = Lexer::new(&module.metadata.src_bytes, module.metadata.script_start)
//             .tokenize(&mut interner);
//
//         let ast_info = parser::parse(&settings, &module, &toks, &mut interner).unwrap();
//
//         // Calls `reporter` internally but the path is fake so this fails
//         let res = NamespaceResolver::new(
//             &settings,
//             &ast_info,
//             &interner,
//             module.mod_id,
//             &mut compiler,
//         )
//         .resolve();
//
//         assert_eq!(res.is_err(), true);
//
//         let correct = "
//             nest->
//                 struct Original {}
//                 struct New {}
//             ";
//
//         let (mut interner, settings, mut compiler) = mock_single_module_compiler(correct);
//
//         let module = &compiler.mods[0];
//
//         let toks = Lexer::new(&module.metadata.src_bytes, module.metadata.script_start)
//             .tokenize(&mut interner);
//
//         let ast_info = parser::parse(&settings, &module, &toks, &mut interner).unwrap();
//
//         // Calls `reporter` internally but the path is fake so this fails
//         let res = NamespaceResolver::new(
//             &settings,
//             &ast_info,
//             &interner,
//             module.mod_id,
//             &mut compiler,
//         )
//         .resolve();
//
//         assert_eq!(res.is_ok(), true);
//         //TEST: -- COMPLEX --
//
//         //TEST: -- OVERRIDE --
//     }
//
//     #[test]
//     fn module_simple_test() {
//         // -- NEUTRAL --
//         let mut interner = mock_interner(0, 2);
//         let settings = ChernSettings::default();
//
//         let main_txt = "
//             let CONSTANT = 3
//         ";
//
//         let main_meta = ChernConfigLoader::new(Path::new(""), main_txt.as_bytes(), &settings)
//             .load_config()
//             .unwrap();
//
//         // Doing this first since if modules were identified during the parsing stage any
//         // syntax error within another module would not be reportable since the parser failed.
//
//         let sub_import = Import::new(InternedId::new(1), PathId::new(1), Default::default(), None);
//
//         let main_mod = Module::new(
//             InternedId::new(0),
//             PathId::new(0),
//             ModuleId::new(0),
//             vec![sub_import],
//             main_meta,
//         );
//
//         let sub_txt = "
//             let OTHER_CONSTANT = 5
//         ";
//
//         let sub_meta = ChernConfigLoader::new(Path::new(""), sub_txt.as_bytes(), &settings)
//             .load_config()
//             .unwrap();
//
//         let sub_mod = Module::new(
//             InternedId::new(1),
//             PathId::new(1),
//             ModuleId::new(1),
//             Default::default(),
//             sub_meta,
//         );
//
//         let mut compiler = ScriptCompiler::new(None, HashMap::default(), vec![main_mod, sub_mod]);
//
//         let mut asts: Vec<AstInfo> = Vec::new();
//
//         for mod_idx in 0..compiler.mods.len() {
//             let module = &compiler.mods[mod_idx];
//             let toks = Lexer::new(&module.metadata.src_bytes, module.metadata.script_start)
//                 .tokenize(&mut interner);
//
//             let ast_info = parser::parse(&settings, &module, &toks, &mut interner).unwrap();
//
//             NamespaceResolver::new(
//                 &settings,
//                 &ast_info,
//                 &interner,
//                 module.mod_id,
//                 &mut compiler,
//             )
//             .resolve()
//             .unwrap();
//
//             asts.push(ast_info);
//         }
//
//         let mut val_ctx = ValueContext::new();
//         for i in 0..compiler.mods.len() {
//             let mod_id = ModuleId::new(i);
//             TypeResolver::new(&settings, &asts[i], mod_id, &interner, &mut compiler)
//                 .resolve()
//                 .unwrap();
//
//             ConstraintResolver::new(
//                 &settings,
//                 &asts,
//                 &interner,
//                 mod_id,
//                 &mut val_ctx,
//                 &mut compiler,
//             )
//             .resolve()
//             .unwrap();
//         }
//     }
//
//     #[test]
//     fn module_alias_test() {
//         let mut interner = Intern::init();
//
//         let main_txt = "
//             var->
//                 reference: sub_alias.Structure
//                 other_reference: sub_alias.Enumeration
//         ";
//
//         let import = mock_import("sub_module", "sub_path", Some("sub_alias"), &mut interner);
//
//         let main_mod = mock_single_module(
//             "main",
//             "main_path",
//             vec![import],
//             0,
//             main_txt,
//             &mut interner,
//         );
//
//         let sub_txt = "
//             nest->
//                 export enum Enumeration {}
//                 export struct Structure {}
//         ";
//
//         let sub_mod = mock_single_module(
//             "sub_module",
//             "sub_path",
//             Default::default(),
//             1,
//             sub_txt,
//             &mut interner,
//         );
//
//         let (_, settings, mut compiler) = mock_multiple_module_compiler(vec![main_mod, sub_mod]);
//
//         let mut asts: Vec<AstInfo> = Vec::new();
//
//         for mod_idx in 0..compiler.mods.len() {
//             let module = &compiler.mods[mod_idx];
//             let toks = Lexer::new(&module.metadata.src_bytes, module.metadata.script_start)
//                 .tokenize(&mut interner);
//
//             let ast_info = parser::parse(&settings, &module, &toks, &mut interner).unwrap();
//
//             NamespaceResolver::new(
//                 &settings,
//                 &ast_info,
//                 &interner,
//                 module.mod_id,
//                 &mut compiler,
//             )
//             .resolve()
//             .unwrap();
//
//             asts.push(ast_info);
//         }
//
//         for i in 0..compiler.mods.len() {
//             let mod_id = ModuleId::new(i);
//             TypeResolver::new(&settings, &asts[i], mod_id, &interner, &mut compiler)
//                 .resolve()
//                 .unwrap();
//         }
//
//         let mut val_ctx = ValueContext::new();
//         for i in 0..compiler.mods.len() {
//             let mod_id = ModuleId::new(i);
//             ConstraintResolver::new(
//                 &settings,
//                 &asts,
//                 &interner,
//                 mod_id,
//                 &mut val_ctx,
//                 &mut compiler,
//             )
//             .resolve()
//             .unwrap();
//         }
//     }
//
//     #[test]
//     fn module_privacy_test() {
//         // -- PRIVATE AND FAILING --
//         let mut interner = Intern::init();
//
//         let main_txt = "
//             var->
//                 reference: sub_module.Structure
//                 other_reference: sub_module.Enumeration
//         ";
//
//         let import = mock_import("sub_module", "sub_path", Some("sub_alias"), &mut interner);
//
//         let main_mod = mock_single_module(
//             "main",
//             "main_path",
//             vec![import],
//             0,
//             main_txt,
//             &mut interner,
//         );
//
//         let sub_txt = "
//             nest->
//                 enum Enumeration {}
//                 struct Structure {}
//         ";
//
//         let sub_mod = mock_single_module(
//             "sub_module",
//             "sub_path",
//             Default::default(),
//             1,
//             sub_txt,
//             &mut interner,
//         );
//
//         let (_, settings, mut compiler) = mock_multiple_module_compiler(vec![main_mod, sub_mod]);
//
//         let mut asts: Vec<AstInfo> = Vec::new();
//
//         for mod_idx in 0..compiler.mods.len() {
//             let module = &compiler.mods[mod_idx];
//             let toks = Lexer::new(&module.metadata.src_bytes, module.metadata.script_start)
//                 .tokenize(&mut interner);
//
//             let ast_info = parser::parse(&settings, &module, &toks, &mut interner).unwrap();
//
//             NamespaceResolver::new(
//                 &settings,
//                 &ast_info,
//                 &interner,
//                 module.mod_id,
//                 &mut compiler,
//             )
//             .resolve()
//             .unwrap();
//
//             asts.push(ast_info);
//         }
//
//         let mut results = Vec::new();
//
//         for i in 0..compiler.mods.len() {
//             let mod_id = ModuleId::new(i);
//             results.push(
//                 TypeResolver::new(&settings, &asts[i], mod_id, &interner, &mut compiler).resolve(),
//             );
//         }
//
//         assert_eq!(results[0].is_err(), true, "Not exported");
//         assert_eq!(results[1].is_ok(), true, "Is fine in own context");
//
//         // -- PUBLIC AND SUCCEEDING --
//         let mut interner = Intern::init();
//
//         let main_txt = "
//             var->
//                 reference: sub_module.Structure
//                 other_reference: sub_module.Enumeration
//         ";
//
//         let import = mock_import("sub_module", "sub_path", Some("sub_alias"), &mut interner);
//
//         let main_mod = mock_single_module(
//             "main",
//             "main_path",
//             vec![import],
//             0,
//             main_txt,
//             &mut interner,
//         );
//
//         let sub_txt = "
//             nest->
//                 export enum Enumeration {}
//                 export struct Structure {}
//         ";
//
//         let sub_mod = mock_single_module(
//             "sub_module",
//             "sub_path",
//             Default::default(),
//             1,
//             sub_txt,
//             &mut interner,
//         );
//
//         let (_, settings, mut compiler) = mock_multiple_module_compiler(vec![main_mod, sub_mod]);
//
//         let mut asts: Vec<AstInfo> = Vec::new();
//
//         for mod_idx in 0..compiler.mods.len() {
//             let module = &compiler.mods[mod_idx];
//             let toks = Lexer::new(&module.metadata.src_bytes, module.metadata.script_start)
//                 .tokenize(&mut interner);
//
//             let ast_info = parser::parse(&settings, &module, &toks, &mut interner).unwrap();
//
//             NamespaceResolver::new(
//                 &settings,
//                 &ast_info,
//                 &interner,
//                 module.mod_id,
//                 &mut compiler,
//             )
//             .resolve()
//             .unwrap();
//
//             asts.push(ast_info);
//         }
//
//         let mut results = Vec::new();
//
//         for i in 0..compiler.mods.len() {
//             let mod_id = ModuleId::new(i);
//             results.push(
//                 TypeResolver::new(&settings, &asts[i], mod_id, &interner, &mut compiler).resolve(),
//             );
//         }
//
//         assert_eq!(results[0].is_ok(), true);
//         assert_eq!(results[1].is_ok(), true);
//     }
//
//     #[test]
//     fn scope_simple_test() {
//         // -- NEUTRAL --
//         let text = "
//             let CONSTANT = 3
//             ";
//
//         let (mut interner, settings, mut compiler) = mock_single_module_compiler(text);
//
//         let module = &compiler.mods[0];
//
//         let toks = Lexer::new(&module.metadata.src_bytes, module.metadata.script_start)
//             .tokenize(&mut interner);
//
//         let ast_info = parser::parse(&settings, &module, &toks, &mut interner).unwrap();
//
//         // Calls `reporter` internally but the path is fake so this fails
//         NamespaceResolver::new(
//             &settings,
//             &ast_info,
//             &interner,
//             module.mod_id,
//             &mut compiler,
//         )
//         .resolve()
//         .unwrap();
//
//         let module = &compiler.mods[0];
//
//         assert_eq!(module.scopes.len(), 1);
//         assert_eq!(module.scopes[0].scope_type, ScopeType::Neutral);
//
//         // -- VAR --
//         let text = "
//             var->
//                 variable: i32
//             ";
//
//         let (mut interner, settings, mut compiler) = mock_single_module_compiler(text);
//
//         let module = &compiler.mods[0];
//
//         let toks = Lexer::new(&module.metadata.src_bytes, module.metadata.script_start)
//             .tokenize(&mut interner);
//
//         let ast_info = parser::parse(&settings, &module, &toks, &mut interner).unwrap();
//
//         // Calls `reporter` internally but the path is fake so this fails
//         NamespaceResolver::new(
//             &settings,
//             &ast_info,
//             &interner,
//             module.mod_id,
//             &mut compiler,
//         )
//         .resolve()
//         .unwrap();
//
//         let module = &compiler.mods[0];
//
//         assert_eq!(module.scopes.len(), 1);
//         assert_eq!(module.scopes[0].scope_type, ScopeType::Var);
//
//         // -- NEST --
//         let text = "
//             nest->
//                 struct Thing1 {}
//                 struct Thing2 {}
//             ";
//
//         let (mut interner, settings, mut compiler) = mock_single_module_compiler(text);
//
//         let module = &compiler.mods[0];
//
//         let toks = Lexer::new(&module.metadata.src_bytes, module.metadata.script_start)
//             .tokenize(&mut interner);
//
//         let ast_info = parser::parse(&settings, &module, &toks, &mut interner).unwrap();
//
//         // Calls `reporter` internally but the path is fake so this fails
//         NamespaceResolver::new(
//             &settings,
//             &ast_info,
//             &interner,
//             module.mod_id,
//             &mut compiler,
//         )
//         .resolve()
//         .unwrap();
//
//         let module = &compiler.mods[0];
//
//         assert_eq!(module.scopes.len(), 1);
//         assert_eq!(module.scopes[0].scope_type, ScopeType::Nest);
//
//         // //TEST: -- COMPLEX --
//         // let mut interner = mock_interner(0, 1);
//         // let settings = ChernSettings::default();
//         //
//         // let text = "
//         //     complex->
//         //
//         //     ";
//         //
//         // let metadata = ChernConfigLoader::new(Path::new(""), text.as_bytes(), &settings)
//         //     .load_config()
//         //     .unwrap();
//         //
//         // // Doing this first since if modules were identified during the parsing stage any
//         // // syntax error within another module would not be reportable since the parser failed.
//         //
//         // let module = Module::mock(metadata);
//         //
//         // let mut compiler = ScriptCompiler::new(None, HashMap::default(), vec![module]);
//         //
//         // let module = &compiler.mods[0];
//         //
//         // let toks = Lexer::new(&module.metadata.src_bytes, module.metadata.script_start)
//         //     .tokenize(&mut interner);
//         //
//         // let ast_info = parser::parse(&settings, &module, &toks, &mut interner).unwrap();
//         //
//         // // Calls `reporter` internally but the path is fake so this fails
//         // NamespaceResolver::new(
//         //     &settings,
//         //     &ast_info,
//         //     &interner,
//         //     module.mod_id,
//         //     &mut compiler,
//         // )
//         // .resolve()
//         // .unwrap();
//         //
//         // let module = &compiler.mods[0];
//         //
//         // assert_eq!(module.scope_manager.scopes.len(), 1);
//         // assert_eq!(
//         //     module.scope_manager.scopes[0].scope_type,
//         //     ScopeType::Complex
//         // );
//         //
//         // //TEST: -- OVERRIDE --
//         // let mut interner = mock_interner(0, 1);
//         // let settings = ChernSettings::default();
//         //
//         // let text = "
//         //     complex->
//         //
//         //     ";
//         //
//         // let metadata = ChernConfigLoader::new(Path::new(""), text.as_bytes(), &settings)
//         //     .load_config()
//         //     .unwrap();
//         //
//         // // Doing this first since if modules were identified during the parsing stage any
//         // // syntax error within another module would not be reportable since the parser failed.
//         //
//         // let module = Module::mock(metadata);
//         //
//         // let mut compiler = ScriptCompiler::new(None, HashMap::default(), vec![module]);
//         //
//         // let module = &compiler.mods[0];
//         //
//         // let toks = Lexer::new(&module.metadata.src_bytes, module.metadata.script_start)
//         //     .tokenize(&mut interner);
//         //
//         // let ast_info = parser::parse(&settings, &module, &toks, &mut interner).unwrap();
//         //
//         // // Calls `reporter` internally but the path is fake so this fails
//         // NamespaceResolver::new(
//         //     &settings,
//         //     &ast_info,
//         //     &interner,
//         //     module.mod_id,
//         //     &mut compiler,
//         // )
//         // .resolve()
//         // .unwrap();
//         //
//         // let module = &compiler.mods[0];
//         //
//         // assert_eq!(module.scope_manager.scopes.len(), 1);
//         // assert_eq!(
//         //     module.scope_manager.scopes[0].scope_type,
//         //     ScopeType::Override
//         // );
//
//         // -- All scopes --
//         let mut interner = mock_interner(0, 1);
//         let settings = ChernSettings::default();
//
//         //TODO: Complex and Override
//         let text = "
//             let NEUTRAL = 3
//             var->
//                 e#var: Nest
//             nest->
//                 struct Nest {}
//             ";
//
//         let (mut interner, settings, mut compiler) = mock_single_module_compiler(text);
//
//         let module = &compiler.mods[0];
//
//         let toks = Lexer::new(&module.metadata.src_bytes, module.metadata.script_start)
//             .tokenize(&mut interner);
//
//         let ast_info = parser::parse(&settings, &module, &toks, &mut interner).unwrap();
//
//         NamespaceResolver::new(
//             &settings,
//             &ast_info,
//             &interner,
//             module.mod_id,
//             &mut compiler,
//         )
//         .resolve()
//         .unwrap();
//
//         //TODO: Override and Complex
//         let module = &compiler.mods[0];
//         assert_eq!(module.scopes.len(), 3);
//         assert_eq!(module.scopes[0].scope_type, ScopeType::Neutral);
//         assert_eq!(module.scopes[1].scope_type, ScopeType::Var);
//         assert_eq!(module.scopes[2].scope_type, ScopeType::Nest);
//     }
//
//     #[test]
//     fn type_resolver_simple_test() {
//         let wrong = "
//             var->
//                 primitive: i32
//                 undeclared_type: Thing
//             ";
//
//         let (mut interner, settings, mut compiler) = mock_single_module_compiler(wrong);
//
//         let module = &compiler.mods[0];
//
//         let toks = Lexer::new(&module.metadata.src_bytes, module.metadata.script_start)
//             .tokenize(&mut interner);
//
//         let ast_info = parser::parse(&settings, &module, &toks, &mut interner).unwrap();
//
//         NamespaceResolver::new(
//             &settings,
//             &ast_info,
//             &interner,
//             module.mod_id,
//             &mut compiler,
//         )
//         .resolve()
//         .unwrap();
//
//         let res = TypeResolver::new(
//             &settings,
//             &ast_info,
//             Default::default(),
//             &interner,
//             &mut compiler,
//         )
//         .resolve();
//
//         assert_eq!(res.is_err(), true);
//
//         let correct = "
//             var->
//                 primitive: i32
//                 declared_type: Thing
//             nest->
//                 struct Thing {}
//             ";
//
//         let (mut interner, settings, mut compiler) = mock_single_module_compiler(correct);
//
//         let module = &compiler.mods[0];
//
//         let toks = Lexer::new(&module.metadata.src_bytes, module.metadata.script_start)
//             .tokenize(&mut interner);
//
//         let ast_info = parser::parse(&settings, &module, &toks, &mut interner).unwrap();
//
//         NamespaceResolver::new(
//             &settings,
//             &ast_info,
//             &interner,
//             module.mod_id,
//             &mut compiler,
//         )
//         .resolve()
//         .unwrap();
//
//         let res = TypeResolver::new(
//             &settings,
//             &ast_info,
//             Default::default(),
//             &interner,
//             &mut compiler,
//         )
//         .resolve();
//
//         assert_eq!(res.is_ok(), true);
//     }
//
//     #[test]
//     fn type_resolver_complex_test() {
//         let text = "
//             let CONSTANT = 4
//             ";
//
//         let (mut interner, settings, mut compiler) = mock_single_module_compiler(text);
//
//         let module = &compiler.mods[0];
//
//         let toks = Lexer::new(&module.metadata.src_bytes, module.metadata.script_start)
//             .tokenize(&mut interner);
//
//         let ast_info = parser::parse(&settings, &module, &toks, &mut interner).unwrap();
//
//         NamespaceResolver::new(
//             &settings,
//             &ast_info,
//             &interner,
//             module.mod_id,
//             &mut compiler,
//         )
//         .resolve()
//         .unwrap();
//
//         TypeResolver::new(
//             &settings,
//             &ast_info,
//             Default::default(),
//             &interner,
//             &mut compiler,
//         )
//         .resolve()
//         .unwrap();
//
//         assert_eq!(compiler.symbols.len(), 1);
//     }
//
//     #[test]
//     fn constraint_resolver_let_test() {
//         let text = "
//             let CONSTANT = 4
//             ";
//
//         let (mut interner, settings, mut compiler) = mock_single_module_compiler(text);
//
//         let pre_loaded_values = compiler.values.len();
//
//         let module = &compiler.mods[0];
//
//         let toks = Lexer::new(&module.metadata.src_bytes, module.metadata.script_start)
//             .tokenize(&mut interner);
//
//         let ast_info = parser::parse(&settings, &module, &toks, &mut interner).unwrap();
//
//         NamespaceResolver::new(
//             &settings,
//             &ast_info,
//             &interner,
//             module.mod_id,
//             &mut compiler,
//         )
//         .resolve()
//         .unwrap();
//
//         TypeResolver::new(
//             &settings,
//             &ast_info,
//             Default::default(),
//             &interner,
//             &mut compiler,
//         )
//         .resolve()
//         .unwrap();
//
//         let mut val_ctx = ValueContext::new();
//
//         ConstraintResolver::new(
//             &settings,
//             &[ast_info],
//             &interner,
//             Default::default(),
//             &mut val_ctx,
//             &mut compiler,
//         )
//         .resolve()
//         .unwrap();
//
//         assert_eq!(compiler.symbols.len(), 1);
//         assert_eq!(compiler.values.len() - pre_loaded_values, 1);
//         match &compiler.values[compiler.values.len() - 1]
//             .const_val
//             .as_ref()
//             .unwrap()
//         {
//             Value::I128(_) => (),
//             _ => panic!("Value mistmatch"),
//         };
//
//         let text = "
//             let CONSTANT = \"Hallo\"
//         ";
//
//         let (mut interner, settings, mut compiler) = mock_single_module_compiler(text);
//
//         let pre_loaded_values = compiler.values.len();
//
//         let module = &compiler.mods[0];
//
//         let toks = Lexer::new(&module.metadata.src_bytes, module.metadata.script_start)
//             .tokenize(&mut interner);
//
//         let ast_info = parser::parse(&settings, &module, &toks, &mut interner).unwrap();
//
//         NamespaceResolver::new(
//             &settings,
//             &ast_info,
//             &interner,
//             module.mod_id,
//             &mut compiler,
//         )
//         .resolve()
//         .unwrap();
//
//         TypeResolver::new(
//             &settings,
//             &ast_info,
//             Default::default(),
//             &interner,
//             &mut compiler,
//         )
//         .resolve()
//         .unwrap();
//
//         ConstraintResolver::new(
//             &settings,
//             &[ast_info],
//             &interner,
//             Default::default(),
//             &mut val_ctx,
//             &mut compiler,
//         )
//         .resolve()
//         .unwrap();
//
//         assert_eq!(compiler.symbols.len(), 1);
//         assert_eq!(compiler.values.len() - pre_loaded_values, 1);
//         match &compiler.values[compiler.values.len() - 1]
//             .const_val
//             .as_ref()
//             .unwrap()
//         {
//             Value::InternedStr(_) => (),
//             _ => panic!("Value mistmatch"),
//         };
//
//         let text = "
//             let CONSTANT = 0e-5
//         ";
//
//         let (mut interner, settings, mut compiler) = mock_single_module_compiler(text);
//
//         let pre_loaded_values = compiler.values.len();
//
//         let module = &compiler.mods[0];
//
//         let toks = Lexer::new(&module.metadata.src_bytes, module.metadata.script_start)
//             .tokenize(&mut interner);
//
//         let ast_info = parser::parse(&settings, &module, &toks, &mut interner).unwrap();
//
//         NamespaceResolver::new(
//             &settings,
//             &ast_info,
//             &interner,
//             module.mod_id,
//             &mut compiler,
//         )
//         .resolve()
//         .unwrap();
//
//         TypeResolver::new(
//             &settings,
//             &ast_info,
//             Default::default(),
//             &interner,
//             &mut compiler,
//         )
//         .resolve()
//         .unwrap();
//
//         ConstraintResolver::new(
//             &settings,
//             &[ast_info],
//             &interner,
//             Default::default(),
//             &mut val_ctx,
//             &mut compiler,
//         )
//         .resolve()
//         .unwrap();
//
//         assert_eq!(compiler.symbols.len(), 1);
//         assert_eq!(compiler.values.len() - pre_loaded_values, 1);
//         match &compiler.values[compiler.values.len() - 1]
//             .const_val
//             .as_ref()
//             .unwrap()
//         {
//             Value::F64(_) => (),
//             _ => panic!("Value mistmatch"),
//         };
//
//         let text = "
//             let CONSTANT = true
//         ";
//
//         let (mut interner, settings, mut compiler) = mock_single_module_compiler(text);
//
//         let module = &compiler.mods[0];
//
//         let toks = Lexer::new(&module.metadata.src_bytes, module.metadata.script_start)
//             .tokenize(&mut interner);
//
//         let ast_info = parser::parse(&settings, &module, &toks, &mut interner).unwrap();
//
//         NamespaceResolver::new(
//             &settings,
//             &ast_info,
//             &interner,
//             module.mod_id,
//             &mut compiler,
//         )
//         .resolve()
//         .unwrap();
//
//         TypeResolver::new(
//             &settings,
//             &ast_info,
//             Default::default(),
//             &interner,
//             &mut compiler,
//         )
//         .resolve()
//         .unwrap();
//
//         ConstraintResolver::new(
//             &settings,
//             &[ast_info],
//             &interner,
//             Default::default(),
//             &mut val_ctx,
//             &mut compiler,
//         )
//         .resolve()
//         .unwrap();
//
//         assert_eq!(compiler.symbols.len(), 1);
//         assert_eq!(VALUE_TRUE_POS, 1);
//         match &compiler.values[VALUE_TRUE_POS].const_val.as_ref().unwrap() {
//             Value::Bool(true) => (),
//             _ => panic!("Value mistmatch"),
//         };
//
//         let text = "
//             let CONSTANT = false
//         ";
//
//         let (mut interner, settings, mut compiler) = mock_single_module_compiler(text);
//
//         let module = &compiler.mods[0];
//
//         let toks = Lexer::new(&module.metadata.src_bytes, module.metadata.script_start)
//             .tokenize(&mut interner);
//
//         let ast_info = parser::parse(&settings, &module, &toks, &mut interner).unwrap();
//
//         NamespaceResolver::new(
//             &settings,
//             &ast_info,
//             &interner,
//             module.mod_id,
//             &mut compiler,
//         )
//         .resolve()
//         .unwrap();
//
//         TypeResolver::new(
//             &settings,
//             &ast_info,
//             Default::default(),
//             &interner,
//             &mut compiler,
//         )
//         .resolve()
//         .unwrap();
//
//         ConstraintResolver::new(
//             &settings,
//             &[ast_info],
//             &interner,
//             Default::default(),
//             &mut val_ctx,
//             &mut compiler,
//         )
//         .resolve()
//         .unwrap();
//
//         assert_eq!(compiler.symbols.len(), 1);
//         assert_eq!(VALUE_FALSE_POS, 0);
//         match &compiler.values[VALUE_FALSE_POS].const_val.as_ref().unwrap() {
//             Value::Bool(false) => (),
//             _ => panic!("Value mistmatch"),
//         };
//     }
// }
