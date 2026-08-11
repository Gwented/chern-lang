use std::fmt::Display;

use chrn_utils::{
    id_types::{InternedId, MemberId, ModuleId, ScopeId, SymbolId, TypeId},
    intern::Intern,
};
use lang::fmter::{Formattable, Formatted};

use crate::{
    script_compiler::ScriptCompiler,
    semantic::hir::{
        hir_concepts::{Table, Type},
        hir_symbols::{MemberSymbolKind, Symbol, SymbolKind, SymbolKindFlat, VariableState},
    },
};

//TODO: Maybe this is the point where the scope wrapper comes in
/// Structure that holds scope data
#[derive(Debug)]
pub struct ScopeInfo {
    pub scope: Scope,
    /// For debugging purposes so that the symbol of origin is known for where a namespace lookup
    /// occured, beyond just the module or scope of origin.
    pub sym_owner: Option<SymbolId>,
    pub mod_owner: ModuleId,
}

impl ScopeInfo {
    pub fn new(scope: Scope, sym_owner: Option<SymbolId>, mod_owner: ModuleId) -> ScopeInfo {
        ScopeInfo {
            scope,
            sym_owner,
            mod_owner,
        }
    }
}

pub const SCOPE_CORE: u8 = 1 << 0;
pub const SCOPE_NEUTRAL: u8 = 1 << 1;
pub const SCOPE_VAR: u8 = 1 << 2;
pub const SCOPE_NEST: u8 = 1 << 3;
pub const SCOPE_COMPLEX: u8 = 1 << 4;
pub const SCOPE_OVERRIDE: u8 = 1 << 5;
pub const SCOPE_LOCAL: u8 = 1 << 6;
pub const SCOPE_COMPILER: u8 = 1 << 7;

//WARN: Hallucinating semantics a bit. wait.

pub static SCOPE_CORE_ENCODED_SCOPES: [ScopeType; 1] = [ScopeType::Core];

/// Elements ordered to fit the languages rules of section `neutral`
pub static SCOPE_NEUTRAL_ENCODED_SCOPES: [ScopeType; 3] =
    [ScopeType::Neutral, ScopeType::Compiler, ScopeType::Core];

/// Elements ordered to fit the languages rules of section `var`
pub static SCOPE_VAR_ENCODED_SCOPES: [ScopeType; 4] = [
    ScopeType::Nest,
    ScopeType::Neutral,
    ScopeType::Compiler,
    ScopeType::Core,
];

/// Elements ordered to fit the languages rules of section `nest`
pub static SCOPE_NEST_ENCODED_SCOPES: [ScopeType; 5] = [
    ScopeType::Var,
    ScopeType::Nest,
    ScopeType::Neutral,
    ScopeType::Compiler,
    ScopeType::Core,
];

// Doesn't have itself because complex assigns properties for types, nothing more.
/// Elements ordered to fit the needs of scope `complex`
pub static SCOPE_COMPLEX_ENCODED_SCOPES: [ScopeType; 5] = [
    ScopeType::Var,
    ScopeType::Nest,
    ScopeType::Neutral,
    ScopeType::Compiler,
    ScopeType::Core,
];

/// Elements ordered to fit the needs of scope `override`
pub static SCOPE_OVERRIDE_ENCODED_SCOPES: [ScopeType; 6] = [
    ScopeType::Override,
    ScopeType::Var,
    ScopeType::Nest,
    ScopeType::Neutral,
    ScopeType::Compiler,
    ScopeType::Core,
];

//WARN: Suspicious accessibility
pub static SCOPE_LOCAL_ENCODED_SCOPES: [ScopeType; 1] = [ScopeType::Local];
pub static SCOPE_VAR_ONLY: [ScopeType; 1] = [ScopeType::Var];
pub static SCOPE_NEST_ONLY: [ScopeType; 1] = [ScopeType::Nest];

// Neutral, var, nest, and complex scopes can only access variables from neutral and nest.
// Override is unsure
#[derive(Debug)]
pub struct Scope {
    pub table: Table,
    /// Own `ScopeId`
    pub scope_id: ScopeId,
    /// `ScopeType` this scope represents
    pub scope_type: ScopeType,
    /// An `Option` scope that is intrinsically a part of this scope
    pub intrinsic_scope: Option<ScopeId>,
    /// Boolean of whether or not the scope is intrinsic
    pub is_intrinsic: bool,
    /// Pre-determined list of `SceopType`s that this scope can access.
    pub accessible_scopes: &'static [ScopeType],
}

impl Scope {
    pub(crate) fn new(
        scope_id: ScopeId,
        scope_type: ScopeType,
        is_intrinsic: bool,
        intrinsic_scope: Option<ScopeId>,
    ) -> Scope {
        let accessible_scopes = scope_type.accessible_scopes();
        Scope {
            table: Table::new(),
            scope_id,
            scope_type,
            intrinsic_scope,
            accessible_scopes,
            is_intrinsic,
            // pub visible_scopes: Vec<ScopeId>,
        }
    }

    pub(crate) fn with_table(
        scope_id: ScopeId,
        scope_type: ScopeType,
        intrinsic_scope: Option<ScopeId>,
        is_intrinsic: bool,
        table: Table,
    ) -> Scope {
        let accessible_scopes = scope_type.accessible_scopes();
        Scope {
            table,
            scope_id,
            scope_type,
            is_intrinsic,
            intrinsic_scope,
            accessible_scopes,
        }
    }
}

#[derive(Debug)]
pub struct SymbolLookupOutput {
    pub found_sym_id: SymbolId,
    pub scope_found_in: ScopeId,
}

impl SymbolLookupOutput {
    pub fn new(found_sym_id: SymbolId, scope_found_in: ScopeId) -> SymbolLookupOutput {
        SymbolLookupOutput {
            found_sym_id,
            scope_found_in,
        }
    }
}

/// Locally searches for the given name id. Locally searching in this context means solely
/// searching the scope given for the identifier due to parent relationships not existing.
pub fn find_sym_id_local(
    compiler: &ScriptCompiler,
    scope_id: ScopeId,
    target_name_id: InternedId,
) -> Option<SymbolId> {
    // There are no parent hierarchiable (Is this a word?) language semantics yet other than single
    // local scopes so this is just a single scope search.
    let local_scope = &compiler.scopes[scope_id].scope;

    for (current_name_id, current_sym_id) in &local_scope.table.interned_to_sym {
        if *current_name_id == target_name_id {
            return Some(*current_sym_id);
        }
    }

    None
}

//NOTE: Exists for separation reasons due to the compiler becoming bloated in many forms
/// Get's `TypeId` associated with the `InternedId` given if possible
pub fn find_type_id(
    compiler: &ScriptCompiler,
    owner_id: ModuleId,
    target_name_id: InternedId,
    scope_type: ScopeType,
    lookup_pattern: ScopeLookupPattern,
) -> Option<TypeId> {
    let current_mod = &compiler.mods[owner_id];
    let accessible_scopes = scope_type.accessible_scopes();
    let accessible_scopes = match lookup_pattern {
        //WARN: Core is always the last scope so this is kept so an owned vec isn't created
        //May change
        ScopeLookupPattern::NamespaceOnly if current_mod.region_id.is_some() => {
            &accessible_scopes[..accessible_scopes.len() - 1]
        }
        // If it's core then it'll only have access to core anyways so this is fine
        ScopeLookupPattern::NoRestrictions | ScopeLookupPattern::NamespaceOnly => accessible_scopes,
        ScopeLookupPattern::OnlyVar => &SCOPE_VAR_ONLY,
        ScopeLookupPattern::OnlyNest => &SCOPE_NEST_ONLY,
    };
    // Loops over all allowed scopes and checks their individual namespaces

    let mut default_return: Option<TypeId> = None;

    for allowed_scope_type in accessible_scopes.iter().copied() {
        // In this scenario the scope may or may not exist since this could be used from
        // another module
        if let Some(scope_info) = find_scope(compiler, allowed_scope_type, current_mod.mod_id) {
            //NOTE: Make sure I work
            if let Some(current_sym_id) = scope_info
                .scope
                .table
                .interned_to_sym
                .get(&target_name_id)
                .copied()
            {
                match &compiler.symbols[current_sym_id].kind {
                    SymbolKind::Type(type_id) => return Some(*type_id),
                    SymbolKind::Variable(var_id) => {
                        let type_id = match compiler.variables[*var_id].state {
                            VariableState::ReservedTypeSlot(type_id) => type_id,
                            VariableState::Known(val_id) => compiler.values[val_id].type_id,
                        };

                        return Some(type_id);
                    }
                    SymbolKind::ExternType | SymbolKind::Namespace | SymbolKind::Directive(_) => {
                        return None;
                    }
                }
            }
        }
    }

    default_return
}

/// Searches the given module for the given `ScopeType` by iterating through it's scopes
/// and returns `Some` if it's found, `None` otherwise.
pub fn find_scope(
    compiler: &ScriptCompiler,
    target: ScopeType,
    owner_id: ModuleId,
) -> Option<&ScopeInfo> {
    let mod_owner = &compiler.mods[owner_id];
    for scope_id in &mod_owner.scopes {
        let scope_info = &compiler.scopes[*scope_id];
        if scope_info.scope.scope_type == target {
            return Some(scope_info);
        }
    }

    None
}

/// - compiler: The environment to seaerch in
/// - associated_scope: The type of scope to search which could differ depending on if the scope
/// belongs to a module, symbol, etc.
/// - target_name_id: The identifier to search for in the given scope
/// - scope_type: The type of scope this search was started from
/// - lookup_pattern: How much access the lookup should have
///
/// - On `Some`: Returns Symbol found and the `ScopeId` from the scope it was found in
/// - Returns `None` when no symbol with the target identifier was found under the constraints
/// given.
pub fn find_sym_id(
    compiler: &ScriptCompiler,
    associated_scope: AssociatedScopeKind,
    target_name_id: InternedId,
    scope_type: ScopeType,
    lookup_pat: ScopeLookupPattern,
    lookup_pref: LookupPreferenceFlags,
    // Named struct maybe
) -> Option<SymbolLookupOutput> {
    // Avoiding vector allocations right now so it can just use a pointer offset instead based off
    // of hard-coded truths but will probably just, not do that.
    match associated_scope {
        AssociatedScopeKind::Module(mod_id) => {
            let current_mod = &compiler.mods[mod_id];

            let accessible_scopes = scope_type.accessible_scopes();
            let accessible_scopes = match lookup_pat {
                ScopeLookupPattern::NamespaceOnly if current_mod.region_id.is_some() => {
                    &accessible_scopes[..accessible_scopes.len() - 1]
                }
                // If it's core then it'll only have access to core anyways so this is fine
                ScopeLookupPattern::NoRestrictions | ScopeLookupPattern::NamespaceOnly => {
                    accessible_scopes
                }
                ScopeLookupPattern::OnlyVar => &SCOPE_VAR_ONLY,
                ScopeLookupPattern::OnlyNest => &SCOPE_NEST_ONLY,
            };

            // `another` failed here
            // if target_name_id.id == 50 {
            //     dbg!(lookup_pattern, associated_scope, accessible_scopes);
            //     panic!();
            // }

            // If a preferred is given, the most recent same ident symbol found that is not
            // preferred is stored so that it can be returned if the preferred symbol was never found.
            // Compromise!
            let mut default_return: Option<SymbolLookupOutput> = None;

            for allowed_scope_type in accessible_scopes {
                if let Some(scope_info) = find_scope(compiler, *allowed_scope_type, mod_id) {
                    // Found a symbol in the particular scope under the given identifier
                    if let Some(sym_id) = scope_info
                        .scope
                        .table
                        .interned_to_sym
                        .get(&target_name_id)
                        .copied()
                    {
                        // Storing what would be the default return if preferences aren't matched
                        default_return =
                            Some(SymbolLookupOutput::new(sym_id, scope_info.scope.scope_id));

                        // Only looping again if not matched
                        let flat = compiler.symbols[sym_id].kind.to_flat();
                        if !lookup_pref.is_preferred(flat) {
                            continue;
                        };

                        // Preferred symbol found
                        return default_return;
                    }

                    //TODO: Make sure this works as intended
                    if let Some(intrinsic_scope_id) = scope_info.scope.intrinsic_scope {
                        let intrinsic_scope = &compiler.scopes[intrinsic_scope_id].scope;

                        // So if in override, but searching complex, it will not try to look at the
                        // intrinsic scope unless it's looking at it's own scope
                        //
                        // Further meaning: Any scope can be given an intrinsic scope. But, it is
                        // only semantically viable to look inside of that intrinsic scope if the
                        // current scope is related. So, if our current scope is complex and we see
                        // an intrinsic scope for override, this disallows that search because
                        // complex should not allow override intrinsic symbols to be used inside complex.
                        if scope_type == *allowed_scope_type {
                            if let Some(sym_id) = intrinsic_scope
                                .table
                                .interned_to_sym
                                .get(&target_name_id)
                                .copied()
                            {
                                //WARN: This is a bit dangerous since it kinda depends on preference
                                //managers having this exact code.
                                let flat = compiler.symbols[sym_id].kind.to_flat();
                                default_return = Some(SymbolLookupOutput::new(
                                    sym_id,
                                    scope_info.scope.scope_id,
                                ));

                                if !lookup_pref.is_preferred(flat) {
                                    continue;
                                }

                                return default_return;
                            }
                        }
                    }
                }
            }

            // If no preferences are matched, returns said default, returning `None` if no default
            // was found
            return default_return;
        }
        AssociatedScopeKind::Scope(scope_id) => {
            let scope = &compiler.scopes[scope_id].scope;
            if let Some(sym_id) = scope.table.interned_to_sym.get(&target_name_id) {
                return Some(SymbolLookupOutput::new(*sym_id, scope_id));
            }

            //TODO: Make sure this works as intended
            //Pretty sure it does work since the java namespace was found, which is only available
            //intrinsically.
            if let Some(intrinsic_scope_id) = scope.intrinsic_scope {
                let intrinsic_scope = &compiler.scopes[intrinsic_scope_id].scope;

                if let Some(sym_id) = intrinsic_scope.table.interned_to_sym.get(&target_name_id) {
                    return Some(SymbolLookupOutput::new(*sym_id, intrinsic_scope_id));
                }
            }
        }
    }
    // `another` failed here

    None
}

/// Finds all symbols under the given string identifier and returns their symbol ids
pub fn find_symbols_named<'a>(
    compiler: &'a ScriptCompiler,
    target_name_id: InternedId,
) -> (Vec<SymbolId>, Vec<&'a MemberSymbolKind>) {
    let mut found: Vec<SymbolId> = Vec::new();
    for sym in &compiler.symbols.items {
        if sym.name_id == target_name_id {
            found.push(sym.sym_id);
        }
    }

    todo!()
}

/// Finds all symbols under the given string identifier and returns their symbols as references
pub fn find_symbols_named_ref<'a>(
    compiler: &'a ScriptCompiler,
    target_name_id: InternedId,
) -> (Vec<&'a Symbol>, Vec<&'a MemberSymbolKind>) {
    let mut found_syms: Vec<&Symbol> = Vec::new();
    let mut found_members: Vec<&MemberSymbolKind> = Vec::new();

    for sym in &compiler.symbols.items {
        if sym.name_id == target_name_id {
            found_syms.push(&sym);
        }

        match sym.kind {
            SymbolKind::Type(type_id) => {
                let new_members = collect_inner_symbols(compiler, type_id, target_name_id);
                found_members.append(
                    &mut new_members
                        .iter()
                        .map(|m_id| &compiler.sym_members[*m_id])
                        .collect(),
                );
            }
            // To avoid allocating Vec if there is no possible inner
            _ => (),
        }
    }

    (found_syms, found_members)
}

// To avoid O(symbols) search by allowing module specific lookups
/// Finds all symbols under the given string identifier within the specific module given
pub fn find_symbols_named_from_module<'a>(
    compiler: &'a ScriptCompiler,
    interner: &Intern,
    target_mod_id: ModuleId,
    ident: &str,
) -> (Vec<SymbolId>, Vec<&'a MemberSymbolKind>) {
    let target_name_id = match interner.try_search_str(ident) {
        Some(id) => id,
        None => return (Vec::new(), Vec::new()),
    };

    let module = &compiler.mods[target_mod_id];

    let mut found_symbols = Vec::new();
    for scope_id in &module.scopes {
        let scope = &compiler.scopes[*scope_id].scope;
        for (interned_id, sym_id) in &scope.table.interned_to_sym {
            if *interned_id == target_name_id {
                found_symbols.push(*sym_id);
            }
        }
    }

    todo!()
}

/// Finds all symbols under the given string identifier within the specific module given
pub fn find_symbols_named_from_module_ref<'a>(
    compiler: &'a ScriptCompiler,
    interner: &Intern,
    target_mod_id: ModuleId,
    ident: &str,
) -> (Vec<&'a Symbol>, Vec<&'a MemberSymbolKind>) {
    let target_name_id = match interner.try_search_str(ident) {
        Some(id) => id,
        None => return (Vec::new(), Vec::new()),
    };

    let mut found_syms: Vec<&Symbol> = Vec::new();
    let mut found_members: Vec<&MemberSymbolKind> = Vec::new();

    let module = &compiler.mods[target_mod_id];

    for scope_id in &module.scopes {
        let scope = &compiler.scopes[*scope_id].scope;
        for sym_id in scope.table.interned_to_sym.values() {
            let sym = &compiler.symbols[*sym_id];

            if sym.name_id == target_name_id {
                found_syms.push(&sym);
            }

            match sym.kind {
                SymbolKind::Type(type_id) => {
                    let new_members = collect_inner_symbols(compiler, type_id, target_name_id);
                    found_members.append(
                        &mut new_members
                            .iter()
                            .map(|m_id| &compiler.sym_members[*m_id])
                            .collect(),
                    );
                }
                // To avoid allocating Vec if there is no possible inner
                _ => (),
            }
        }
    }

    (found_syms, found_members)
}

fn collect_inner_symbols<'a>(
    compiler: &'a ScriptCompiler,
    type_id: TypeId,
    target_name_id: InternedId,
) -> Vec<MemberId> {
    // Um, 20 mb?
    let mut found: Vec<MemberId> = Vec::new();

    match &compiler.types[type_id].ty {
        Type::Struct(struct_def) => {
            for member_id in &struct_def.fields {
                let field = compiler.get_field(*member_id);
                if field.name_id == target_name_id {
                    found.push(field.member_id);
                }
            }
        }
        Type::Enum(enum_def) => {
            for member_id in &enum_def.variants {
                let variant = compiler.get_variant(*member_id);
                if variant.name_id == target_name_id {
                    found.push(variant.member_id);
                }
            }
        }
        Type::Alias(alias_def) => {
            // Not sure what to do with params yet
            // for thing in &alias_def.params {}
        }
        Type::Deferred(inner_type_id) => found.append(&mut collect_inner_symbols(
            compiler,
            *inner_type_id,
            target_name_id,
        )),
        // Function isn't possible as an inner
        Type::Func(_)
        | Type::TypeDef(_)
        | Type::Boundaries(_)
        | Type::BuiltinTypeInfo(_)
        | Type::Unknown => (),
    }

    found
}

/// Enum representing all kinds of scopes usable in chrn
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum ScopeType {
    Compiler,
    Core,
    Local,
    Neutral,
    Var,
    Nest,
    Complex,
    Override,
}

impl ScopeType {
    /// Direct representation of how the language views scope accessibility.
    /// `needs_global` purely exists for all scope accessibility reasons
    pub fn accessible_scopes(self) -> &'static [ScopeType] {
        match self {
            ScopeType::Core => &SCOPE_CORE_ENCODED_SCOPES,
            // Mainly for internal usage, not an actual program recognizable scope
            // Neutral can only access neutral because this section is purely for declaring and
            // using in other sections
            ScopeType::Neutral => &SCOPE_NEUTRAL_ENCODED_SCOPES,
            ScopeType::Var => &SCOPE_VAR_ENCODED_SCOPES,
            ScopeType::Override => &SCOPE_OVERRIDE_ENCODED_SCOPES,
            ScopeType::Nest => &SCOPE_NEST_ENCODED_SCOPES,
            ScopeType::Complex => &SCOPE_COMPLEX_ENCODED_SCOPES,
            ScopeType::Local => &SCOPE_LOCAL_ENCODED_SCOPES,
            // Should be a recognized builtin at this point
            ScopeType::Compiler => &[],
        }
    }

    pub(crate) fn to_u8(self) -> u8 {
        match self {
            ScopeType::Core => SCOPE_CORE,
            ScopeType::Neutral => SCOPE_NEUTRAL,
            ScopeType::Var => SCOPE_VAR,
            ScopeType::Nest => SCOPE_NEST,
            ScopeType::Complex => SCOPE_COMPLEX,
            ScopeType::Override => SCOPE_OVERRIDE,
            ScopeType::Local => SCOPE_LOCAL,
            ScopeType::Compiler => SCOPE_COMPILER,
        }
    }

    pub(crate) fn has_intrinsic_scope(self) -> bool {
        match self {
            ScopeType::Complex | ScopeType::Override => true,
            ScopeType::Core
            | ScopeType::Local
            | ScopeType::Neutral
            | ScopeType::Nest
            // I don't know does it?
            | ScopeType::Compiler
            | ScopeType::Var => false,
        }
    }
}

// Maybe Only(ScopeType)
/// This enum is intended to disallow core defined values from being searched for when syntax such
/// as "main::i32" is used. i32 is not owned by main, but innately main is attached to core, meaning
/// without the explicit noting of whether we are searching a singular module's namespace it would
/// innately allow for main.i32 to be interpreted the same as if just i32 was written, which is
/// wrong since the namespace "main" owns no such thing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScopeLookupPattern {
    /// Applies no restriction to lookups. Meaning, core is automatically searched since it's
    /// intrinsic, any scope's accessible scopes can be searched with no restriction.
    NoRestrictions,
    // WHat
    /// Restricts lookup to only search what is within the given namespace, which restricts modules
    /// such as core, or anything not declared within the symbol's scope containment?
    NamespaceOnly,
    /// Lookup that not only allows for `nest` to be searched, but also enforces it's the only section
    /// that can be searched
    OnlyNest,
    /// Lookup that not only allows for `var` to be searched, but also enforces it's the only section
    /// that can be searched
    OnlyVar,
}

// TODO: Formattable
impl Display for ScopeType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ScopeType::Core => write!(f, "core"),
            ScopeType::Neutral => write!(f, "neutral"),
            ScopeType::Var => write!(f, "var"),
            ScopeType::Nest => write!(f, "nest"),
            ScopeType::Complex => write!(f, "complex"),
            ScopeType::Override => write!(f, "override"),
            ScopeType::Local => write!(f, "local"),
            ScopeType::Compiler => write!(f, "compiler"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Scope type specifically for if a symbol has an associated scope tied to it
pub enum AssociatedScopeKind {
    // A bit redundant since odules already hold themselves as a scope
    /// Meaning the scope is inside of a module's vector of `ScopeId`
    Module(ModuleId),
    /// Meaning the scope is just attached to a symbol's namespace
    Scope(ScopeId),
}

impl Formattable for AssociatedScopeKind {
    fn to_fmt(&self) -> lang::fmter::Formatted {
        match self {
            AssociatedScopeKind::Module(_) => Formatted::Module,
            AssociatedScopeKind::Scope(_) => Formatted::Namespace,
        }
    }
}

pub struct IntrinsicRegistry {
    pub core_mod_id: ModuleId,
    pub override_scope_id: Option<ScopeId>,
}

impl IntrinsicRegistry {
    pub fn new(core_mod_id: ModuleId, override_scope_id: Option<ScopeId>) -> IntrinsicRegistry {
        IntrinsicRegistry {
            core_mod_id,
            override_scope_id,
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct LookupPreferenceFlags {
    /// If `None`, no preference is accounted for meaning all preference checks are `true`
    flags: Option<u16>,
}

// This is more like a general purpose set of flags since it doesn't really matter if flat kinds are
// used or not
impl LookupPreferenceFlags {
    pub const TYPE: u16 = 1 << 0;
    pub const VARIABLE: u16 = 1 << 1;
    pub const NAMESPACE: u16 = 1 << 2;
    pub const DIRECTIVE: u16 = 1 << 3;
    pub const EXTERN_TYPE: u16 = 1 << 4;

    pub fn new(flags: Option<u16>) -> Self {
        Self { flags }
    }

    /// Creates lookup preference with no preferred options
    pub fn none() -> LookupPreferenceFlags {
        LookupPreferenceFlags::new(None)
    }

    pub fn is_none(self) -> bool {
        self.flags.is_none()
    }

    /// Checks if the `SymbolKindFlat` converted to a valid set of bits for `LookupPreferenceFlags`
    /// is contained within `self`
    pub fn is_preferred(self, kind: SymbolKindFlat) -> bool {
        if let Some(flags) = self.flags {
            flags & flat_sym_kind_to_preferred_bits(kind) != 0
        } else {
            // No options chosen. Anything attempted to be matched to a `None` preference succeeds.
            true
        }
    }
}

/// Local function to turn `SymbolKindFlat` into a preferred option.
/// This exists because the `to_bits()` from flat symbols are just direct mappings, meaning there is
/// no signifying bit usable to say "No options selected", hence the explicit translation layer here.
const fn flat_sym_kind_to_preferred_bits(kind: SymbolKindFlat) -> u16 {
    match kind {
        SymbolKindFlat::Type => LookupPreferenceFlags::TYPE,
        SymbolKindFlat::Variable => LookupPreferenceFlags::VARIABLE,
        SymbolKindFlat::Namespace => LookupPreferenceFlags::NAMESPACE,
        SymbolKindFlat::Directive => LookupPreferenceFlags::DIRECTIVE,
        SymbolKindFlat::ExternType => LookupPreferenceFlags::EXTERN_TYPE,
    }
}

// // TODO: Make bit-wise. In override we lookup with the intention of a namespace OR type.
// /// The type of lookup outcome to prefer.
// /// For example, if there is a module symbol called "module" and a variable "let module = 4",
// /// in the scenario of "module::Type" if it sees the variable first, it stores it but tries to
// /// search for the preferred type first, if not found, it will return the variable.
// #[derive(Debug, PartialEq, Eq, Clone, Copy)]
// pub enum LookupPreference {
//     /// No preference is accounted for, returns the first symbol it finds.
//     None,
//     /// e
//     Type,
//     Variable,
//     Namespace,
// }
//
// // Ok but what if it was bit-wise and `SymbolKind` had a to_bits and we instead made sets
// // Please we haven't even used it yet
// impl LookupPreference {
//     pub fn is_none(self) -> bool {
//         self == LookupPreference::None
//     }
//     /// Checks if the given `SymbolKindFlat` is preferred by `self`
//     pub fn is_preferred(self, kind: SymbolKindFlat) -> bool {
//         match self {
//             // Nothing is preferred so all are valid
//             LookupPreference::None => true,
//             LookupPreference::Type => match kind {
//                 SymbolKindFlat::Type => true,
//                 _ => false,
//             },
//             LookupPreference::Variable => match kind {
//                 SymbolKindFlat::Variable => true,
//                 _ => false,
//             },
//             LookupPreference::Namespace => match kind {
//                 SymbolKindFlat::Namespace => true,
//                 _ => false,
//             },
//         }
//     }
// }
