use std::collections::HashMap;
use std::io::Cursor;
use std::path::PathBuf;

use chrn_utils::id_types::InternedId;
use chrn_utils::id_types::ModuleId;
use chrn_utils::id_types::PathId;
use chrn_utils::intern::Intern;
use chrn_utils::keywords::Keyword;
use chrn_utils::values::Value as CValue;
use common::fmter::Formattable;
use script_lib::config_loader::ChernConfigLoader;
use script_lib::lexer::Lexer;
use script_lib::modules::Module;
use script_lib::script_compiler::ScriptCompiler;
use script_lib::semantic::name_resolver::NamespaceResolver;
use script_lib::semantic::representation::Type;
use script_lib::semantic::scopes::ScopeType;
use script_lib::semantic::type_resolver::TypeResolver;
use script_lib::semantic::type_resolver::type_context::TypeContext;
use script_lib::token::Token as ScriptToken;
use tower_lsp::lsp_types::*;

use common::chrn_settings::ChernSettings;

use crate::{
    hover,
    text::{extract_word_at, position_to_offset},
};

pub fn compute_hover(text: &str, uri: &str, pos: Position) -> Option<Hover> {
    let path_buf = PathBuf::from(uri);
    let src_bytes = text.as_bytes().to_vec();
    let settings = ChernSettings::default();

    let metadata = match ChernConfigLoader::new(
        path_buf.as_path(),
        Cursor::new(src_bytes.clone()),
        &settings,
    )
    .load_config()
    {
        Ok(m) => m,
        Err(_) => return None,
    };

    let mut interner = Intern::init();
    let toks = script_lib::lexer::Lexer::new(text.as_bytes(), metadata.script_start)
        .tokenize(&mut interner);

    let offset = position_to_offset(&text, pos);

    let serial_start = metadata.serial_start.unwrap_or(text.len());

    if offset < metadata.script_start || offset >= serial_start {
        return None;
    }

    let mut found: Option<(ScriptToken, usize, usize)> = None;
    for st in toks.iter() {
        let span = st.span;
        if offset >= span.start && offset <= span.end {
            found = Some((st.tok, span.start, span.end));
            break;
        }
    }

    let settings = ChernSettings::default();
    let path_buf = PathBuf::from(uri);
    let metadata = match ChernConfigLoader::new(
        path_buf.as_path(),
        Cursor::new(text.as_bytes().to_vec()),
        &settings,
    )
    .load_config()
    {
        Ok(m) => m,
        Err(_) => return None,
    };

    let settings = ChernSettings::default();
    let path_buf = PathBuf::from(uri);

    let (hover_text, hover_range) = if let Some((tok, span_start, span_end)) = found {
        match tok {
            ScriptToken::Def => {
                let msg = "**@def** — Starts embedded script block\n\n---\n\n**Example:**\n```chrn\n@def\n    let x = 1\n    var->\n        name: str\n@end\n```".into();
                (msg, Some((span_start, span_end.saturating_add(1))))
            }
            ScriptToken::End => {
                let msg = "**@end** — Ends embedded script block\n\n---\n\n**Example:**\n```chrn\n@end\n// Everything after this is serialized data\n```".into();
                (msg, Some((span_start, span_end.saturating_add(1))))
            }
            ScriptToken::Keyword(kw) => {
                let s = match kw {
                    Keyword::As => "**as** — Aliases imported module names\n\n---\n\n**Example:**\n```chrn\nimport \"module.chrn\" as m\n```".into(),
                    Keyword::Struct => "**struct** — Defines a data structure\n\n---\n\n**Example:**\n```chrn\nstruct Person {\n    name: str\n    age: u8\n}\n```".into(),
                    Keyword::Enum => "**enum** — Defines an enum type\n\n---\n\n**Example:**\n```chrn\nenum Status {\n    Pending\n    Active: Tuple<i32>\n    Completed\n}\n```".into(),
                    Keyword::Import => "**import** — Imports other .chrn files\n\n---\n\n**Example:**\n```chrn\nimport \"definitions.chrn\"\nimport \"utils.chrn\" as u\n```".into(),
                    Keyword::Export => "**export** — Exports types for cross-module use\n\n---\n\n**Example:**\n```chrn\nexport let CONST = 42\nexport struct MyStruct { }\nexport enum MyEnum { }\n```".into(),
                    Keyword::Bind => "**bind** — References external serialized file\n\n---\n\n**Example:**\n```chrn\nbind \"data.chrn\"\n```".into(),
                    Keyword::Alias => "**alias** — Creates reusable predicate functions\n\n---\n\n**Example:**\n```chrn\nalias Positive() = [Range(0.0, 100.0)]\nalias ValidName() = [!IsEmpty, StartsW(\"A\")]\n```".into(),
                    Keyword::Let => "**let** — Declares reusable values\n\n---\n\n**Example:**\n```chrn\nlet count = 10\nlet name = \"test\"\nlet result = value * 2\n```".into(),
                    Keyword::Var => "**var->** — Defines serializable fields section\n\n---\n\n**Example:**\n```chrn\nvar->\n    name: str\n    age: u8 #warn\n    score: f64 [Range(0.0, 100.0)]\n```".into(),
                    Keyword::Nest => "**nest->** — Defines structs and enums section\n\n---\n\n**Example:**\n```chrn\nnest->\n    struct Address {\n        city: str\n        zip: u32\n    }\n    enum Color {Red Blue Green}\n```".into(),
                    Keyword::Change => "**Change** — Unimplemented\n\n---\n\n**Example:**\n```chrn\n// Not yet implemented\n```".into(),
                    Keyword::Complex => "**complex->** — Unimplemented\n\n---\n\n**Example:**\n```chrn\n// Not yet implemented\n```".into(),
                    Keyword::Override => "**override->** — Unimplemented\n\n---\n\n**Example:**\n```chrn\n// Not yet implemented\n```".into(),
                };
                (s, Some((span_start, span_end.saturating_add(1))))
            }
            ScriptToken::Id(id) => {
                let name = PathBuf::from(uri)
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("<unnamed>")
                    .to_string();

                let name_id = InternedId::new(interner.intern(&name));
                let path_id = PathId::new(interner.intern_path(&PathBuf::from(uri)));
                let module = Module::new(name_id, path_id, ModuleId::new(0), Vec::new(), metadata);

                let mut mod_map = std::collections::HashMap::new();
                mod_map.insert(name_id, ModuleId::new(0));
                let mut compiler = ScriptCompiler::new(None, mod_map, vec![module]);

                let mut found_name_id: Option<u32> = None;
                for st in toks.iter() {
                    let span = st.span;
                    if offset >= span.start && offset <= span.end {
                        if let ScriptToken::Id(nid) = st.tok {
                            found_name_id = Some(nid);
                        }
                        break;
                    }
                }

                let mut hover_text = String::new();
                if let Ok(ast_info) =
                    script_lib::parser::parse(&settings, &compiler.mods[0], &toks, &interner)
                {
                    let mut ns_resolver = NamespaceResolver::new(
                        &settings,
                        &ast_info,
                        &interner,
                        ModuleId::new(0),
                        &mut compiler,
                    );

                    if ns_resolver.resolve().is_ok() {
                        let mut ty_ctx = TypeContext::new();
                        let mut type_resolver = TypeResolver::new(
                            &settings,
                            &ast_info,
                            ModuleId::new(0),
                            &mut ty_ctx,
                            &interner,
                            &mut compiler,
                        );

                        if type_resolver.resolve().is_ok() {
                            // Build a lookup map from interned id -> hover text for
                            // struct fields and enum variants so we can answer
                            // hovers in O(1) instead of scanning all types each time.
                            let mut member_map: HashMap<u32, String> = HashMap::new();
                            for ty_info in compiler.types.iter() {
                                match &ty_info.ty {
                                    Type::Struct(sdef) => {
                                        for fld in sdef.fields.iter() {
                                            let field_name =
                                                interner.search(fld.name_id.id as usize);
                                            let field_ty =
                                                &compiler.types[fld.type_id.id as usize].ty;
                                            let field_ty_str =
                                                format_type(field_ty, &compiler, &interner);
                                            member_map.insert(
                                                fld.name_id.id,
                                                format!("{}: {}", field_name, field_ty_str),
                                            );
                                        }
                                    }
                                    Type::Enum(edef) => {
                                        for v in edef.variants.iter() {
                                            let variant_name =
                                                interner.search(v.name_id.id as usize);
                                            if let Some(type_id) = &v.type_id {
                                                let variant_ty =
                                                    &compiler.types[type_id.id as usize].ty;
                                                let variant_ty_str =
                                                    format_type(variant_ty, &compiler, &interner);
                                                member_map.insert(
                                                    v.name_id.id,
                                                    format!("{}: {}", variant_name, variant_ty_str),
                                                );
                                            } else {
                                                member_map
                                                    .insert(v.name_id.id, variant_name.to_string());
                                            }
                                        }
                                    }
                                    _ => {}
                                }
                            }

                            if let Some(nid) = found_name_id {
                                let interned = InternedId::new(nid);
                                if let Some(sym_id) =
                                    compiler.mods[0].get_sym_id(interned, ScopeType::Var)
                                {
                                    if let Some(sym) = compiler.symbols.get(&sym_id) {
                                        match sym.kind {
                                            script_lib::semantic::representation::SymbolKind::Type(type_id) => {
                                                let ty_info = &compiler.types[type_id.id as usize];
                                                let t = format_type(&ty_info.ty, &compiler, &interner);
                                                match &ty_info.ty {
                                                    script_lib::semantic::representation::Type::TypeDef(_) => {
                                                        hover_text = format!("**typedef**: {}", t);
                                                    }
                                                    _ => {
                                                        hover_text = if t.starts_with("struct ")
                                                            || t.starts_with("enum ")
                                                            || t.starts_with("alias ")
                                                        {
                                                            t
                                                        } else {
                                                            format!("type: {}", t)
                                                        };
                                                    }
                                                }
                                            }
                                            script_lib::semantic::representation::SymbolKind::Val(val_id) => {
                                                let val_info = &compiler.values[val_id.id as usize];
                                                let ty_info = &compiler.types[val_info.type_id.id as usize];

                                                let var_name = interner.search(sym.name_id.id as usize);
                                                let type_str = format_type(&ty_info.ty, &compiler, &interner);
                                                let val_str = match &val_info.const_val {
                                                    Some(v) => format_value(v, &interner),
                                                    None => "unknown".to_string(),
                                                };

                                                hover_text = format!("{}: {} = {}", var_name, type_str, val_str);
                                            }
                                            script_lib::semantic::representation::SymbolKind::Unknown => {
                                                hover_text = "unknown".to_string();
                                            }
                                        }
                                    }
                                }

                                // If we still don't have hover info, check the
                                // precomputed member_map for fields/variants.
                                if hover_text.is_empty() {
                                    if let Some(s) = member_map.get(&nid) {
                                        hover_text = s.clone();
                                    }
                                }
                            }
                        }
                    }
                }

                if hover_text.is_empty() {
                    let s = interner.search(id as usize);
                    hover_text = hover::lookup_hover(&s);
                }

                (hover_text, Some((span_start, span_end.saturating_add(1))))
            }
            ScriptToken::Str(id) => {
                let s = interner.search(id as usize);
                (
                    format!("string literal: \"{}\"", s),
                    Some((span_start, span_end.saturating_add(1))),
                )
            }
            ScriptToken::Integer(id, _) => {
                let s = interner.search(id as usize);
                (
                    format!("Integer literal: {}", s),
                    Some((span_start, span_end.saturating_add(1))),
                )
            }
            ScriptToken::Float(id, _) => {
                let s = interner.search(id as usize);
                (
                    format!("Float literal: {}", s),
                    Some((span_start, span_end.saturating_add(1))),
                )
            }
            ScriptToken::BoolLiteral(b) => (
                format!("bool literal: {}", b),
                Some((span_start, span_end.saturating_add(1))),
            ),
            ScriptToken::Char(c) => (
                format!("char literal: '{}'", c),
                Some((span_start, span_end.saturating_add(1))),
            ),
            ScriptToken::At => (
                "**@** — Directive marker (e.g. @def/@end)".into(),
                Some((span_start, span_end.saturating_add(1))),
            ),
            ScriptToken::HashSymbol => (
                "**#** — Argument prefix (#warn/#ignore)".into(),
                Some((span_start, span_end.saturating_add(1))),
            ),
            ScriptToken::SlimArrow => (
                "**->** — Section declaration operator".into(),
                Some((span_start, span_end.saturating_add(1))),
            ),
            _ => (
                String::new(),
                Some((span_start, span_end.saturating_add(1))),
            ),
        }
    } else {
        if offset >= text.len() {
            return None;
        }
        let (start_b, end_b) = crate::text::find_word_bounds(text, offset);
        let line = text.lines().nth(pos.line as usize).unwrap_or("");
        let byte_idx = pos.character as usize;
        let word = extract_word_at(line, byte_idx);

        if word.is_empty() {
            return None;
        }

        (hover::lookup_hover(&word), Some((start_b, end_b)))
    };

    if hover_text.is_empty() {
        return None;
    }

    let contents = HoverContents::Markup(MarkupContent {
        value: hover_text,
        kind: MarkupKind::Markdown,
    });
    let hover = Hover {
        contents,
        range: hover_range.and_then(|(s, e)| {
            let start_pos = crate::text::offset_to_position(text, s);
            let end_pos = crate::text::offset_to_position(text, e);
            Some(Range {
                start: start_pos,
                end: end_pos,
            })
        }),
    };
    Some(hover)
}

pub fn lookup_hover(token: &str) -> String {
    match token {
        "@def" => "**@def** — Starts embedded script block\n\n---\n\n**Example:**\n```chrn\n@def\n    let x = 1\n@end\n```".into(),
        "@end" => "**@end** — Ends embedded script block\n\n---\n\n**Example:**\n```chrn\n@end\n// serialized data\n```".into(),
        "bind" => "**bind** — References external serialized file\n\n---\n\n**Example:**\n```chrn\nbind \"data.chrn\"\n```".into(),
        "var->" => "**var->** — Defines serializable fields section\n\n---\n\n**Example:**\n```chrn\nvar->\n    name: str\n    age: u8\n```".into(),
        "nest->" => "**nest->** — Defines structs and enums section\n\n---\n\n**Example:**\n```chrn\nnest->\n    struct Person { }\n    enum Status { }\n```".into(),
        "import" => "**import** — Imports other .chrn files\n\n---\n\n**Example:**\n```chrn\nimport \"module.chrn\"\n```".into(),
        "export" => "**export** — Exports values/functions for import\n\n---\n\n**Example:**\n```chrn\nexport let X = 1\n```".into(),
        "alias" => "**alias** — Creates predicate reusable functions\n\n---\n\n**Example:**\n```chrn\nalias Positive() = [Range(0, 100)]\n```".into(),
        "let" => "**let** — Declares a variable to store a value\n\n---\n\n**Example:**\n```chrn\nlet x = 42\nlet name = \"test\"\n```".into(),
        "struct" => "**struct** — Defines a structure of data\n\n---\n\n**Example:**\n```chrn\nstruct Point {\n    x: i32\n    y: i32\n}\n```".into(),
        "enum" => "**enum** — Defines an enum type\n\n---\n\n**Example:**\n```chrn\nenum Color {\n    Red\n    Blue\n    Green\n}\n```".into(),
        "List" => "**List<T>** — Generic list type\n\n---\n\n**Example:**\n```chrn\nitems: List<Item>\n```".into(),
        "Set" => "**Set<T>** — Generic set type\n\n---\n\n**Example:**\n```chrn\ntags: Set<Tag>\n```".into(),
        "Map" => "**Map<K, V>** — Generic map type\n\n---\n\n**Example:**\n```chrn\nlookup: Map<str, Value>\n```".into(),
        "Tuple" => "**Tuple<A, B, ...>** — Generic tuple type\n\n---\n\n**Example:**\n```chrn\ncoord: Tuple<i32, i32>\n```".into(),
        "?" => "**?** — Type inference placeholder\n\n---\n\n**Example:**\n```chrn\nvalue: ?  // infers and enforces type\n```".into(),
        "#warn" => "**#warn** — Treat as warning instead of error\n\n---\n\n**Example:**\n```chrn\nfield: str #warn\n```".into(),
        "#ignore" => "**#ignore** — Ignore type errors\n\n---\n\n**Example:**\n```chrn\nfield: ? #ignore\n```".into(),
        _ => String::new(),
    }
}

fn format_type(
    ty: &script_lib::semantic::representation::Type,
    compiler: &ScriptCompiler,
    interner: &Intern,
) -> String {
    match ty {
        Type::BuiltinType(builtin_ty) => match builtin_ty {
            chrn_utils::builtins::BuiltinType::List(type_id) => {
                let inner = &compiler.types[type_id.id as usize].ty;
                format!("List<{}>", format_type(inner, compiler, interner))
            }
            chrn_utils::builtins::BuiltinType::Set(type_id) => {
                let inner = &compiler.types[type_id.id as usize].ty;
                format!("Set<{}>", format_type(inner, compiler, interner))
            }
            chrn_utils::builtins::BuiltinType::Map(kid, vid) => {
                let k = &compiler.types[kid.id as usize].ty;
                let v = &compiler.types[vid.id as usize].ty;
                format!(
                    "Map<{}, {}>",
                    format_type(k, compiler, interner),
                    format_type(v, compiler, interner)
                )
            }
            chrn_utils::builtins::BuiltinType::Tuple(type_ids) => {
                let elems: Vec<String> = type_ids
                    .iter()
                    .map(|type_id| {
                        let ty = &compiler.types[type_id.id as usize].ty;
                        format_type(ty, compiler, interner)
                    })
                    .collect();
                format!("Tuple<{}>", elems.join(", "))
            }
            chrn_utils::builtins::BuiltinType::Any(opt_type_id) => match opt_type_id {
                Some(type_id) => {
                    let ty = &compiler.types[type_id.id as usize].ty;
                    format!("Any<{}>", format_type(ty, compiler, interner))
                }
                None => "Any".into(),
            },
            b => b.kind().to_fmt().to_string(),
        },
        Type::Struct(struct_def) => {
            let name = compiler
                .symbols
                .get(&struct_def.sym_id)
                .map(|sym| interner.search(sym.name_id.id as usize))
                .unwrap_or("<struct>".into());

            if struct_def.fields.is_empty() {
                format!("struct {} {{}}", name)
            } else {
                let fields: Vec<String> = struct_def
                    .fields
                    .iter()
                    .map(|field| {
                        let field_name = interner.search(field.name_id.id as usize);
                        let field_ty = &compiler.types[field.type_id.id as usize].ty;
                        let field_ty_str = match field_ty {
                            Type::Struct(sdef) => compiler
                                .symbols
                                .get(&sdef.sym_id)
                                .map(|sym| interner.search(sym.name_id.id as usize))
                                .unwrap_or("<struct>".into()),
                            Type::Enum(edef) => compiler
                                .symbols
                                .get(&edef.sym_id)
                                .map(|sym| interner.search(sym.name_id.id as usize))
                                .unwrap_or("<enum>".into()),
                            _ => &format_type(field_ty, compiler, interner),
                        };

                        format!("\t{}: {}", field_name, field_ty_str)
                    })
                    .collect();
                format!("struct {} {{\n{}\n}}", name, fields.join("\n"))
            }
        }
        Type::Enum(enum_def) => {
            let name = compiler
                .symbols
                .get(&enum_def.sym_id)
                .map(|sym| interner.search(sym.name_id.id as usize))
                .unwrap_or("<enum>".into());

            if enum_def.variants.is_empty() {
                format!("enum {} {{}}", name)
            } else {
                let variants: Vec<String> = enum_def
                    .variants
                    .iter()
                    .map(|v| {
                        let variant_name = interner.search(v.name_id.id as usize);

                        if let Some(type_id) = &v.type_id {
                            let variant_ty = &compiler.types[type_id.id as usize].ty;
                            let variant_ty_str = match variant_ty {
                                Type::Struct(sdef) => compiler
                                    .symbols
                                    .get(&sdef.sym_id)
                                    .map(|sym| interner.search(sym.name_id.id as usize))
                                    .unwrap_or("<struct>".into()),
                                Type::Enum(edef) => compiler
                                    .symbols
                                    .get(&edef.sym_id)
                                    .map(|sym| interner.search(sym.name_id.id as usize))
                                    .unwrap_or("<enum>".into()),
                                _ => &format_type(variant_ty, compiler, interner),
                            };
                            format!("\t{}: {}", variant_name, variant_ty_str)
                        } else {
                            variant_name.to_string()
                        }
                    })
                    .collect();
                format!("enum {} {{\n{}\n}}", name, variants.join("\n"))
            }
        }
        Type::Func(_) => "function type".into(),
        Type::Alias(_) => "alias type".into(),
        Type::TypeDef(type_def) => {
            let inner = &compiler.types[type_def.type_id.id as usize].ty;
            format_type(inner, compiler, interner)
        }
        Type::Unknown => "unknown".into(),
    }
}

fn format_value(v: &CValue, interner: &Intern) -> String {
    match v {
        CValue::I64(num) => format!("{}", num),
        CValue::F64(num) => format!("{}", num),
        CValue::Bool(boolean) => format!("{}", boolean),
        CValue::Char(c) => format!("'{}'", c),
        CValue::Tuple(elems) => {
            let parts: Vec<String> = elems.iter().map(|ev| format_value(ev, interner)).collect();
            format!("({})", parts.join(", "))
        }
        CValue::InternedStr(id) => {
            format!("\"{}\"", interner.search(id.id as usize).to_string())
        }
        CValue::RuntimeStr(s) => format!("\"{s}\""),
        CValue::Unknown => "unknown".into(),
    }
}
