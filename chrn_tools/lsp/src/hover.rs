//! # hover
//!
//! Computes rich Markdown hover content for tokens and semantic entities.
//!
//! The single public entry point is [`compute_hover`], which is called from
//! [`crate::backend::Backend::hover`] after the document state has been analysed.
//!
//! ## Hover dispatch
//!
//! 1. The cursor byte offset is computed from the LSP `Position`.
//! 2. If the offset is inside a comment, `None` is returned immediately.
//! 3. [`DocumentState::get_symbol_at_offset`](crate::state::DocumentState::get_symbol_at_offset)
//!    attempts to find an identifier token.  If that fails, a linear scan over all
//!    tokens finds the closest non-identifier token.
//! 4. The resulting [`ScriptToken`](compilation::token::Token) variant determines
//!    which documentation branch runs:
//!
//!    | Token kind           | Hover content |
//!    |----------------------|---------------|
//!    | `@def` / `@end`      | Block-delimiter docs |
//!    | `Keyword`            | [`Document::keyword_docs`](crate::document::Document::keyword_docs) |
//!    | `Id`                 | Semantic-entity dispatch (see below) |
//!    | Literals             | Type + literal value string |
//!    | Punctuation / ops    | Brief description |
//!
//! ### Identifier hover (semantic dispatch)
//!
//! When the token is an identifier the [`SemanticEntity`](crate::state::SemanticEntity)
//! at that offset is retrieved from `symbol_map` and dispatched:
//!
//! * **Symbol** — type, variable, or module hover with full type signature.
//! * **Field** / **Variant** — `name: Type` from the resolved struct/enum HIR.
//! * **Module** — module name, alias prefix, and file path.
//! * **Local** — name with `(param)` tag.
//!
//! A name-based fallback for modules and a builtin-type fallback are applied when
//! the semantic entity lookup yields no result.

use chrn_utils::id_types::ModuleId;
use chrn_utils::intern::Intern;
use compilation::lexer::token::Token as ScriptToken;
use compilation::script_compiler::ScriptCompiler;
use compilation::semantic::hir::hir_concepts::{
    self, MemberSymbolKind, SymbolKind, Type, VariableState,
};
use lang::fmter::Formattable;
use lang::types::builtins::{BuiltinType, BuiltinTypeKind};
use lang::values::Value;
use tower_lsp::lsp_types;

use crate::document::{self, Document};
use crate::state::SemanticEntity;
use crate::text::position_to_offset;

/// Computes the hover response for a cursor position within a Chern document.
///
/// # Parameters
/// * `_uri`  — The document URI (currently unused but kept for future use).
/// * `pos`   — The cursor position in LSP UTF-16 coordinates.
/// * `state` — The fully-analysed document state.
///
/// # Returns
/// * `Some(Hover)` with Markdown-formatted text and the token range when
///   meaningful documentation can be produced.
/// * `None` when the cursor is in a comment, on whitespace, or on a token with
///   no associated documentation.
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
            match state.get_token_at_offset(offset) {
                // The token's span is relative to the region's `src_bytes`; shift
                // it to absolute file coordinates so the returned hover range
                // is usable as an LSP `Range` directly.
                Some(st) => (
                    st.tok,
                    crate::text::rel_to_abs_offset(st.span.start, state.script_start) as usize,
                    crate::text::rel_to_abs_offset(st.span.end, state.script_start) as usize,
                ),
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

            (msg, Some((span_start, span_end)))
        }
        ScriptToken::End => {
            let msg = format!(
                "**@end** — Ends embedded script block\n\n{}\n\n**Example:**\n```chrn\n@end\n// Everything after this is serialized data\n```",
                document::HOVER_DASHES
            );

            (msg, Some((span_start, span_end)))
        }
        ScriptToken::Keyword(kw) => {
            let doc = Document::keyword_docs(kw);
            (doc.compose(), Some((span_start, span_end)))
        }
        ScriptToken::Id(id) => {
            let mut hover_text = String::new();
            let compiler = match &state.compiler {
                Some(c) => c,
                None => return None,
            };
            let interner = &state.interner;
            let interned = id;

            let entity = state.get_entity_at_offset(offset);

            if let Some(entity) = entity {
                match entity {
                    SemanticEntity::Symbol(sym_id) => {
                        // `entity: &SemanticEntity`, so `sym_id: &SymbolId`.  The
                        // Arena's `get` takes the index by value, so dereference.
                        if let Some(sym) = compiler.symbols.get(*sym_id) {
                            match sym.kind {
                                SymbolKind::Type(type_id) => {
                                    let ty_info = &compiler.types[type_id];
                                    let t = format_type(&ty_info.ty, compiler, interner, false);
                                    match &ty_info.ty {
                                        hir_concepts::Type::TypeDef(type_def) => {
                                            let inner = &compiler.types[type_def.type_id].ty;
                                            let shallow_t = strip_struct_enum_prefix(&format_type(
                                                inner, compiler, interner, true,
                                            ));
                                            hover_text = format!("**typedef**: {}", shallow_t);
                                        }
                                        _ => {
                                            if let compilation::semantic::hir::hir_concepts::Type::BuiltinTypeInfo(
                                                builtin_info,
                                            ) = &ty_info.ty
                                            {
                                                hover_text =
                                                    Document::builtin_type_docs(builtin_info.ty.kind())
                                                        .compose();
                                            } else if let compilation::semantic::hir::hir_concepts::Type::Func(
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
                                                    let owner_id = match sym.sym_origin {
                                                        hir_concepts::SymbolOrigin::Module(mid) => {
                                                            mid.id
                                                        }
                                                        hir_concepts::SymbolOrigin::Compiler => 0,
                                                    };
                                                    // The `Arena` is parameterised by `ModuleId`; the
                                                    // primary `Index` impl expects a `ModuleId` by value,
                                                    // so wrap the raw `usize`.
                                                    let module = &compiler.mods[ModuleId::new(owner_id)];
                                                    let raw_mod_name =
                                                        interner.search(module.name_id);
                                                    final_text.push_str(&format!(
                                                        "module **{}**\n\n",
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
                                    let var = &compiler.variables[var_id];
                                    match var.state {
                                        VariableState::Known(val_id) => {
                                            let val_info = &compiler.values[val_id];
                                            let ty_info = &compiler.types[val_info.type_id];

                                            let var_name = interner.search(sym.name_id);
                                            let type_str = strip_struct_enum_prefix(&format_type(
                                                &ty_info.ty,
                                                compiler,
                                                interner,
                                                true,
                                            ));
                                            let val_str = match &val_info.const_val {
                                                Some(v) => format_value(v, interner),
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
                                    // `SymbolKind::Module` carries the `ModuleId` directly;
                                    // index the `Arena` with the typed id.
                                    let module = &compiler.mods[mod_id];
                                    let mod_name = interner.search(module.name_id);
                                    hover_text = format!("module **{}**", mod_name);
                                }
                                SymbolKind::Config(cfg_id) => {
                                    let cfg_root = &compiler.cfgs[cfg_id];
                                    let name = interner.search(cfg_root.name_id);

                                    let linked_type =
                                        if let Some(linked_sym_id) = cfg_root.linked_sym_id {
                                            if let Some(linked_sym) =
                                                compiler.symbols.get(linked_sym_id)
                                            {
                                                interner.search(linked_sym.name_id).to_string()
                                            } else {
                                                "Unknown".to_string()
                                            }
                                        } else {
                                            "Unknown".to_string()
                                        };

                                    hover_text = format!("**Config** `{}`", name);
                                }
                                SymbolKind::Directive(_) => {
                                    let name = interner.search(sym.name_id);
                                    hover_text = Document::directive_docs(name)
                                        .map(|d| d.compose())
                                        .unwrap_or_else(|| format!("`#{}`", name));
                                }
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
                        if let Some(sym) = compiler.symbols.get(*owner_sym_id)
                            && let Some(ast_id) = sym.ast_id
                        {
                            let owner_id = match sym.sym_origin {
                                hir_concepts::SymbolOrigin::Module(mid) => mid.id as usize,
                                hir_concepts::SymbolOrigin::Compiler => 0,
                            };
                            if let Some(Some(ast)) = state.asts.get(owner_id) {
                                let abs_struct = ast.get_struct(ast_id);
                                if let Some(field) = abs_struct.fields.get(*field_idx) {
                                    let field_name = interner.search(field.name_id);
                                    // We need the resolved type from the compiler, not just AST
                                    if let SymbolKind::Type(tid) = sym.kind
                                        && let Type::Struct(sdef) = &compiler.types[tid].ty
                                        && let Some(member_id) = sdef.fields.get(*field_idx)
                                        && let Some(MemberSymbolKind::Field(field_repre)) =
                                            compiler.members.get(*member_id)
                                    {
                                        let type_str = strip_struct_enum_prefix(&format_type(
                                            &compiler.types[field_repre.type_id].ty,
                                            compiler,
                                            interner,
                                            true,
                                        ));
                                        hover_text = format!("{}: {}", field_name, type_str);
                                    }
                                    if hover_text.is_empty() {
                                        // Fallback to name-only if type resolution failed
                                        hover_text = format!("{}: Unknown", field_name);
                                    }
                                }
                            }
                        }
                    }
                    SemanticEntity::Variant {
                        owner_sym_id,
                        variant_idx,
                    } => {
                        if let Some(sym) = compiler.symbols.get(*owner_sym_id)
                            && let Some(ast_id) = sym.ast_id
                        {
                            let owner_id = match sym.sym_origin {
                                hir_concepts::SymbolOrigin::Module(mid) => mid.id as usize,
                                hir_concepts::SymbolOrigin::Compiler => 0,
                            };
                            if let Some(Some(ast)) = state.asts.get(owner_id) {
                                let abs_enum = ast.get_enum(ast_id);
                                if let Some(variant) = abs_enum.variants.get(*variant_idx) {
                                    let variant_name = interner.search(variant.name_id);
                                    if let SymbolKind::Type(tid) = sym.kind
                                        && let Type::Enum(edef) = &compiler.types[tid].ty
                                        && let Some(member_id) = edef.variants.get(*variant_idx)
                                        && let Some(MemberSymbolKind::Variant(variant_repre)) =
                                            compiler.members.get(*member_id)
                                    {
                                        if let Some(vty_id) = variant_repre.type_id {
                                            let type_str = strip_struct_enum_prefix(&format_type(
                                                &compiler.types[vty_id].ty,
                                                compiler,
                                                interner,
                                                true,
                                            ));
                                            hover_text = format!("{}: {}", variant_name, type_str);
                                        } else {
                                            hover_text = variant_name.to_string();
                                        }
                                    }
                                    if hover_text.is_empty() {
                                        hover_text = variant_name.to_string();
                                    }
                                }
                            }
                        }
                    }
                    SemanticEntity::Module(mod_id) => {
                        // `entity: &SemanticEntity` so `mod_id: &ModuleId`.  Dereference
                        // to pass the typed `ModuleId` to the `Arena` index operator.
                        let module = &compiler.mods[*mod_id];
                        let mod_name = interner.search(module.name_id);
                        let mod_path = if let Some(region_id) = module.region_id {
                            if let Some(region) = state.region_arena.get(region_id) {
                                interner.search_path(region.path_id).display().to_string()
                            } else {
                                "<builtin>".to_string()
                            }
                        } else {
                            "<builtin>".to_string()
                        };

                        let alias_prefix = compiler.mods[ModuleId::new(0)]
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
                    SemanticEntity::ConfigMember { member_id, .. } => {
                        let cfg_member = compiler.get_cfg_def_member(*member_id);
                        let name = interner.search(cfg_member.name_id);
                        let tagged = if let Some(MemberSymbolKind::Field(field_repre)) =
                            compiler.members.get(cfg_member.linked_member_id)
                        {
                            let type_str = strip_struct_enum_prefix(&format_type(
                                &compiler.types[field_repre.type_id].ty,
                                compiler,
                                interner,
                                true,
                            ));
                            format!("Configures field **{}**: `{}`", name, type_str)
                        } else if let Some(MemberSymbolKind::Variant(variant_repre)) =
                            compiler.members.get(cfg_member.linked_member_id)
                        {
                            if let Some(vty_id) = variant_repre.type_id {
                                let type_str = strip_struct_enum_prefix(&format_type(
                                    &compiler.types[vty_id].ty,
                                    compiler,
                                    interner,
                                    true,
                                ));
                                format!("Configures variant **{}**: `{}`", name, type_str)
                            } else {
                                format!("Configures variant **{}**", name)
                            }
                        } else {
                            format!("Configures **{}**: Unknown", name)
                        };
                        hover_text = tagged;
                    }
                    SemanticEntity::ConfigOption { member_id, .. } => {
                        let name_id = match &compiler.members[*member_id] {
                            MemberSymbolKind::OptAssignmentRoot(opt) => Some(opt.name_id),
                            MemberSymbolKind::OptAssignmentMember(opt) => Some(opt.name_id),
                            _ => None,
                        };
                        if let Some(name_id) = name_id {
                            let name = interner.search(name_id);
                            if let Some(doc) = document::Document::config_option_docs(name) {
                                hover_text = doc.compose();
                            } else {
                                hover_text = format!("**{}**\n\nUnknown option", name);
                            }
                        }
                    }
                }
            }

            // Fallback to name-based lookup for modules or builtins
            if hover_text.is_empty()
                && let Some(module) = compiler.mods.iter().find(|m| m.name_id == interned)
            {
                let raw_mod_name = interner.search(module.name_id);
                let mod_path = if let Some(region_id) = module.region_id {
                    if let Some(region) = state.region_arena.get(region_id) {
                        interner.search_path(region.path_id).display().to_string()
                    } else {
                        "<builtin>".to_string()
                    }
                } else {
                    "<builtin>".to_string()
                };

                let alias_prefix = compiler.mods[ModuleId::new(0)]
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

            if hover_text.is_empty()
                && let Some(kind) = BuiltinTypeKind::try_from_interned_id(id.id)
            {
                hover_text = Document::builtin_type_docs(kind).compose();
            }

            (hover_text, Some((span_start, span_end)))
        }
        ScriptToken::Str(id) => {
            let s = state.interner.search(id);
            (
                format!("string literal: \"{}\"", s),
                Some((span_start, span_end)),
            )
        }
        ScriptToken::Integer(id, _) => {
            let s = state.interner.search(id);
            (
                format!("Integer literal: {}", s),
                Some((span_start, span_end)),
            )
        }
        ScriptToken::Float(id, _) => {
            let s = state.interner.search(id);
            (
                format!("Float literal: {}", s),
                Some((span_start, span_end)),
            )
        }
        ScriptToken::BoolLiteral(b) => {
            (format!("bool literal: {}", b), Some((span_start, span_end)))
        }
        ScriptToken::Char(c) => (
            format!("char literal: '{}'", c),
            Some((span_start, span_end)),
        ),
        ScriptToken::At => (
            "**@** — Directive marker (e.g. @def/@end)".into(),
            Some((span_start, span_end)),
        ),
        ScriptToken::HashSymbol => (
            "**#** — Directive prefix".into(),
            Some((span_start, span_end)),
        ),
        ScriptToken::SlimArrow => (
            "**->** — Section declaration operator".into(),
            Some((span_start, span_end)),
        ),
        _ => (String::new(), Some((span_start, span_end))),
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
        range: hover_range.map(|(s, e)| {
            let start_pos = crate::text::offset_to_position(text, s);
            let end_pos = crate::text::offset_to_position(text, e);
            lsp_types::Range {
                start: start_pos,
                end: end_pos,
            }
        }),
    };
    Some(hover)
}

/// Formats a HIR [`Type`] as a human-readable string.
///
/// # Parameters
/// * `ty`       — The type to format.
/// * `compiler` — The script compiler holding the type and symbol arenas.
/// * `interner` — Used to recover string names from interned IDs.
/// * `shallow`  — When `true`, struct and enum types are rendered as just
///   `"struct Name"` / `"enum Name"` without expanding fields or variants.
///   This is used for nested type display to avoid exponential output.
///
/// # Recursive types
/// Container types (`List`, `Set`, `Map`, `Tuple`) and `TypeDef` / `Deferred`
/// wrappers call this function recursively with `shallow = true` for inner types.
fn format_type(ty: &Type, compiler: &ScriptCompiler, interner: &Intern, shallow: bool) -> String {
    match ty {
        // The inner `Type::BuiltinTypeInfo(builtin_info)` match binds `builtin_info: &BuiltinTypeInfo`.
        // Access the inner `BuiltinType` via `builtin_info.ty`.
        // The destructured `TypeId`s are `&TypeId` references.
        // The `Arena` index takes `TypeId` by value, so each binding must be dereferenced.
        Type::BuiltinTypeInfo(builtin_info) => match &builtin_info.ty {
            BuiltinType::List(type_id) => {
                let inner = &compiler.types[*type_id].ty;
                format!(
                    "List<{}>",
                    strip_struct_enum_prefix(&format_type(inner, compiler, interner, true))
                )
            }
            BuiltinType::Set(type_id) => {
                let inner = &compiler.types[*type_id].ty;
                format!(
                    "Set<{}>",
                    strip_struct_enum_prefix(&format_type(inner, compiler, interner, true))
                )
            }
            BuiltinType::Map(kid, vid) => {
                let k = &compiler.types[*kid].ty;
                let v = &compiler.types[*vid].ty;
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
                        let ty = &compiler.types[*type_id].ty;
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
                .get(struct_def.sym_id)
                .map(|sym| interner.search(sym.name_id))
                .unwrap_or("<struct>");

            if shallow {
                return format!("struct {}", name);
            }

            if struct_def.fields.is_empty() {
                format!("struct {} {{}}", name)
            } else {
                let fields: Vec<String> = struct_def
                    .fields
                    .iter()
                    .filter_map(|member_id| match compiler.members.get(*member_id)? {
                        MemberSymbolKind::Field(field) => {
                            let field_name = interner.search(field.name_id);
                            let field_ty = &compiler.types[field.type_id].ty;
                            let field_ty_str = strip_struct_enum_prefix(&format_type(
                                field_ty, compiler, interner, true,
                            ));
                            Some(format!("\t{}: {}", field_name, field_ty_str))
                        }
                        _ => None,
                    })
                    .collect();
                format!("struct {} {{\n{}\n}}", name, fields.join("\n"))
            }
        }
        Type::Enum(enum_def) => {
            let name = compiler
                .symbols
                .get(enum_def.sym_id)
                .map(|sym| interner.search(sym.name_id))
                .unwrap_or("<enum>");

            if shallow {
                return format!("enum {}", name);
            }

            if enum_def.variants.is_empty() {
                format!("enum {} {{}}", name)
            } else {
                let variants: Vec<String> = enum_def
                    .variants
                    .iter()
                    .filter_map(|member_id| match compiler.members.get(*member_id)? {
                        MemberSymbolKind::Variant(v) => {
                            let variant_name = interner.search(v.name_id);

                            if let Some(type_id) = v.type_id {
                                let variant_ty = &compiler.types[type_id].ty;
                                let variant_ty_str = strip_struct_enum_prefix(&format_type(
                                    variant_ty, compiler, interner, true,
                                ));
                                Some(format!("\t{}: {}", variant_name, variant_ty_str))
                            } else {
                                Some(format!("\t{}", variant_name))
                            }
                        }
                        _ => None,
                    })
                    .collect();
                format!("enum {} {{\n{}\n}}", name, variants.join("\n"))
            }
        }
        Type::Func(func) => {
            let name = interner.search(func.name_id);
            let prefix = if func.is_callable {
                "function"
            } else {
                "predicate"
            };

            format!("{prefix} {name}")
        }
        Type::Alias(alias_def) => {
            let name = compiler
                .symbols
                .get(alias_def.sym_id)
                .map(|sym| interner.search(sym.name_id))
                .unwrap_or("<alias>");

            if shallow {
                return format!("alias {}", name);
            }

            let params: Vec<String> = alias_def
                .params
                .iter()
                .map(|p| {
                    let p_name = compiler
                        .symbols
                        .get(p.sym_id)
                        .map(|sym| interner.search(sym.name_id))
                        .unwrap_or("<param>");
                    let p_constraint = alias_def
                        .ty_constraints
                        .to_fmt_vec()
                        .iter()
                        .map(|f| f.to_string())
                        .collect::<Vec<_>>()
                        .join(" | ");
                    format!("{}: {}", p_name, p_constraint)
                })
                .collect();

            format!("alias {}({})", name, params.join(", "))
        }
        Type::TypeDef(type_def) => {
            let inner = &compiler.types[type_def.type_id].ty;
            format_type(inner, compiler, interner, shallow)
        }
        Type::Boundaries(flags) => flags
            .to_fmt_vec()
            .iter()
            .map(|f| f.to_string())
            .collect::<Vec<_>>()
            .join(" + "),
        Type::Deferred(type_id) => {
            // `Type::Deferred(type_id)` here matches through `&Type`, so `type_id: &TypeId`.
            let inner = &compiler.types[*type_id].ty;
            format_type(inner, compiler, interner, shallow)
        }
        Type::Unknown => "Unknown".into(),
    }
}

/// Strips the `"struct "` or `"enum "` prefix produced by [`format_type`] when used
/// as a nested type reference (e.g. inside `List<...>` or as a field type).
///
/// Returns the input unchanged if neither prefix is present.
fn strip_struct_enum_prefix(s: &str) -> String {
    if let Some(stripped) = s.strip_prefix("struct ") {
        stripped.to_string()
    } else if let Some(stripped) = s.strip_prefix("enum ") {
        stripped.to_string()
    } else {
        s.to_string()
    }
}

/// Formats a compile-time constant [`Value`] as a display string.
///
/// Used in variable hover to show `name: Type = <value>` when the value is
/// statically known.  Returns `"Unknown"` for runtime / unresolved values.
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
            format!("\"{}\"", interner.search(*id))
        }
        Value::RuntimeStr(s) => format!("\"{s}\""),
        Value::Func(_) => "Function".to_string(),
        Value::Array(elems) => {
            let parts: Vec<String> = elems.iter().map(|ev| format_value(ev, interner)).collect();
            format!("[{}]", parts.join(", "))
        }
        Value::Unknown => "Unknown".into(),
    }
}
