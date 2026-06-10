use chrn_utils::intern::Intern;
use compilation::script_compiler::ScriptCompiler;
use compilation::semantic::hir::{SymbolKind, Type, VariableState};
use compilation::token::Token as ScriptToken;
use lang::fmter::Formattable;
use lang::types::builtins::{BuiltinType, BuiltinTypeKind};
use lang::types::type_constraints::TypeConstraint;
use lang::values::Value;
use tower_lsp::lsp_types;

use crate::document::{self, Document};
use crate::text::position_to_offset;

pub fn compute_hover(
    _uri: &tower_lsp::lsp_types::Url,
    pos: lsp_types::Position,
    state: &crate::state::DocumentState,
) -> Option<lsp_types::Hover> {
    let text = &state.text;
    let offset = position_to_offset(text, pos);

    if state.offset_in_comment(offset) {
        return None;
    }

    let (tok, span_start, span_end) = match state.get_symbol_at_offset(offset) {
        Some(res) => (ScriptToken::Id(res.0), res.1, res.2),
        None => {
            // Check for non-identifier tokens
            let mut found = None;
            for st in &state.tokens {
                let span = st.span;
                if offset >= span.start as usize && offset <= span.end as usize {
                    found = Some((st.tok, span.start as usize, span.end as usize));
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
            let interned = id;

            use crate::state::SemanticEntity;
            let entity = state.get_entity_at_offset(offset);

            if let Some(entity) = entity {
                match entity {
                    SemanticEntity::Symbol(sym_id) => {
                        if let Some(sym) = compiler.symbols.get(sym_id.id as usize) {
                            match sym.kind {
                                SymbolKind::Type(type_id) => {
                                    let ty_info = &compiler.types[type_id.id as usize];
                                    let t = format_type(&ty_info.ty, &compiler, &interner, false);
                                    match &ty_info.ty {
                                        compilation::semantic::hir::Type::TypeDef(type_def) => {
                                            let inner =
                                                &compiler.types[type_def.type_id.id as usize].ty;
                                            let shallow_t = strip_struct_enum_prefix(&format_type(
                                                inner, &compiler, &interner, true,
                                            ));
                                            hover_text = format!("**typedef**: {}", shallow_t);
                                        }
                                        _ => {
                                            if let compilation::semantic::hir::Type::BuiltinType(
                                                builtin,
                                            ) = &ty_info.ty
                                            {
                                                hover_text =
                                                    Document::builtin_type_docs(builtin.kind())
                                                        .compose();
                                            } else if let compilation::semantic::hir::Type::Func(
                                                func_def,
                                            ) = &ty_info.ty
                                            {
                                                hover_text =
                                                    Document::func_docs(func_def.kind).compose();
                                            } else {
                                                let is_struct_or_enum = t.starts_with("struct ")
                                                    || t.starts_with("enum ");
                                                let is_alias = t.starts_with("alias ");

                                                let export_prefix = if !sym.is_priv
                                                    && (is_struct_or_enum || is_alias)
                                                {
                                                    "export "
                                                } else {
                                                    ""
                                                };

                                                let mut final_text = String::new();
                                                if is_struct_or_enum || is_alias {
                                                    let module =
                                                        &compiler.mods[sym.owner.id as usize];
                                                    let raw_mod_name =
                                                        interner.search(module.name_id);
                                                    final_text.push_str(&format!(
                                                        "module: **{}**\n\n",
                                                        raw_mod_name
                                                    ));
                                                    final_text.push_str(&format!(
                                                        "{}{}",
                                                        export_prefix, t
                                                    ));
                                                } else {
                                                    final_text.push_str(&format!("type: {}", t));
                                                }
                                                hover_text = final_text;
                                            }
                                        }
                                    }
                                }
                                SymbolKind::Variable(var_id) => {
                                    let var = &compiler.variables[var_id.id as usize];
                                    match var.state {
                                        VariableState::Known(val_id) => {
                                            let val_info = &compiler.values[val_id.id as usize];
                                            let ty_info =
                                                &compiler.types[val_info.type_id.id as usize];

                                            let var_name = interner.search(sym.name_id);
                                            let type_str = strip_struct_enum_prefix(&format_type(
                                                &ty_info.ty,
                                                &compiler,
                                                &interner,
                                                true,
                                            ));
                                            let val_str = match &val_info.const_val {
                                                Some(v) => format_value(v, &interner),
                                                None => "unknown".to_string(),
                                            };

                                            hover_text =
                                                format!("{}: {} = {}", var_name, type_str, val_str);
                                        }
                                        VariableState::ReservedTypeSlot(_) => {
                                            hover_text = "Unknown".to_string();
                                        }
                                    }
                                }
                                SymbolKind::Module(mod_id) => {
                                    let module = &compiler.mods[mod_id.id as usize];
                                    let mod_name = interner.search(module.name_id);
                                    hover_text = format!("module **{}**", mod_name);
                                }
                                SymbolKind::Config(config_id) => todo!(),
                            }

                            if !hover_text.is_empty() {
                                let privacy = if sym.is_priv { "private" } else { "public" };
                                hover_text.push_str(&format!(
                                    "\n\n{}\n\n{} | **Scope:** {}",
                                    document::HOVER_DASHES,
                                    privacy,
                                    sym.scope_origin
                                ));
                            }
                        }
                    }
                    SemanticEntity::Field {
                        owner_sym_id,
                        field_idx,
                    } => {
                        if let Some(sym) = compiler.symbols.get(owner_sym_id.id as usize) {
                            if let Some(ast_id) = sym.ast_id {
                                if let Some(Some(ast)) = state.asts.get(sym.owner.id as usize) {
                                    let abs_struct = ast.get_struct(ast_id);
                                    if let Some(field) = abs_struct.fields.get(*field_idx) {
                                        let field_name = interner.search(field.name_id);
                                        // We need the resolved type from the compiler, not just AST
                                        if let SymbolKind::Type(tid) = sym.kind {
                                            if let Type::Struct(sdef) =
                                                &compiler.types[tid.id as usize].ty
                                            {
                                                if let Some(member_id) = sdef.fields.get(*field_idx)
                                                {
                                                    if let Some(compilation::semantic::hir::MemberSymbolKind::Field(field_repre)) = compiler.members.get(member_id.id as usize) {
                                                        let type_str = strip_struct_enum_prefix(&format_type(
                                                            &compiler.types[field_repre.type_id.id as usize].ty,
                                                            compiler,
                                                            interner,
                                                            true,
                                                        ));
                                                        hover_text = format!("{}: {}", field_name, type_str);
                                                    }
                                                }
                                            }
                                        }
                                        if hover_text.is_empty() {
                                            // Fallback to name-only if type resolution failed
                                            hover_text = format!("{}: Unknown", field_name);
                                        }
                                    }
                                }
                            }
                        }
                    }
                    SemanticEntity::Variant {
                        owner_sym_id,
                        variant_idx,
                    } => {
                        if let Some(sym) = compiler.symbols.get(owner_sym_id.id as usize) {
                            if let Some(ast_id) = sym.ast_id {
                                if let Some(Some(ast)) = state.asts.get(sym.owner.id as usize) {
                                    let abs_enum = ast.get_enum(ast_id);
                                    if let Some(variant) = abs_enum.variants.get(*variant_idx) {
                                        let variant_name = interner.search(variant.name_id);
                                        if let SymbolKind::Type(tid) = sym.kind {
                                            if let Type::Enum(edef) =
                                                &compiler.types[tid.id as usize].ty
                                            {
                                                if let Some(member_id) =
                                                    edef.variants.get(*variant_idx)
                                                {
                                                    if let Some(compilation::semantic::hir::MemberSymbolKind::Variant(variant_repre)) = compiler.members.get(member_id.id as usize) {
                                                        if let Some(vty_id) = variant_repre.type_id {
                                                            let type_str = strip_struct_enum_prefix(&format_type(
                                                                &compiler.types[vty_id.id as usize].ty,
                                                                compiler,
                                                                interner,
                                                                true,
                                                            ));
                                                            hover_text = format!("{}: {}", variant_name, type_str);
                                                        } else {
                                                            hover_text = variant_name.to_string();
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                        if hover_text.is_empty() {
                                            hover_text = variant_name.to_string();
                                        }
                                    }
                                }
                            }
                        }
                    }
                    SemanticEntity::Module(mod_id) => {
                        let module = &compiler.mods[mod_id.id];
                        let mod_name = interner.search(module.name_id);
                        let mod_path = if let Some(region_id) = module.region_id {
                            if let Some(region) = state.region_arena.get_region(region_id) {
                                interner.search_path(region.path_id).display().to_string()
                            } else {
                                "<builtin>".to_string()
                            }
                        } else {
                            "<builtin>".to_string()
                        };

                        let alias_prefix = compiler.mods[0]
                            .imports
                            .iter()
                            .find_map(|i| {
                                i.alias_id
                                    .filter(|a| *a == interned)
                                    .map(|a| format!("alias **{}** | ", interner.search(a)))
                            })
                            .unwrap_or_default();

                        hover_text = format!(
                            "{}module **{}**\n{}\npath: `{}`",
                            alias_prefix,
                            mod_name,
                            document::HOVER_DASHES,
                            mod_path
                        );
                    }
                    SemanticEntity::Local { name_id, .. } => {
                        let name = interner.search(*name_id);
                        hover_text = format!("{name}: (param)");
                    }
                }
            }

            // Fallback to name-based lookup for modules or builtins
            if hover_text.is_empty() {
                if let Some(module) = compiler.mods.iter().find(|m| m.name_id == interned) {
                    let raw_mod_name = interner.search(module.name_id);
                    let mod_path = if let Some(region_id) = module.region_id {
                        if let Some(region) = state.region_arena.get_region(region_id) {
                            interner.search_path(region.path_id).display().to_string()
                        } else {
                            "<builtin>".to_string()
                        }
                    } else {
                        "<builtin>".to_string()
                    };

                    let alias_prefix = compiler.mods[0]
                        .imports
                        .iter()
                        .find_map(|i| {
                            i.alias_id
                                .filter(|a| *a == interned)
                                .map(|a| format!("alias **{}** | ", interner.search(a)))
                        })
                        .unwrap_or_default();

                    hover_text = format!(
                        "{}module **{}**\n{}\npath: `{}`",
                        alias_prefix,
                        raw_mod_name,
                        document::HOVER_DASHES,
                        mod_path
                    );
                }
            }

            if hover_text.is_empty() {
                if let Some(kind) = BuiltinTypeKind::try_from_interned_id(id.id as u32) {
                    hover_text = Document::builtin_type_docs(kind).compose();
                }
            }

            (hover_text, Some((span_start, span_end.saturating_add(1))))
        }
        ScriptToken::Str(id) => {
            let s = state.interner.search(id);
            (
                format!("string literal: \"{}\"", s),
                Some((span_start, span_end.saturating_add(1))),
            )
        }
        ScriptToken::Integer(id, _) => {
            let s = state.interner.search(id);
            (
                format!("Integer literal: {}", s),
                Some((span_start, span_end.saturating_add(1))),
            )
        }
        ScriptToken::Float(id, _) => {
            let s = state.interner.search(id);
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

    let contents = lsp_types::HoverContents::Markup(lsp_types::MarkupContent {
        value: hover_text,
        kind: lsp_types::MarkupKind::Markdown,
    });
    let hover = lsp_types::Hover {
        contents,
        range: hover_range.and_then(|(s, e)| {
            let start_pos = crate::text::offset_to_position(text, s);
            let end_pos = crate::text::offset_to_position(text, e);
            Some(lsp_types::Range {
                start: start_pos,
                end: end_pos,
            })
        }),
    };
    Some(hover)
}

fn format_type(ty: &Type, compiler: &ScriptCompiler, interner: &Intern, shallow: bool) -> String {
    match ty {
        Type::BuiltinType(builtin_ty) => match builtin_ty {
            BuiltinType::List(type_id) => {
                let inner = &compiler.types[type_id.id as usize].ty;
                format!(
                    "List<{}>",
                    strip_struct_enum_prefix(&format_type(inner, compiler, interner, true))
                )
            }
            BuiltinType::Set(type_id) => {
                let inner = &compiler.types[type_id.id as usize].ty;
                format!(
                    "Set<{}>",
                    strip_struct_enum_prefix(&format_type(inner, compiler, interner, true))
                )
            }
            BuiltinType::Map(kid, vid) => {
                let k = &compiler.types[kid.id as usize].ty;
                let v = &compiler.types[vid.id as usize].ty;
                format!(
                    "Map<{}, {}>",
                    strip_struct_enum_prefix(&format_type(k, compiler, interner, true)),
                    strip_struct_enum_prefix(&format_type(v, compiler, interner, true))
                )
            }
            BuiltinType::Tuple(type_ids) => {
                let elems: Vec<String> = type_ids
                    .iter()
                    .map(|type_id| {
                        let ty = &compiler.types[type_id.id as usize].ty;
                        strip_struct_enum_prefix(&format_type(ty, compiler, interner, true))
                    })
                    .collect();
                format!("Tuple<{}>", elems.join(", "))
            }
            BuiltinType::Runtime => "Runtime".into(),
            b => b.kind().to_fmt().to_string(),
        },
        Type::Struct(struct_def) => {
            let name = compiler
                .symbols
                .get(struct_def.sym_id.id as usize)
                .map(|sym| interner.search(sym.name_id))
                .unwrap_or("<struct>".into());

            if shallow {
                return format!("struct {}", name);
            }

            if struct_def.fields.is_empty() {
                format!("struct {} {{}}", name)
            } else {
                let fields: Vec<String> = struct_def
                    .fields
                    .iter()
                    .filter_map(
                        |member_id| match compiler.members.get(member_id.id as usize)? {
                            compilation::semantic::hir::MemberSymbolKind::Field(field) => {
                                let field_name = interner.search(field.name_id);
                                let field_ty = &compiler.types[field.type_id.id as usize].ty;
                                let field_ty_str = strip_struct_enum_prefix(&format_type(
                                    field_ty, compiler, interner, true,
                                ));
                                Some(format!("\t{}: {}", field_name, field_ty_str))
                            }
                            _ => None,
                        },
                    )
                    .collect();
                format!("struct {} {{\n{}\n}}", name, fields.join("\n"))
            }
        }
        Type::Enum(enum_def) => {
            let name = compiler
                .symbols
                .get(enum_def.sym_id.id as usize)
                .map(|sym| interner.search(sym.name_id))
                .unwrap_or("<enum>".into());

            if shallow {
                return format!("enum {}", name);
            }

            if enum_def.variants.is_empty() {
                format!("enum {} {{}}", name)
            } else {
                let variants: Vec<String> = enum_def
                    .variants
                    .iter()
                    .filter_map(
                        |member_id| match compiler.members.get(member_id.id as usize)? {
                            compilation::semantic::hir::MemberSymbolKind::Variant(v) => {
                                let variant_name = interner.search(v.name_id);

                                if let Some(type_id) = v.type_id {
                                    let variant_ty = &compiler.types[type_id.id as usize].ty;
                                    let variant_ty_str = strip_struct_enum_prefix(&format_type(
                                        variant_ty, compiler, interner, true,
                                    ));
                                    Some(format!("\t{}: {}", variant_name, variant_ty_str))
                                } else {
                                    Some(format!("\t{}", variant_name))
                                }
                            }
                            _ => None,
                        },
                    )
                    .collect();
                format!("enum {} {{\n{}\n}}", name, variants.join("\n"))
            }
        }
        Type::Func(_) => "function type".into(),
        Type::Alias(alias_def) => {
            let name = compiler
                .symbols
                .get(alias_def.sym_id.id as usize)
                .map(|sym| interner.search(sym.name_id))
                .unwrap_or("<alias>".into());

            if shallow {
                return format!("alias {}", name);
            }

            let params: Vec<String> = alias_def
                .params
                .iter()
                .map(|p| {
                    let p_name = compiler
                        .symbols
                        .get(p.sym_id.id as usize)
                        .map(|sym| interner.search(sym.name_id))
                        .unwrap_or("<param>");
                    let p_constraint = alias_def
                        .ty_constraints
                        .to_type_constraint_vec()
                        .iter()
                        .map(|c: &TypeConstraint| c.to_fmt().to_string())
                        .collect::<Vec<_>>()
                        .join(" | ");
                    format!("{}: {}", p_name, p_constraint)
                })
                .collect();

            format!("alias {}({})", name, params.join(", "))
        }
        Type::TypeDef(type_def) => {
            let inner = &compiler.types[type_def.type_id.id as usize].ty;
            format_type(inner, compiler, interner, shallow)
        }
        Type::Constrained(flags) => flags
            .to_type_constraint_vec()
            .iter()
            .map(|c: &TypeConstraint| c.to_fmt().to_string())
            .collect::<Vec<_>>()
            .join(" | "),
        Type::Deferred(type_id) => {
            let inner = &compiler.types[type_id.id as usize].ty;
            format_type(inner, compiler, interner, shallow)
        }
        Type::Unknown => "Unknown".into(),
    }
}

fn strip_struct_enum_prefix(s: &str) -> String {
    if let Some(stripped) = s.strip_prefix("struct ") {
        stripped.to_string()
    } else if let Some(stripped) = s.strip_prefix("enum ") {
        stripped.to_string()
    } else {
        s.to_string()
    }
}

fn format_value(v: &Value, interner: &Intern) -> String {
    match v {
        Value::I64(num) => format!("{}", num),
        Value::F64(num) => format!("{}", num),
        Value::Bool(boolean) => format!("{}", boolean),
        Value::Char(c) => format!("'{}'", c),
        Value::Tuple(elems) => {
            let parts: Vec<String> = elems.iter().map(|ev| format_value(ev, interner)).collect();
            format!("({})", parts.join(", "))
        }
        Value::InternedStr(id) => {
            format!("\"{}\"", interner.search(*id).to_string())
        }
        Value::RuntimeStr(s) => format!("\"{s}\""),
        Value::Func(_) => format!("Function"),
        Value::Array(elems) => {
            let parts: Vec<String> = elems.iter().map(|ev| format_value(ev, interner)).collect();
            format!("[{}]", parts.join(", "))
        }
        Value::Unknown => "Unknown".into(),
    }
}
