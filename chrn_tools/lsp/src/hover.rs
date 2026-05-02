use std::sync::Arc;

use chrn_utils::id_types::InternedId;
use chrn_utils::intern::Intern;
use chrn_utils::values::Value as CValue;
use common::fmter::Formattable;
use script_lib::script_compiler::ScriptCompiler;
use script_lib::semantic::representation::Type;
use script_lib::semantic::scopes::ScopeType;
use script_lib::token::Token as ScriptToken;
use tower_lsp::lsp_types::*;

use crate::document::{self, Document};
use crate::text::position_to_offset;

use parking_lot::RwLock;

pub fn compute_hover(
    _uri: &tower_lsp::lsp_types::Url,
    pos: Position,
    state_arc: Arc<RwLock<crate::state::DocumentState>>,
) -> Option<Hover> {
    let state = state_arc.read();
    let text = &state.text;
    let offset = position_to_offset(text, pos);

    let (tok, span_start, span_end) = match state.get_symbol_at_offset(offset) {
        Some(res) => (ScriptToken::Id(res.0), res.1, res.2),
        None => {
            // Check for non-identifier tokens
            let mut found = None;
            for st in &state.tokens {
                let span = st.span;
                if offset >= span.start && offset <= span.end {
                    found = Some((st.tok, span.start, span.end));
                    break;
                }
            }
            match found {
                Some(f) => f,
                None => return None,
            }
        }
    };
    let (hover_text, hover_range): (String, Option<(usize, usize)>) = match tok {
        ScriptToken::Def => {
            let msg = format!(
                "**@def** — Starts embedded script block\n\n{}\n\n**Example:**\n```chrn\n@def\n    let x = 1\n    var->\n        name: str\n@end\n```",
                document::HOVER_DASHES
            );

            (msg, Some((span_start, span_end.saturating_add(1))))
        }
        ScriptToken::End => {
            let msg = format!(
                "**@end** — Ends embedded script block\n\n{}\n\n**Example:**\n```chrn\n@end\n// Everything after this is serialized data\n```",
                document::HOVER_DASHES
            );

            (msg, Some((span_start, span_end.saturating_add(1))))
        }
        ScriptToken::Keyword(kw) => {
            let doc = Document::keyword_docs(kw);
            (
                doc.compose(),
                Some((span_start, span_end.saturating_add(1))),
            )
        }
        ScriptToken::Id(id) => {
            let mut hover_text = String::new();
            let compiler = match &state.compiler {
                Some(c) => c,
                None => return None,
            };
            let interner = &state.interner;

            let nid = id;
            let interned = InternedId::new(nid);
            if let Some(sym_id) = compiler.mods[0].get_sym_id(interned, ScopeType::Var) {
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
                                    let is_struct_or_enum =
                                        t.starts_with("struct ") || t.starts_with("enum ");
                                    let is_alias = t.starts_with("alias ");

                                    let export_prefix = if !sym.is_priv && is_struct_or_enum {
                                        "export "
                                    } else {
                                        ""
                                    };

                                    let mut final_text = String::new();
                                    if is_struct_or_enum || is_alias {
                                        let module = &compiler.mods[sym.owner.id];
                                        let raw_mod_name =
                                            interner.search(module.name_id.id as usize);
                                        let mod_name = raw_mod_name
                                            .strip_suffix(".chrn")
                                            .unwrap_or(raw_mod_name);
                                        final_text
                                            .push_str(&format!("module: **{}**\n\n", mod_name));
                                        final_text.push_str(&format!("{}{}", export_prefix, t));
                                    } else {
                                        final_text.push_str(&format!("type: {}", t));
                                    }
                                    hover_text = final_text;
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
                            hover_text = "Unknown".to_string();
                        }
                    }

                    if !hover_text.is_empty() {
                        let privacy = if sym.is_priv { "private" } else { "public" };
                        hover_text.push_str(&format!(
                            "\n\n{}\n\n{} | **Scope:** {}",
                            document::HOVER_DASHES,
                            privacy,
                            sym.scope_type
                        ));
                    }
                }
            }

            if hover_text.is_empty() && state.member_ids.contains(&id) {
                for ty_info in &compiler.types {
                    let mut found_exact = false;
                    match &ty_info.ty {
                        script_lib::semantic::representation::Type::Struct(struct_def) => {
                            for field in &struct_def.fields {
                                if field.name_id.id == id {
                                    let mut is_def = false;
                                    if let Some(owner) = ty_info.owner {
                                        if let Some(ast) = state.asts.get(owner.id) {
                                            let span = ast.get_sym_span(field.ast_id);
                                            if span_start >= span.start && span_start <= span.end {
                                                is_def = true;
                                            }
                                        }
                                    }

                                    if is_def || hover_text.is_empty() {
                                        let field_name = interner.search(field.name_id.id as usize);
                                        let field_ty =
                                            &compiler.types[field.type_id.id as usize].ty;
                                        let type_str = format_type(field_ty, compiler, interner);
                                        hover_text = format!("{}: {}", field_name, type_str);
                                        if is_def {
                                            found_exact = true;
                                            break;
                                        }
                                    }
                                }
                            }
                        }
                        script_lib::semantic::representation::Type::Enum(enum_def) => {
                            for variant in &enum_def.variants {
                                if variant.name_id.id == id {
                                    let mut is_def = false;
                                    if let Some(owner) = ty_info.owner {
                                        if let Some(ast) = state.asts.get(owner.id) {
                                            let span = ast.get_sym_span(variant.ast_id);
                                            if span_start >= span.start && span_start <= span.end {
                                                is_def = true;
                                            }
                                        }
                                    }

                                    if is_def || hover_text.is_empty() {
                                        let variant_name =
                                            interner.search(variant.name_id.id as usize);
                                        if let Some(tid) = variant.type_id {
                                            let variant_ty = &compiler.types[tid.id as usize].ty;
                                            let type_str =
                                                format_type(variant_ty, compiler, interner);
                                            hover_text = format!("{}: {}", variant_name, type_str);
                                        } else {
                                            hover_text = variant_name.to_string();
                                        }
                                        if is_def {
                                            found_exact = true;
                                            break;
                                        }
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                    if found_exact {
                        break;
                    }
                }
            }

            if hover_text.is_empty() {
                if let Some(mod_id) = compiler.mod_map.get(&interned) {
                    let module = &compiler.mods[mod_id.id];

                    // Alias checking for display specifics
                    let alias_mention = if interned != module.name_id {
                        let alias_name = interner.search(interned.id as usize);
                        // Questionable markdown display behavior when parenthesis are used
                        format!("alias **{alias_name}**|")
                    } else {
                        "".to_string()
                    };

                    let raw_mod_name = interner.search(module.name_id.id as usize);
                    // Something is happening inside of mod_finder
                    let mod_name = raw_mod_name.strip_suffix(".chrn").unwrap_or(raw_mod_name);
                    let mod_path = interner.search_path(module.path_id.id as usize).display();

                    hover_text = format!(
                        "{}module **{}**\n{}\npath: `{}`",
                        alias_mention,
                        mod_name,
                        document::HOVER_DASHES,
                        mod_path
                    );
                }
            }

            if hover_text.is_empty() {
                let s = interner.search(id as usize);
                hover_text = lookup_hover(&s);
            }

            (hover_text, Some((span_start, span_end.saturating_add(1))))
        }
        ScriptToken::Str(id) => {
            let s = state.interner.search(id as usize);
            (
                format!("string literal: \"{}\"", s),
                Some((span_start, span_end.saturating_add(1))),
            )
        }
        ScriptToken::Integer(id, _) => {
            let s = state.interner.search(id as usize);
            (
                format!("Integer literal: {}", s),
                Some((span_start, span_end.saturating_add(1))),
            )
        }
        ScriptToken::Float(id, _) => {
            let s = state.interner.search(id as usize);
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
    if let Some(doc) = Document::lookup(token) {
        return doc.compose();
    }
    String::new()
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
                                .map(|sym| {
                                    format!(
                                        "struct {} {{}}",
                                        interner.search(sym.name_id.id as usize)
                                    )
                                })
                                .unwrap_or("<struct>".into()),
                            Type::Enum(edef) => compiler
                                .symbols
                                .get(&edef.sym_id)
                                .map(|sym| {
                                    format!(
                                        "enum {} {{}}",
                                        interner.search(sym.name_id.id as usize)
                                    )
                                })
                                .unwrap_or("<enum>".into()),
                            _ => format_type(field_ty, compiler, interner),
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
                                    .map(|sym| {
                                        format!(
                                            "struct {} {{}}",
                                            interner.search(sym.name_id.id as usize)
                                        )
                                    })
                                    .unwrap_or("<struct>".into()),
                                Type::Enum(edef) => compiler
                                    .symbols
                                    .get(&edef.sym_id)
                                    .map(|sym| {
                                        format!(
                                            "enum {} {{}}",
                                            interner.search(sym.name_id.id as usize)
                                        )
                                    })
                                    .unwrap_or("<enum>".into()),
                                _ => format_type(variant_ty, compiler, interner),
                            };
                            format!("\t{}: {}", variant_name, variant_ty_str)
                        } else {
                            format!("\t{}", variant_name)
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
