/// Module intended for storing free functions that perform general resolution tasks
use chrn_utils::{
    id_types::{InternedId, ModuleId, SpannedContainer, SymbolId, TypeId},
    intern,
    source_map::source_span::SourceSpan,
};
use lang::{directives::Directive, fmter::Formatted};
use lang::{
    fmter::Formattable,
    types::builtins::{BuiltinType, BuiltinTypeKind},
};

use crate::{
    lookup::scopes::{
        self, AssociatedScopeKind, ScopeLookupPattern, ScopeType, SymbolLookupOutput,
    },
    parser::ast::{
        ast_concepts::AbstractDirective,
        ast_exprs::{PathSegment, SpannedPathSegment, TypeExpr},
    },
    resolvers::resolver_env::ResolverEnv,
    script_compiler::ScriptCompiler,
    semantic::hir::{
        hir_concepts::{BuiltinTypeInfo, Type, TypeInfo},
        hir_symbols::{Symbol, SymbolKind, SymbolOrigin},
    },
};

//TODO: As of right now this is basically entirely hinging off of being usable for reporting, which
//is probably not the most concise design but it works for now so may not need changing.
/// Result type for type expr resolution attempts. This exists due to the fact that there is no `Ok` or `Err`
/// inherit concept behind whether or not something was found.
pub enum TypeExprResult {
    /// Found a type with no issues
    Type(TypeId),
    /// Found a smybol but it wasn't a type
    NotAType {
        found_sym_id: SymbolId,
        sp_name_id: SpannedContainer<InternedId>,
        kind: Formatted,
        scope_found_in: AssociatedScopeKind,
    },
    /// (Identifier not found as any symbol, scope searched)
    SymbolNotFound(SpannedContainer<InternedId>, AssociatedScopeKind),
    /// Symbol found but private to another module
    PrivateTypeAccess {
        found_type_id: TypeId,
        found_sym_id: SymbolId,
        current_mod: ModuleId,
        ty_expr_span: SourceSpan,
    },
    /// Found a valid data structure but the inputs exceed the expected
    InvalidGenericArgCount {
        base: InternedId,
        expected: usize,
        inputs_span: SourceSpan,
    },
    /// Found an identifier using generic parameters after it while not being a known data structure
    UnknownGenericIdent(SpannedContainer<InternedId>),
    /// Static access variant
    StaticAccessFailure(StaticAccessResult),
}

impl TypeExprResult {
    pub fn type_id(&self) -> Option<TypeId> {
        //TODO: Maybe give the Type variant the output as well as any others that it may be relevant
        //for
        match self {
            TypeExprResult::Type(type_id) => Some(*type_id),
            TypeExprResult::NotAType { .. }
            | TypeExprResult::PrivateTypeAccess { .. }
            | TypeExprResult::InvalidGenericArgCount { .. }
            | TypeExprResult::SymbolNotFound(_, _)
            | TypeExprResult::UnknownGenericIdent(_)
            | TypeExprResult::StaticAccessFailure(_) => None,
        }
    }
}

/// Result type for static access resolution attempts. This exists due to the fact that there is no
/// `Ok` or `Err` inherit concept behind whether or not something was found
pub enum StaticAccessResult {
    /// Scope found with no issues
    Scope(AssociatedScopeKind),
    /// If `prev_seg` is `None` then `current_seg` was a symbol that wasn't found.
    /// If it's `None` then then `current_seg` was found by going through `prev_seg`
    // Should be different variants
    SymNotFound {
        current_seg: SpannedContainer<InternedId>,
        prev_seg: Option<SpannedContainer<InternedId>>,
    },
    /// A segment was found but does not expose a further namespace
    NoNamespace(SpannedContainer<InternedId>),
    /// A generic found using "::" access
    /// (Generic span)
    GenericUsingStaticPath(SourceSpan),
    //// The parser cannot process a generic inside of exprs
    // GenericInExpr(SourceSpan),
}

impl StaticAccessResult {
    /// Tries to get associated scope out of result
    pub fn associated_scope(&self) -> Option<AssociatedScopeKind> {
        match self {
            StaticAccessResult::Scope(associated_scope) => Some(*associated_scope),
            StaticAccessResult::SymNotFound { .. }
            | StaticAccessResult::NoNamespace(_)
            | StaticAccessResult::GenericUsingStaticPath(_) => None,
        }
    }
}

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
    match &sp_ty_expr.inner {
        TypeExpr::Var(name_id) => {
            let sp_name = SpannedContainer::new(*name_id, sp_ty_expr.span);

            match scopes::find_sym_id(
                compiler,
                associated_scope,
                *name_id,
                scope_type,
                lookup_pattern,
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
                                        found_type_id: type_id,
                                        found_sym_id,
                                        current_mod: env.current_mod,
                                        ty_expr_span: sp_ty_expr.span,
                                    };
                                }
                            }

                            TypeExprResult::Type(type_id)
                        }
                        SymbolKind::Namespace => {
                            let fmtted = found_sym
                                .associated_scope
                                .expect("Should be namespace")
                                .to_fmt();
                            return TypeExprResult::NotAType {
                                found_sym_id,
                                sp_name_id: sp_name,
                                kind: fmtted,
                                scope_found_in: associated_scope,
                            };
                        }
                        SymbolKind::Variable(_) => {
                            return TypeExprResult::NotAType {
                                found_sym_id,
                                sp_name_id: sp_name,
                                kind: Formatted::Variable,
                                //WARN: Feels like this should be very concerning...
                                scope_found_in: associated_scope,
                            };
                        }
                        SymbolKind::Directive(_) => {
                            return TypeExprResult::NotAType {
                                found_sym_id,
                                sp_name_id: sp_name,
                                kind: Formatted::Directive,
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
                    return TypeExprResult::SymbolNotFound(sp_name, associated_scope);
                }
            }
        }
        TypeExpr::Generic(generic) => {
            match BuiltinTypeKind::try_from_interned_id(generic.base.id) {
                Some(kind) => match kind {
                    BuiltinTypeKind::List | BuiltinTypeKind::Set => {
                        if generic.inputs.len() != 1 {
                            return TypeExprResult::InvalidGenericArgCount {
                                base: generic.base,
                                expected: 1,
                                inputs_span: sp_ty_expr.span,
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
                                inputs_span: sp_ty_expr.span,
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
                            ScopeType::Compiler,
                            SymbolKind::Type(type_id),
                        );

                        let map = Type::BuiltinTypeInfo(BuiltinTypeInfo::new(
                            sym_id,
                            BuiltinType::Map(key, val),
                        ));

                        let ty_info = TypeInfo::new(map, compiler.intrinsic_registry.core_mod_id);
                        compiler.types.push(ty_info);
                        compiler.symbols.push(sym);

                        return TypeExprResult::Type(type_id);
                    }
                    _ => (),
                },
                None => (),
            }

            TypeExprResult::UnknownGenericIdent(SpannedContainer::new(
                generic.base,
                sp_ty_expr.span,
            ))
        }
        TypeExpr::Path(sp_path_segs) => {
            let last_scope = match resolve_static_access(
                compiler,
                sp_path_segs,
                associated_scope,
                scope_type,
                true,
            ) {
                StaticAccessResult::Scope(scope) => scope,
                other => return TypeExprResult::StaticAccessFailure(other),
            };

            let last_segment = &sp_path_segs[sp_path_segs.len() - 1];

            let inline_ty_expr = match &last_segment.kind {
                PathSegment::Ident(interned_id) => {
                    SpannedContainer::new(TypeExpr::Var(*interned_id), last_segment.span)
                }
                PathSegment::Generic(generic) => {
                    SpannedContainer::new(TypeExpr::Generic(generic.clone()), last_segment.span)
                }
            };

            resolve_type_expr(
                compiler,
                last_scope,
                &inline_ty_expr,
                scope_type,
                lookup_pattern,
                env,
            )
        }
    }
}

/// Genral function for traversing scopes inside of static access.
///
/// Takes in the segments to traverse, scope to start in, scope type for scoping rules, and
/// whether or not type expression restrictions should be applied.
/// - sp_path_segs: Segments that will be traversed
/// - current_scope: Scope environment to start in
/// - scope_type: ScopeType environment
/// - in_ty_expr: Decides if a particular path gone down is wrong, given the context of what type of
/// expr is being traversed
pub fn resolve_static_access(
    compiler: &mut ScriptCompiler,
    sp_path_segs: &[SpannedPathSegment],
    mut current_scope: AssociatedScopeKind,
    scope_type: ScopeType,
    in_ty_expr: bool,
) -> StaticAccessResult {
    for (i, sp_path_seg) in sp_path_segs.iter().enumerate() {
        match &sp_path_seg.kind {
            PathSegment::Ident(interned_id) => {
                // NOTE: `another` searched here
                if let Some(SymbolLookupOutput { found_sym_id, .. }) = scopes::find_sym_id(
                    compiler,
                    current_scope,
                    *interned_id,
                    scope_type,
                    ScopeLookupPattern::NamespaceOnly,
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
                        match &sp_path_segs[i - 1].kind {
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
            PathSegment::Generic(_) if in_ty_expr => {
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
