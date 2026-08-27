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
use compilation::lookup::scopes::scopes_concepts::AssociatedScopeKind;
use compilation::script_compiler::ScriptCompiler;
use compilation::semantic::hir::hir_concepts::Type;
use compilation::semantic::hir::hir_impls::ImplMemberKind;
use compilation::semantic::hir::hir_symbols::{
    MemberSymbolKind, SymbolKind, SymbolOrigin, VariableState,
};
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
            let compiler = match &state.compiler {
                Some(c) => c,
                None => return None,
            };
            (
                identifier_hover(state, compiler, id, offset),
                Some((span_start, span_end)),
            )
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

/// Hover text for an identifier token.
///
/// The semantic entity at the cursor answers first; when it produces nothing, the
/// name itself is matched against the module list and then the builtin types.
fn identifier_hover(
    state: &crate::state::DocumentState,
    compiler: &ScriptCompiler,
    id: chrn_utils::id_types::InternedId,
    offset: usize,
) -> String {
    let mut hover_text = state
        .get_entity_at_offset(offset)
        .map(|entity| entity_hover(state, compiler, entity, id))
        .unwrap_or_default();

    if hover_text.is_empty()
        && let Some(module) = compiler.mods.iter().find(|m| m.name_id == id)
    {
        hover_text = module_hover(state, compiler, module, id);
    }

    if hover_text.is_empty()
        && let Some(kind) = BuiltinTypeKind::try_from_interned_id(id.id)
    {
        hover_text = Document::builtin_type_docs(kind).compose();
    }

    hover_text
}

/// Dispatches on what the cursor is sitting on.  An empty string means "nothing to
/// say", which lets the caller fall through to its name-based fallbacks.
fn entity_hover(
    state: &crate::state::DocumentState,
    compiler: &ScriptCompiler,
    entity: &SemanticEntity,
    id: chrn_utils::id_types::InternedId,
) -> String {
    match entity {
        SemanticEntity::Symbol(sym_id) => symbol_hover(state, compiler, *sym_id),
        SemanticEntity::Field {
            owner_sym_id,
            field_idx,
        } => field_hover(state, compiler, *owner_sym_id, *field_idx),
        SemanticEntity::Variant {
            owner_sym_id,
            variant_idx,
        } => variant_hover(state, compiler, *owner_sym_id, *variant_idx),
        SemanticEntity::Module(mod_id) => {
            module_hover(state, compiler, &compiler.mods[*mod_id], id)
        }
        SemanticEntity::Local { name_id, .. } => {
            format!("{}: (param)", state.interner.search(*name_id))
        }
        SemanticEntity::ConfigMember { member_id, .. } => {
            config_member_hover(state, compiler, *member_id)
        }
        SemanticEntity::ConfigOption { member_id, .. } => {
            config_option_hover(state, compiler, *member_id)
        }
    }
}

/// Hover for a named symbol, with the shared privacy/scope footer appended.
fn symbol_hover(
    state: &crate::state::DocumentState,
    compiler: &ScriptCompiler,
    sym_id: chrn_utils::id_types::SymbolId,
) -> String {
    let Some(sym) = compiler.symbols.get(sym_id) else {
        return String::new();
    };
    let interner = &state.interner;

    let mut hover_text = match sym.kind {
        SymbolKind::Type(type_id) => type_symbol_hover(compiler, interner, sym, type_id),
        SymbolKind::Variable(var_id) => match compiler.variables[var_id].state {
            VariableState::Known(val_id) => {
                let val_info = &compiler.values[val_id];
                let type_str = strip_struct_enum_prefix(&format_type(
                    &compiler.types[val_info.type_id].ty,
                    compiler,
                    interner,
                    true,
                ));
                let val_str = match &val_info.const_val {
                    Some(v) => format_value(v, interner),
                    None => "unknown".to_string(),
                };
                format!(
                    "{}: {} = {}",
                    interner.search(sym.name_id),
                    type_str,
                    val_str
                )
            }
            VariableState::ReservedTypeSlot(_) => "Unknown".to_string(),
        },
        SymbolKind::Namespace => match sym
            .associated_scope
            .expect("Namespace should have associated scope")
        {
            AssociatedScopeKind::Module(mod_id) => {
                format!(
                    "module **{}**",
                    interner.search(compiler.mods[mod_id].name_id)
                )
            }
            AssociatedScopeKind::Scope(_) => {
                format!("namespace **{}**", interner.search(sym.name_id))
            }
        },
        SymbolKind::Directive(_) => {
            let name = interner.search(sym.name_id);
            Document::directive_docs(name)
                .map(|d| d.compose())
                .unwrap_or_else(|| format!("`#{}`", name))
        }
        //TODO: `ExternType` is unfinished in core and carries no type or value to
        //describe yet, so hover only names it.
        SymbolKind::ExternType => format!("extern type **{}**", interner.search(sym.name_id)),
    };

    if !hover_text.is_empty() {
        let privacy = if sym.is_priv { "private" } else { "public" };
        hover_text.push_str(&format!(
            "\n\n{}\n\n{} | **Scope:** {}",
            document::HOVER_DASHES,
            privacy,
            sym.scope_origin
        ));
    }
    hover_text
}

/// Hover body for a symbol that names a type.
fn type_symbol_hover(
    compiler: &ScriptCompiler,
    interner: &Intern,
    sym: &compilation::semantic::hir::hir_symbols::Symbol,
    type_id: chrn_utils::id_types::TypeId,
) -> String {
    let ty_info = &compiler.types[type_id];
    match &ty_info.ty {
        Type::TypeDef(type_def) => {
            let inner = &compiler.types[type_def.type_id].ty;
            let shallow = strip_struct_enum_prefix(&format_type(inner, compiler, interner, true));
            format!("**typedef**: {}", shallow)
        }
        Type::BuiltinTypeInfo(builtin_info) => {
            Document::builtin_type_docs(builtin_info.ty.kind()).compose()
        }
        Type::Func(func_def) => Document::func_docs(func_def.kind).compose(),
        _ => {
            let t = format_type(&ty_info.ty, compiler, interner, false);
            let is_named_decl =
                t.starts_with("struct ") || t.starts_with("enum ") || t.starts_with("alias ");
            if !is_named_decl {
                return format!("type: {}", t);
            }

            let owner_id = match sym.sym_origin {
                SymbolOrigin::Module(mid) => mid.id,
                SymbolOrigin::Compiler => 0,
            };
            let export_prefix = if sym.is_priv { "" } else { "export " };
            format!(
                "module **{}**\n\n{}{}",
                interner.search(compiler.mods[ModuleId::new(owner_id)].name_id),
                export_prefix,
                t
            )
        }
    }
}

/// Hover for a struct field: `name: Type`, from the resolved HIR when available.
fn field_hover(
    state: &crate::state::DocumentState,
    compiler: &ScriptCompiler,
    owner_sym_id: chrn_utils::id_types::SymbolId,
    field_idx: usize,
) -> String {
    let Some((sym, ast)) = owner_decl(state, compiler, owner_sym_id) else {
        return String::new();
    };
    let ast_id = sym.ast_id.expect("owner_decl checked the ast id");
    let Some(field) = ast.get_struct(ast_id).fields.get(field_idx) else {
        return String::new();
    };
    let field_name = state.interner.search(field.name_id);

    if let SymbolKind::Type(tid) = sym.kind
        && let Type::Struct(sdef) = &compiler.types[tid].ty
        && let Some(member_id) = sdef.fields.get(field_idx)
        && let Some(MemberSymbolKind::Field(field_repre)) = compiler.sym_members.get(*member_id)
    {
        let type_str = strip_struct_enum_prefix(&format_type(
            &compiler.types[field_repre.type_id].ty,
            compiler,
            &state.interner,
            true,
        ));
        return format!("{}: {}", field_name, type_str);
    }

    // Type resolution failed; the name alone is still worth showing.
    format!("{}: Unknown", field_name)
}

/// Hover for an enum variant: `name: Type`, or just the name when it carries no type.
fn variant_hover(
    state: &crate::state::DocumentState,
    compiler: &ScriptCompiler,
    owner_sym_id: chrn_utils::id_types::SymbolId,
    variant_idx: usize,
) -> String {
    let Some((sym, ast)) = owner_decl(state, compiler, owner_sym_id) else {
        return String::new();
    };
    let ast_id = sym.ast_id.expect("owner_decl checked the ast id");
    let Some(variant) = ast.get_enum(ast_id).variants.get(variant_idx) else {
        return String::new();
    };
    let variant_name = state.interner.search(variant.name_id);

    if let SymbolKind::Type(tid) = sym.kind
        && let Type::Enum(edef) = &compiler.types[tid].ty
        && let Some(member_id) = edef.variants.get(variant_idx)
        && let Some(MemberSymbolKind::Variant(variant_repre)) = compiler.sym_members.get(*member_id)
        && let Some(vty_id) = variant_repre.type_id
    {
        let type_str = strip_struct_enum_prefix(&format_type(
            &compiler.types[vty_id].ty,
            compiler,
            &state.interner,
            true,
        ));
        return format!("{}: {}", variant_name, type_str);
    }

    variant_name.to_string()
}

/// The symbol owning a member plus the AST of the module that declared it.
fn owner_decl<'a>(
    state: &'a crate::state::DocumentState,
    compiler: &'a ScriptCompiler,
    owner_sym_id: chrn_utils::id_types::SymbolId,
) -> Option<(
    &'a compilation::semantic::hir::hir_symbols::Symbol,
    &'a compilation::parser::ast::ast_concepts::AstInfo,
)> {
    let sym = compiler.symbols.get(owner_sym_id)?;
    sym.ast_id?;
    let owner_id = match sym.sym_origin {
        SymbolOrigin::Module(mid) => mid.id as usize,
        SymbolOrigin::Compiler => 0,
    };
    let ast = state.asts.get(owner_id)?.as_ref()?;
    Some((sym, ast))
}

/// Hover for a module reference: its name, the alias it was imported under, and the
/// file it came from.
fn module_hover(
    state: &crate::state::DocumentState,
    compiler: &ScriptCompiler,
    module: &compilation::module::module_concepts::Module,
    referenced_as: chrn_utils::id_types::InternedId,
) -> String {
    let interner = &state.interner;
    let mod_path = module
        .region_id
        .and_then(|region_id| state.region_arena.get(region_id))
        .map(|region| interner.search_path(region.path_id).display().to_string())
        .unwrap_or_else(|| "<builtin>".to_string());

    let alias_prefix = compiler.mods[ModuleId::new(0)]
        .imports
        .iter()
        .find_map(|i| {
            i.sp_alias_id
                .as_ref()
                .map(|sp_alias| sp_alias.inner)
                .filter(|a| *a == referenced_as)
                .map(|a| format!("alias **{}** | ", interner.search(a)))
        })
        .unwrap_or_default();

    format!(
        "{}module **{}**\n{}\npath: `{}`",
        alias_prefix,
        interner.search(module.name_id),
        document::HOVER_DASHES,
        mod_path
    )
}

/// Hover for a `complex->` member block, naming the field or variant it configures.
fn config_member_hover(
    state: &crate::state::DocumentState,
    compiler: &ScriptCompiler,
    member_id: chrn_utils::id_types::ImplMemberId,
) -> String {
    let cfg_member = compiler.get_cfg_def_member(member_id);
    let name = state.interner.search(cfg_member.name_id);
    let type_of = |type_id| {
        strip_struct_enum_prefix(&format_type(
            &compiler.types[type_id].ty,
            compiler,
            &state.interner,
            true,
        ))
    };

    match compiler.sym_members.get(cfg_member.linked_member_id) {
        Some(MemberSymbolKind::Field(field_repre)) => format!(
            "Configures field **{}**: `{}`",
            name,
            type_of(field_repre.type_id)
        ),
        Some(MemberSymbolKind::Variant(variant_repre)) => match variant_repre.type_id {
            Some(vty_id) => format!("Configures variant **{}**: `{}`", name, type_of(vty_id)),
            None => format!("Configures variant **{}**", name),
        },
        None => format!("Configures **{}**: Unknown", name),
    }
}

/// Hover for an option assignment key inside a config block.
fn config_option_hover(
    state: &crate::state::DocumentState,
    compiler: &ScriptCompiler,
    member_id: chrn_utils::id_types::ImplMemberId,
) -> String {
    let name_id = match &compiler.impl_members[member_id] {
        ImplMemberKind::OptAssignmentRoot(opt) => opt.name_id,
        ImplMemberKind::OptAssignmentMember(opt) => opt.name_id,
        _ => return String::new(),
    };
    let name = state.interner.search(name_id);
    document::Document::config_option_docs(name)
        .map(|doc| doc.compose())
        .unwrap_or_else(|| format!("**{}**\n\nUnknown option", name))
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
            b => interner.search(b.kind().name_id()).to_string(),
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
                    .filter_map(|member_id| match compiler.sym_members.get(*member_id)? {
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
                    .filter_map(|member_id| match compiler.sym_members.get(*member_id)? {
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
