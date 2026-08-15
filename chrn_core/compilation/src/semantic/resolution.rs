//! Module intended for storing free functions that perform general resolution tasks

pub(crate) mod resolution_concepts;
pub(crate) mod resolution_helpers;

use chrn_utils::{
    id_types::{InternedId, SpannedContainer, SymbolId, TypeId},
    intern,
    source_map::source_span::SourceSpan,
};
use lang::{
    chrn_classifier::ChrnClassifiable,
    types::builtins::{BuiltinType, BuiltinTypeKind},
};
use lang::{chrn_classifier::ChrnClassifier, directives::Directive};

use crate::{
    lookup::scopes::{
        self,
        scopes_concepts::{
            self, AssociatedScopeKind, ScopeLookupPattern, ScopeLookupPreferenceFlags, ScopeType,
            SymbolLookupOutput,
        },
    },
    parser::ast::{
        ast_concepts::AbstractDirective,
        ast_exprs::{AbstractGeneric, PathSegment, TypeExpr},
    },
    resolvers::resolver_env::ResolverEnv,
    script_compiler::ScriptCompiler,
    semantic::{
        hir::{
            hir_concepts::{BuiltinTypeInfo, Type, TypeInfo},
            hir_symbols::{Symbol, SymbolKind, SymbolOrigin},
        },
        preset_reporter::preset_err::PresetErr,
        resolution::resolution_concepts::{
            AmbiguousAccessOutput, StaticAccessOption, StaticAccessResult, TypeExprResult,
        },
    },
};
/// - associated_scope: The environment to search in
/// - sp_ty_expr: The current type expression
/// - scope_type: The ScopeType environment
/// - lookup_pattern: The type of lookup which is recursively changed depending on if a direct
/// member access is being searched, or if a library such as core can be searched externally.
pub fn resolve_type_expr(
    compiler: &mut ScriptCompiler,
    associated_scope: AssociatedScopeKind,
    sp_ty_expr: &SpannedContainer<TypeExpr>,
    scope_type: ScopeType,
    lookup_pattern: ScopeLookupPattern,
    env: &ResolverEnv,
) -> TypeExprResult {
    let lookup_pref = ScopeLookupPreferenceFlags::new(ScopeLookupPreferenceFlags::TYPE.into());
    match &sp_ty_expr.inner {
        TypeExpr::Var(name_id) => {
            let sp_name_id = SpannedContainer::new(*name_id, sp_ty_expr.span);

            //NOTE: Should encode that, it prefers a namespace over a type,
            match scopes::find_sym_id(
                compiler,
                associated_scope,
                *name_id,
                scope_type,
                lookup_pattern,
                lookup_pref,
            ) {
                Some(SymbolLookupOutput { found_sym_id, .. }) => {
                    let found_sym = &compiler.symbols[found_sym_id];
                    match found_sym.kind {
                        SymbolKind::Type(type_id) => {
                            let sym = &compiler.symbols[found_sym_id];

                            // If the symbol being looked up is private and the owner isn't the current
                            // module then failed
                            // Should probably return the type id with it since this isn't
                            // supposed to assume any errors
                            if let SymbolOrigin::Module(mod_origin_id) = sym.sym_origin {
                                if sym.is_priv && mod_origin_id != env.current_mod {
                                    return TypeExprResult::PrivateTypeAccess {
                                        sp_found_type_id: SpannedContainer::new(
                                            type_id,
                                            sp_ty_expr.span,
                                        ),
                                        found_sym_id,
                                        current_mod_id: env.current_mod,
                                    };
                                }
                            }

                            TypeExprResult::Type(type_id)
                        }
                        SymbolKind::Namespace => {
                            let fmtted = found_sym
                                .associated_scope
                                .expect("Should be namespace")
                                .to_classified();
                            return TypeExprResult::NotAType {
                                found_sym_id,
                                sp_name_id,
                                scope_found_in: associated_scope,
                            };
                        }
                        SymbolKind::Variable(_) => {
                            return TypeExprResult::NotAType {
                                found_sym_id,
                                sp_name_id,
                                scope_found_in: associated_scope,
                            };
                        }
                        SymbolKind::Directive(_) => {
                            return TypeExprResult::NotAType {
                                found_sym_id,
                                sp_name_id,
                                scope_found_in: associated_scope,
                            };
                        }
                        // NOTE:
                        // External types are too restricted to their particular scope that this is
                        // not actually possible. `override` doesn't have access to type expression
                        // declarations.
                        SymbolKind::ExternType => unreachable!(),
                    }
                }
                None => {
                    return TypeExprResult::SymbolNotFound(sp_name_id, associated_scope);
                }
            }
        }
        TypeExpr::Generic(generic) => resolve_generic(
            compiler,
            generic,
            associated_scope,
            sp_ty_expr.span,
            scope_type,
            env,
        ),
        TypeExpr::Path(sp_path_segs) => {
            let last_scope = match resolve_static_access(
                compiler,
                sp_path_segs,
                associated_scope,
                scope_type,
                ScopeLookupPreferenceFlags::new(ScopeLookupPreferenceFlags::TYPE.into()),
                StaticAccessOption::Type,
            ) {
                StaticAccessResult::Scope(scope) => scope,
                other => return TypeExprResult::StaticAccessFailure(other),
            };

            let last_segment = &sp_path_segs[sp_path_segs.len() - 1];

            match &last_segment.inner {
                PathSegment::Ident(interned_id) => {
                    // Is this a sign that resolve_type_expr should have an identifier only version
                    // which doesn't need this translation layer and instead is just a spanned container?
                    let inline_ty_expr =
                        SpannedContainer::new(TypeExpr::Var(*interned_id), last_segment.span);

                    resolve_type_expr(
                        compiler,
                        last_scope,
                        &inline_ty_expr,
                        scope_type,
                        lookup_pattern,
                        env,
                    )
                }
                PathSegment::Generic(generic) => {
                    //NOTE: This avoids turning the generic into a new owned type expr by giving the
                    //generic a specific resolution function, which isn't under a generic
                    //requirement for an owned `TypeExpr`
                    return resolve_generic(
                        compiler,
                        generic,
                        last_scope,
                        sp_ty_expr.span,
                        scope_type,
                        env,
                    );
                }
            }
        }
    }
}

/// Generic specific resolution function
fn resolve_generic(
    compiler: &mut ScriptCompiler,
    // Span me a new container
    generic: &AbstractGeneric,
    associated_scope: AssociatedScopeKind,
    ty_expr_span: SourceSpan,
    scope_type: ScopeType,
    env: &ResolverEnv,
) -> TypeExprResult {
    match BuiltinTypeKind::try_from_interned_id(generic.base.id) {
        Some(kind) => match kind {
            BuiltinTypeKind::List | BuiltinTypeKind::Set => {
                if generic.inputs.len() != 1 {
                    return TypeExprResult::InvalidGenericArgCount {
                        base: generic.base,
                        expected: 1,
                        inputs_span: ty_expr_span,
                    };
                }

                let inner = match resolve_type_expr(
                    compiler,
                    associated_scope,
                    &generic.inputs[0],
                    scope_type,
                    ScopeLookupPattern::NoRestrictions,
                    env,
                ) {
                    TypeExprResult::Type(tid) => tid,
                    other => return other,
                };
                //TEST:

                let sym_id = SymbolId::new(compiler.symbols.len() as u32);
                let type_id = TypeId::new(compiler.types.len() as u32);

                let (interned_id, ty) = if kind == BuiltinTypeKind::List {
                    let ty = Type::BuiltinTypeInfo(BuiltinTypeInfo::new(
                        sym_id,
                        BuiltinType::List(inner),
                    ));
                    (InternedId::new(intern::INTERNED_LIST), ty)
                } else {
                    let ty = Type::BuiltinTypeInfo(BuiltinTypeInfo::new(
                        sym_id,
                        BuiltinType::Set(inner),
                    ));
                    (InternedId::new(intern::INTERNED_SET), ty)
                };

                //WARN: Definitely not sure about this setup
                let sym = Symbol::new(
                    interned_id,
                    sym_id,
                    None,
                    SymbolOrigin::Module(env.current_mod),
                    true,
                    None,
                    ScopeType::Compiler,
                    SymbolKind::Type(type_id),
                );

                //WARN: Still not sure about this
                let ty_info = TypeInfo::new(ty, compiler.intrinsic_registry.core_mod_id);
                compiler.types.push(ty_info);
                compiler.symbols.push(sym);

                return TypeExprResult::Type(type_id);
            }
            //TEST: TEST:
            BuiltinTypeKind::Tuple => {
                let mut elements: Vec<TypeId> = Vec::new();

                for input in &generic.inputs {
                    match resolve_type_expr(
                        compiler,
                        associated_scope,
                        input,
                        scope_type,
                        ScopeLookupPattern::NoRestrictions,
                        env,
                    ) {
                        TypeExprResult::Type(tid) => elements.push(tid),
                        other => return other,
                    }
                }

                let sym_id = SymbolId::new(compiler.symbols.len() as u32);
                let type_id = TypeId::new(compiler.types.len() as u32);

                //WARN: Definitely not sure about this setup
                let sym = Symbol::new(
                    InternedId::new(intern::INTERNED_TUPLE),
                    sym_id,
                    None,
                    SymbolOrigin::Module(env.current_mod),
                    true,
                    None,
                    ScopeType::Compiler,
                    SymbolKind::Type(type_id),
                );

                let tuple = Type::BuiltinTypeInfo(BuiltinTypeInfo::new(
                    sym_id,
                    BuiltinType::Tuple(elements),
                ));

                let ty_info = TypeInfo::new(tuple, compiler.intrinsic_registry.core_mod_id);
                compiler.types.push(ty_info);
                compiler.symbols.push(sym);

                return TypeExprResult::Type(type_id);
            }
            BuiltinTypeKind::Map => {
                if generic.inputs.len() != 2 {
                    return TypeExprResult::InvalidGenericArgCount {
                        base: generic.base,
                        expected: 2,
                        inputs_span: ty_expr_span,
                    };
                }

                let key = match resolve_type_expr(
                    compiler,
                    AssociatedScopeKind::Module(env.current_mod),
                    &generic.inputs[0],
                    scope_type,
                    ScopeLookupPattern::NoRestrictions,
                    env,
                ) {
                    TypeExprResult::Type(tid) => tid,
                    other => return other,
                };

                let val = match resolve_type_expr(
                    compiler,
                    AssociatedScopeKind::Module(env.current_mod),
                    &generic.inputs[1],
                    scope_type,
                    ScopeLookupPattern::NoRestrictions,
                    env,
                ) {
                    TypeExprResult::Type(tid) => tid,
                    other => return other,
                };

                //TEST: TEST:
                let sym_id = SymbolId::new(compiler.symbols.len() as u32);
                let type_id = TypeId::new(compiler.types.len() as u32);

                let sym = Symbol::new(
                    InternedId::new(intern::INTERNED_MAP),
                    sym_id,
                    None,
                    SymbolOrigin::Module(env.current_mod),
                    true,
                    None,
                    // Compiler generated but user created...
                    // Not sure about this one buddy
                    ScopeType::Compiler,
                    SymbolKind::Type(type_id),
                );

                let map =
                    Type::BuiltinTypeInfo(BuiltinTypeInfo::new(sym_id, BuiltinType::Map(key, val)));

                let ty_info = TypeInfo::new(map, compiler.intrinsic_registry.core_mod_id);
                compiler.types.push(ty_info);
                compiler.symbols.push(sym);

                return TypeExprResult::Type(type_id);
            }
            _ => (),
        },
        None => (),
    }

    TypeExprResult::UnknownGenericIdent(SpannedContainer::new(generic.base, ty_expr_span))
}

/// Genral function for traversing scopes inside of static access.
///
/// Takes in the segments to traverse, scope to start in, scope type for scoping rules, and
/// whether or not type expression restrictions should be applied.
/// - sp_path_segs: Segments that will be traversed
/// - current_scope: Scope environment to start in
/// - scope_type: ScopeType environment
/// - lookup_pref: Bias flags to adhere to for all scope checks.
/// - opt: What rules should be applied for when to return a particular result, given a value or
/// type only setting.
///
/// NOTE: If `sp_path_seg.len()` == 1, `current_scope` is returned, resulting in a no-op. This is
/// done because if there is no `::` after the current segment, then going further does no real help
/// for resolution.
pub fn resolve_static_access(
    compiler: &mut ScriptCompiler,
    sp_path_segs: &[SpannedContainer<PathSegment>],
    mut current_scope: AssociatedScopeKind,
    scope_type: ScopeType,
    lookup_pref: ScopeLookupPreferenceFlags,
    opt: StaticAccessOption,
) -> StaticAccessResult {
    //TEST: If the path segment len == 1 this will not do anything because this is not actually
    //accessing anything. Doing this because if we have something like "Thing::Thingy" and either
    //could be a type or symbol, being able to call this without explicitly at the call site
    //skipping sounds like a more valid argument than, if you call this without accounting for that
    //your system breaks. For example in the intrinsic scope "JAVA" if "JAVA {}" is typed, it goes
    //into the namespace of JAVA as though it acutally had access used, but in reality it should be
    //a no-op.
    if sp_path_segs.len() == 1 {
        return StaticAccessResult::Scope(current_scope);
    }

    for (i, sp_path_seg) in sp_path_segs.iter().enumerate() {
        match &sp_path_seg.inner {
            PathSegment::Ident(interned_id) => {
                // NOTE: `another` searched here
                if let Some(SymbolLookupOutput { found_sym_id, .. }) = scopes::find_sym_id(
                    compiler,
                    current_scope,
                    *interned_id,
                    scope_type,
                    ScopeLookupPattern::NamespaceOnly,
                    lookup_pref,
                ) {
                    let sym = &compiler.symbols[found_sym_id];
                    match sym.associated_scope {
                        Some(new_scope) => {
                            // Transitioning to next namespace
                            current_scope = new_scope;
                        }
                        None => {
                            if i + 1 < sp_path_segs.len() {
                                return StaticAccessResult::NoNamespace(SpannedContainer::new(
                                    *interned_id,
                                    sp_path_seg.span,
                                ));
                            }
                        }
                    }
                } else {
                    let prev_seg = if i > 0 {
                        match &sp_path_segs[i - 1].inner {
                            PathSegment::Ident(prev_name_id) => Some(SpannedContainer::new(
                                *prev_name_id,
                                sp_path_segs[i - 1].span,
                            )),
                            PathSegment::Generic(_) => None,
                        }
                    } else {
                        None
                    };

                    // `another` failed here
                    return StaticAccessResult::SymNotFound {
                        current_seg: SpannedContainer::new(*interned_id, sp_path_seg.span),
                        prev_seg,
                    };
                }
            }
            PathSegment::Generic(_) if opt == StaticAccessOption::Val => {
                if i + 1 != sp_path_segs.len() {
                    return StaticAccessResult::GenericUsingStaticPath(sp_path_seg.span);
                }
                break;
            }
            PathSegment::Generic(_) => {
                unreachable!("Cannot have generics within expr at the language level");
                // return StaticAccessResult::GenericInExpr(sp_path_seg.span);
            }
        }
    }

    StaticAccessResult::Scope(current_scope)
}

// I KNOW WHAT THIS LOOKS LIKE BUT IT MIGHT BECOME MORE THAN THIS SO IT STILL GETS ITS OWN FUNCTION
/// Resolves directive
pub fn resolve_directive(abs_directive: &AbstractDirective) -> Option<Directive> {
    Directive::try_from_interned_str(abs_directive.sp_name_id.inner)
}
