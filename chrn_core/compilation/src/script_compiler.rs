// TODO: MAYBE eventually change from SipHash
// What is a hash?
pub mod reporter;
pub mod script_compiler_store;
pub mod script_compiler_summary;
use chrn_utils::{
    arena::Arena,
    budget::mem_cost::MemoryCost,
    id_types::{
        ConfigRootId, DirectiveId, ExprId, InternedId, MemberId, ModuleId, ScopeId, SymbolId,
        TypeId, ValueId, VariableId,
    },
    intern, loop_abort,
    source_map::source_span::SourceSpan,
};
use lang::{
    directives::{Directive, TypeDirective},
    types::{boundaries::TypeBoundaryFlags, builtins::BuiltinType},
    values::ValueInfo,
};

use crate::{
    constraints::ArgConstraint,
    lookup::scopes::{self, AssociatedScopeKind, IntrinsicRegistry, Scope, ScopeInfo, ScopeType},
    modules::{Bind, Import, ImportKind, Module, ModuleState},
    resolvers::resolver_state::ResolverState,
    semantic::hir::{
        hir_concepts::{
            AliasDef, BuiltinTypeInfo, ConfigDefMember, ConfigDefRoot, EnumDef, FieldRepre,
            FuncDef, FuncKind, MemberSymbolKind, OptionAssignmentMember, OptionAssignmentRoot,
            StructDef, Symbol, SymbolKind, SymbolOrigin, Table, Type, TypeDef, TypeInfo, VarDef,
            VariableState, VariantRepre,
        },
        hir_exprs::ResolvedExpr,
    },
};

// Should this be in utils?
/// Script compiler that holds all essential data for incremental updates through resolution
pub struct ScriptCompiler {
    /// Optional bind statement that is obtained from the main module
    // Maybe the module should keep it's bind info rather than give it to the compiler so that the
    // information isn't lossy and contextual
    pub bind: Option<Bind>,
    /// Module name to module id mapping to index module array. import `as` aliases are also stored here
    // This feels out of place
    // Can this be removed? Probably.
    // pub mod_map: HashMap<PathId, ModuleId>,
    /// All modules found during compilation
    pub mods: Arena<Module, ModuleId>,
    /// Type table which contains every module's stored types
    pub types: Arena<TypeInfo, TypeId>,
    /// All values that were cached
    pub values: Arena<ValueInfo, ValueId>,
    /// All expressions that were found
    pub exprs: Arena<ResolvedExpr, ExprId>,
    /// All symbols that were found
    pub symbols: Arena<Symbol, SymbolId>,
    /// All symbols considered a "member" of another. This is here to serve the same purpose of a
    /// collection that would be considered fields, but more general since the language is small
    /// scale and would likely not benefit much from such a wide variety of collections.
    pub members: Arena<MemberSymbolKind, MemberId>,
    /// All variables that were found
    pub variables: Arena<VarDef, VariableId>,
    /// All user defined configuration. Is considered it's own class instead of a type since it
    /// behaves uniquely
    pub cfgs: Arena<ConfigDefRoot, ConfigRootId>,
    /// All directives that were found
    pub directives: Arena<Directive, DirectiveId>,
    /// Scope arena
    pub scopes: Arena<ScopeInfo, ScopeId>,
    /// Information regarding intrinsic data such as core's `ModuleId`
    pub intrinsic_registry: IntrinsicRegistry,
    /// The current stage the compiler is in
    pub resolver_state: ResolverState,
}

// -- CORE TYPE CONSTANTS --
//NOTE: I think these can be removed. Maybe. I don't know actually.
pub const CORE_I8: u32 = 0;
pub const CORE_U8: u32 = 1;
pub const CORE_I16: u32 = 2;
pub const CORE_U16: u32 = 3;
pub const CORE_F16: u32 = 4;
pub const CORE_I32: u32 = 5;
pub const CORE_U32: u32 = 6;
pub const CORE_F32: u32 = 7;
pub const CORE_I64: u32 = 8;
pub const CORE_U64: u32 = 9;
pub const CORE_F64: u32 = 10;
pub const CORE_I128: u32 = 11;
pub const CORE_U128: u32 = 12;
pub const CORE_F128: u32 = 13;
pub const CORE_SIZED: u32 = 14;
pub const CORE_UNSIZED: u32 = 15;
pub const CORE_STR: u32 = 16;
pub const CORE_CHAR: u32 = 17;
pub const CORE_NIL: u32 = 18;
pub const CORE_BOOL: u32 = 19;
pub const CORE_BIGINT: u32 = 20;
pub const CORE_BIGFLOAT: u32 = 21;
pub const CORE_RUNTIME: u32 = 22;
pub const CORE_UNKNOWN: u32 = 23;
// pub const CORE_CHARACTER_MAPPABLE: u32 = 24;
// pub const CORE_LIST: u32 = 23;
// pub const CORE_SET: u32 = 24;
// pub const CORE_MAP: u32 = 25;
// pub const CORE_TUPLE: u32 = 26;
// Called idx but is u32...

// --  DIRECTIVE CONSTANTS --
pub const DIRECTIVE_WARN_IDX: usize = 0;
pub const DIRECTIVE_IGNORE_IDX: usize = 1;
pub const DIRECTIVE_SCIENT_IDX: usize = 2;
pub const DIRECTIVE_HEX_IDX: usize = 3;
pub const DIRECTIVE_BIN_IDX: usize = 4;
pub const DIRECTIVE_OCTAL_IDX: usize = 5;
pub const DIRECTIVE_UNICODE_IDX: usize = 6;

pub fn directive_to_id(directive: &Directive) -> DirectiveId {
    let idx = match directive {
        Directive::Warn => DIRECTIVE_WARN_IDX,
        Directive::Ignore => DIRECTIVE_IGNORE_IDX,
        Directive::Type(type_directive) => match type_directive {
            TypeDirective::Scient => DIRECTIVE_SCIENT_IDX,
            TypeDirective::Hex => DIRECTIVE_HEX_IDX,
            TypeDirective::Bin => DIRECTIVE_BIN_IDX,
            TypeDirective::Octal => DIRECTIVE_OCTAL_IDX,
            TypeDirective::Unicode => DIRECTIVE_UNICODE_IDX,
        },
    };

    DirectiveId::new(idx as u32)
}

// ---- DIRECTIVE CONSTANTS END ---

// NOTE: May turn this into an innate option type inside of HIR
// Ok now this really needs to be an option
// pub const VALUE_UNKNOWN: usize = 0;

impl ScriptCompiler {
    //FIX: Arbitrary ordering of pushes tied to the actual order of the enums. Should not be tied
    //to anything, similar to the interner's constants.
    /// Loads core library and builds script specific compiler with parameters given
    pub fn init(bind: Option<Bind>, mods: Arena<Module, ModuleId>) -> ScriptCompiler {
        // dbg!(&mods[ModuleId::new(0)]);
        //TEST:
        let core_mod_id = ModuleId::new(mods.len() as u32);
        let intrinsic_registry = IntrinsicRegistry::new(core_mod_id, None);

        let mut compiler = ScriptCompiler {
            bind,
            mods,
            types: Arena::new(),
            values: Arena::new(),
            exprs: Arena::new(),
            symbols: Arena::new(),
            variables: Arena::new(),
            members: Arena::new(),
            cfgs: Arena::new(),
            scopes: Arena::new(),
            directives: Arena::new(),
            //TEST:
            intrinsic_registry,
            resolver_state: ResolverState::NAMESPACE,
        };

        // Should this lazy load the section intrinsics though?
        // Yuppy
        Self::load_core(&mut compiler);
        Self::load_directives(&mut compiler);
        Self::create_module_symbols(&mut compiler);

        compiler
    }

    /// Creates the symbols needed for modules to be able to access to access their imports
    ///
    /// This is done by going through each module and injecting the module symbols found during the
    /// initial module dependency graph stage.
    fn create_module_symbols(compiler: &mut ScriptCompiler) {
        // Loops through all modules, registering themselves as a symbol to themselves, iterating
        // through their imports to then inject those symbols as modules that can be looked up

        // So, if we have main AND other
        // It registers "main" as a module symbol so usage such as "main::MainType" can be used
        // It then registers a symbol for "other" so that the same "other::OtherType" semantics can
        // be done
        // If there is an alias, that is also ensured to be pushed as a symbol connected to the
        // module "other"
        for i in 0..compiler.mods.len() {
            let current_mod_id = ModuleId::new(i as u32);
            let module = &compiler.mods[current_mod_id];

            // Avoiding borrow issues by storing the ids earlier
            let current_mod_name_id = module.name_id;
            let current_mod_id = module.mod_id;

            // Pushing the module symbol inside of itself. So if we're indexing module `main`, we
            // would be pushing `main` inside of itself, once, as a known symbol.
            let sym_id = SymbolId::new(compiler.symbols.len() as u32);
            let symbol = Symbol::new(
                current_mod_name_id,
                sym_id,
                None,
                SymbolOrigin::Module(current_mod_id),
                true,
                Some(AssociatedScopeKind::Module(current_mod_id)),
                ScopeType::Neutral,
                SymbolKind::Namespace,
            );

            // Module symbols go into the neutral scope because, uh
            // Um
            let scope_id = compiler.push_scope(ScopeType::Neutral, current_mod_id);
            let scope = &mut compiler.get_scope_mut(scope_id).scope;
            scope
                .table
                .interned_to_sym
                .insert(current_mod_name_id, sym_id);
            compiler.symbols.push(symbol);

            // Re-borrowing for iteration
            let module = &compiler.mods[current_mod_id];

            // Clone..
            for import in module.imports.clone() {
                let mod_id = match import.kind {
                    ImportKind::Source(_, m_id) => m_id,
                    ImportKind::ErrorSource(_) => continue,
                    // Should not stay as unresolved source, should be converted to err or a
                    // resolved source
                    ImportKind::UnresolvedSource(_) => unreachable!(),
                    // Core DOES exist at this point, but is already given so this needs to be skipped.
                    // Or is that not the case?
                    // We'll see
                    ImportKind::Core(m_id) => m_id,
                };

                let import_sym_id = SymbolId::new(compiler.symbols.len() as u32);
                // Pushing any imports found within the given module
                let symbol = Symbol::new(
                    import.name_id,
                    import_sym_id,
                    None,
                    SymbolOrigin::Module(current_mod_id),
                    true,
                    Some(AssociatedScopeKind::Module(mod_id)),
                    ScopeType::Neutral,
                    SymbolKind::Namespace,
                );

                // Module symbols go into the neutral scope because, uh
                // Um
                let scope_id = compiler.push_scope(ScopeType::Neutral, current_mod_id);

                let scope = &mut compiler.get_scope_mut(scope_id).scope;
                scope
                    .table
                    .interned_to_sym
                    .insert(import.name_id, import_sym_id);
                compiler.symbols.push(symbol);

                // Maybe it can just point to the import directly instead of needing it's own
                // symbol?
                if let Some(alias_name_id) = import.alias_id {
                    let alias_sym_id = SymbolId::new(compiler.symbols.len() as u32);

                    // Pushing the alias associated with the import symbol if present
                    let symbol = Symbol::new(
                        alias_name_id,
                        alias_sym_id,
                        None,
                        SymbolOrigin::Module(current_mod_id),
                        true,
                        Some(AssociatedScopeKind::Module(mod_id)),
                        ScopeType::Neutral,
                        SymbolKind::Namespace,
                    );

                    let scope = &mut compiler.get_scope_mut(scope_id).scope;
                    scope
                        .table
                        .interned_to_sym
                        .insert(alias_name_id, alias_sym_id);

                    compiler.symbols.push(symbol);
                }
            }
        }
    }

    pub(super) fn get_typedef(&self, sym_id: SymbolId) -> &TypeDef {
        match &self.symbols[sym_id] {
            sym_info => match &sym_info.kind {
                SymbolKind::Type(type_id) => match &self.types[*type_id].ty {
                    Type::TypeDef(type_def) => type_def,
                    _ => unreachable!(),
                },
                _ => unreachable!(),
            },
        }
    }

    pub(super) fn get_typedef_mut(&mut self, sym_id: SymbolId) -> &mut TypeDef {
        match &self.symbols[sym_id] {
            sym_info => match &sym_info.kind {
                SymbolKind::Type(type_id) => match &mut self.types[*type_id].ty {
                    Type::TypeDef(type_def) => type_def,
                    _ => unreachable!(),
                },
                _ => unreachable!(),
            },
        }
    }

    pub(super) fn get_struct(&self, sym_id: SymbolId) -> &StructDef {
        match &self.symbols[sym_id] {
            sym_info => match &sym_info.kind {
                SymbolKind::Type(type_id) => match &self.types[*type_id].ty {
                    Type::Struct(struct_def) => struct_def,
                    _ => unreachable!(),
                },
                _ => unreachable!(),
            },
        }
    }

    pub(super) fn get_struct_mut(&mut self, sym_id: SymbolId) -> &mut StructDef {
        match self.symbols.get_mut(sym_id).expect("misusage") {
            sym_info => match &mut sym_info.kind {
                SymbolKind::Type(type_id) => match &mut self.types[*type_id].ty {
                    Type::Struct(struct_def) => struct_def,
                    _ => unreachable!(),
                },
                _ => unreachable!(),
            },
        }
    }

    pub(super) fn get_func(&self, sym_id: SymbolId) -> &FuncDef {
        match &self.symbols[sym_id] {
            sym_info => match &sym_info.kind {
                SymbolKind::Type(type_id) => match &self.types[*type_id].ty {
                    Type::Func(func_def) => func_def,
                    _ => unreachable!(),
                },
                _ => unreachable!(),
            },
        }
    }

    pub(super) fn get_func_mut(&mut self, sym_id: SymbolId) -> &mut FuncDef {
        match self.symbols.get_mut(sym_id).expect("misusage") {
            sym_info => match &mut sym_info.kind {
                SymbolKind::Type(type_id) => match &mut self.types[*type_id].ty {
                    Type::Func(func_def) => func_def,
                    _ => unreachable!(),
                },
                _ => unreachable!(),
            },
        }
    }

    pub(super) fn get_enum(&self, sym_id: SymbolId) -> &EnumDef {
        match &self.symbols[sym_id] {
            sym_info => match &sym_info.kind {
                SymbolKind::Type(type_id) => match &self.types[*type_id].ty {
                    Type::Enum(enum_def) => enum_def,
                    _ => unreachable!(),
                },
                _ => unreachable!(),
            },
        }
    }

    pub(super) fn get_enum_mut(&mut self, sym_id: SymbolId) -> &mut EnumDef {
        match self.symbols.get_mut(sym_id).expect("misusage") {
            sym_info => match &mut sym_info.kind {
                SymbolKind::Type(type_id) => match &mut self.types[*type_id].ty {
                    Type::Enum(enum_def) => enum_def,
                    _ => unreachable!(),
                },
                _ => unreachable!(),
            },
        }
    }

    pub(super) fn get_alias(&self, sym_id: SymbolId) -> &AliasDef {
        match &self.symbols[sym_id] {
            sym_info => match &sym_info.kind {
                SymbolKind::Type(type_id) => match &self.types[*type_id].ty {
                    Type::Alias(alias_def) => alias_def,
                    _ => unreachable!(),
                },
                _ => unreachable!(),
            },
        }
    }

    pub(super) fn get_alias_mut(&mut self, sym_id: SymbolId) -> &mut AliasDef {
        match self.symbols.get_mut(sym_id).expect("Misusage") {
            sym_info => match &mut sym_info.kind {
                SymbolKind::Type(type_id) => match &mut self.types[*type_id].ty {
                    Type::Alias(alias_def) => alias_def,
                    _ => unreachable!(),
                },
                _ => unreachable!(),
            },
        }
    }

    /// Assumes the symbol given is a variable, meaning a symbol with a value inside of it
    pub(super) fn get_var(&self, sym_id: SymbolId) -> &VarDef {
        match &self.symbols[sym_id] {
            sym_info => match &sym_info.kind {
                SymbolKind::Variable(var_id) => &self.variables[*var_id],
                _ => unreachable!(),
            },
        }
    }

    /// Assumes the symbol given is a variable, meaning a symbol with a value inside of it
    pub(super) fn get_var_mut(&mut self, sym_id: SymbolId) -> &mut VarDef {
        match &self.symbols[sym_id] {
            sym_info => match &sym_info.kind {
                SymbolKind::Variable(var_id) => &mut self.variables[*var_id],
                _ => unreachable!(),
            },
        }
    }

    pub(super) fn get_cfg_def_root(&self, sym_id: SymbolId) -> &ConfigDefRoot {
        match &self.symbols[sym_id] {
            sym_info => match &sym_info.kind {
                SymbolKind::Config(cfg_id) => &self.cfgs[*cfg_id],
                _ => unreachable!(),
            },
        }
    }

    pub(super) fn get_cfg_def_mut(&mut self, sym_id: SymbolId) -> &mut ConfigDefRoot {
        match &self.symbols[sym_id] {
            sym_info => match &sym_info.kind {
                SymbolKind::Config(cfg_id) => &mut self.cfgs[*cfg_id],
                _ => unreachable!(),
            },
        }
    }

    pub(super) fn get_directive(&self, sym_id: SymbolId) -> &Directive {
        match &self.symbols[sym_id] {
            sym_info => match &sym_info.kind {
                SymbolKind::Directive(directive_id) => &self.directives[*directive_id],
                _ => unreachable!(),
            },
        }
    }

    // pub(super) fn get_cfg_schema(&self, cfg_id: ConfigId) -> &ConfigSchema {
    //     match &self.configs[cfg_id ] {
    //         ConfigKind::Schema(cfg_schema) => cfg_schema,
    //         ConfigKind::Def(_) => unreachable!(),
    //     }
    // }
    //
    // pub(super) fn get_cfg_schema_mut(&mut self, cfg_id: ConfigId) -> &mut ConfigSchema {
    //     match &mut self.configs[cfg_id ] {
    //         ConfigKind::Schema(cfg_schema) => cfg_schema,
    //         ConfigKind::Def(_) => unreachable!(),
    //     }
    // }

    /// Assumes the member symbol given is a field
    pub(super) fn get_field(&self, member_id: MemberId) -> &FieldRepre {
        match &self.members[member_id] {
            MemberSymbolKind::Field(field_repre) => field_repre,
            MemberSymbolKind::Variant(_)
            | MemberSymbolKind::OptAssignmentRoot(_)
            | MemberSymbolKind::ConfigDefMember(_)
            | MemberSymbolKind::Unknown { .. }
            | MemberSymbolKind::OptAssignmentMember(_) => unreachable!(),
        }
    }

    /// Assumes the member symbol given is a field
    pub(super) fn get_field_mut(&mut self, member_id: MemberId) -> &mut FieldRepre {
        match &mut self.members[member_id] {
            MemberSymbolKind::Field(field_repre) => field_repre,
            _ => unreachable!(),
        }
    }

    /// Assumes the member symbol given is a variant
    pub(super) fn get_variant(&self, member_id: MemberId) -> &VariantRepre {
        match &self.members[member_id] {
            MemberSymbolKind::Variant(variant_repre) => variant_repre,
            _ => unreachable!(),
        }
    }

    /// Assumes the member symbol given is a variant
    pub(super) fn get_variant_mut(&mut self, member_id: MemberId) -> &mut VariantRepre {
        match &mut self.members[member_id] {
            MemberSymbolKind::Variant(variant_repre) => variant_repre,
            _ => unreachable!(),
        }
    }

    /// Assumes the member symbol given is a field
    pub fn get_cfg_def_member(&self, member_id: MemberId) -> &ConfigDefMember {
        match &self.members[member_id] {
            MemberSymbolKind::ConfigDefMember(cfg_def_member) => cfg_def_member,
            _ => unreachable!(),
        }
    }

    /// Assumes the member symbol given is a field
    pub(super) fn get_cfg_def_member_mut(&mut self, member_id: MemberId) -> &mut ConfigDefMember {
        match &mut self.members[member_id] {
            MemberSymbolKind::ConfigDefMember(cfg_def_member) => cfg_def_member,
            _ => unreachable!(),
        }
    }

    // /// Assumes the member symbol given is a parameter
    // pub(super) fn get_param(&self, member_id: MemberId) -> &Param {
    //     match &self.members[member_id ] {
    //         MemberSymbolKind::Param(param) => &param,
    //         _ => unreachable!(),
    //     }
    // }
    //
    // /// Assumes the member symbol given is a parameter
    // pub(super) fn get_param_mut(&mut self, member_id: MemberId) -> &mut Param {
    //     match &mut self.members[member_id ] {
    //         MemberSymbolKind::Param(param) => param,
    //         _ => unreachable!(),
    //     }
    // }

    /// Assumes the member symbol given is a field
    pub(super) fn get_opt_assignment_root(&self, member_id: MemberId) -> &OptionAssignmentRoot {
        match &self.members[member_id] {
            MemberSymbolKind::OptAssignmentRoot(opt_root) => opt_root,
            _ => unreachable!(),
        }
    }

    /// Assumes the member symbol given is a field
    pub(super) fn get_opt_assignment_root_mut(
        &mut self,
        member_id: MemberId,
    ) -> &mut OptionAssignmentRoot {
        match &mut self.members[member_id] {
            MemberSymbolKind::OptAssignmentRoot(opt_root) => opt_root,
            _ => unreachable!(),
        }
    }

    /// Assumes the member symbol given is a field
    pub(super) fn get_opt_assignment_member(&self, member_id: MemberId) -> &OptionAssignmentMember {
        match &self.members[member_id] {
            MemberSymbolKind::OptAssignmentMember(opt_member) => opt_member,
            _ => unreachable!(),
        }
    }

    /// Assumes the member symbol given is a field
    pub(super) fn get_opt_assignment_member_mut(
        &mut self,
        member_id: MemberId,
    ) -> &mut OptionAssignmentMember {
        match &mut self.members[member_id] {
            MemberSymbolKind::OptAssignmentMember(opt_member) => opt_member,
            _ => unreachable!(),
        }
    }

    // Maybe return option?
    /// Assumes the symbol given has a `TypeId` attached. Will return a `TypeId` of `Unknown` if
    /// the `SymbolKind` is unknown.
    pub(super) fn extract_type_id(&self, sym_id: SymbolId) -> TypeId {
        match &self.symbols[sym_id] {
            sym_info => match &sym_info.kind {
                SymbolKind::Type(type_id) => *type_id,
                SymbolKind::Variable(var_id) => match self.variables[*var_id].state {
                    VariableState::ReservedTypeSlot(type_id) => type_id,
                    VariableState::Known(val_id) => self.values[val_id].type_id,
                },
                SymbolKind::Namespace | SymbolKind::Config(_) | SymbolKind::Directive(_) => {
                    unreachable!()
                }
            },
        }
    }

    // Maybe return option?
    /// Attempts to get a `TypeId` out of the given symbol if possible
    pub(super) fn get_type_id_from_sym_id(&self, sym_id: SymbolId) -> Option<TypeId> {
        match &self.symbols[sym_id] {
            sym_info => match &sym_info.kind {
                SymbolKind::Type(type_id) => Some(*type_id),
                SymbolKind::Variable(var_id) => match self.variables[*var_id].state {
                    VariableState::ReservedTypeSlot(type_id) => Some(type_id),
                    VariableState::Known(val_id) => Some(self.values[val_id].type_id),
                },
                // Not a type, just a symbol with a scope
                SymbolKind::Directive(_) | SymbolKind::Namespace | SymbolKind::Config(_) => None,
            },
        }
    }

    /// Attempts to get a `SymbolId` out of a `TypeId`
    pub(super) fn get_sym_id_from_type_id(&self, mut type_id: TypeId) -> Option<SymbolId> {
        for _ in 0..chrn_utils::MAX_LOOPS {
            match &self.types[type_id].ty {
                Type::Struct(struct_def) => return Some(struct_def.sym_id),
                Type::Enum(enum_def) => return Some(enum_def.sym_id),
                Type::Func(func_def) => return Some(func_def.sym_id),
                Type::Alias(alias_def) => return Some(alias_def.sym_id),
                Type::TypeDef(type_def) => return Some(type_def.sym_id),
                Type::Deferred(inner) => {
                    type_id = *inner;
                    continue;
                }
                Type::BuiltinTypeInfo(info) => return Some(info.sym_id),
                Type::Boundaries(_) | Type::Unknown => {
                    return None;
                }
            }
        }
        loop_abort!()
    }

    /// Attempts to get a `TypeId` out of the given `MemberId` if possible
    pub(super) fn get_type_id_from_member_id(&self, member_id: MemberId) -> Option<TypeId> {
        match &self.members[member_id] {
            MemberSymbolKind::Field(field_repre) => Some(field_repre.type_id),
            MemberSymbolKind::Variant(variant_repre) => variant_repre.type_id,
            MemberSymbolKind::ConfigDefMember(_)
            | MemberSymbolKind::OptAssignmentRoot(_)
            | MemberSymbolKind::Unknown { .. }
            | MemberSymbolKind::OptAssignmentMember(_) => None,
        }
    }

    pub(super) fn get_span_from_sym_id(&self, sym_id: SymbolId) -> Option<SourceSpan> {
        match &self.symbols[sym_id].kind {
            SymbolKind::Type(type_id) => self.get_span_from_type_id(*type_id),
            SymbolKind::Variable(var_id) => Some(self.variables[*var_id].name_span),
            SymbolKind::Config(cfg_id) => Some(self.cfgs[*cfg_id].name_span),
            SymbolKind::Namespace | SymbolKind::Directive(_) => None,
        }
    }

    pub(super) fn get_span_from_member_id(&self, member_id: MemberId) -> Option<SourceSpan> {
        match &self.members[member_id] {
            MemberSymbolKind::Field(field_repre) => Some(field_repre.name_span),
            MemberSymbolKind::Variant(variant_repre) => Some(variant_repre.name_span),
            MemberSymbolKind::OptAssignmentRoot(cfg_opt) => Some(cfg_opt.name_span),
            MemberSymbolKind::ConfigDefMember(cfg_def_member) => Some(cfg_def_member.name_span),
            MemberSymbolKind::OptAssignmentMember(opt_assignment_member) => {
                Some(opt_assignment_member.name_span)
            }
            MemberSymbolKind::Unknown { .. } => None,
        }
    }

    //TODO: Not
    pub(super) fn get_span_from_type_id(&self, mut type_id: TypeId) -> Option<SourceSpan> {
        // TODO: Ok
        for _ in 0..chrn_utils::MAX_LOOPS {
            match &self.types[type_id].ty {
                Type::BuiltinTypeInfo(builtin_type) => return None,
                Type::Struct(struct_def) => return Some(struct_def.name_span),
                Type::Enum(enum_def) => return Some(enum_def.name_span),
                // Functions can't be declared
                Type::Alias(alias_def) => return Some(alias_def.name_span),
                Type::TypeDef(type_def) => return Some(type_def.name_span),
                Type::Deferred(inner) => type_id = *inner,
                // Type spanning needs to be reasoned about first
                Type::Boundaries(boundary_flags) => todo!(),
                Type::Unknown => todo!("Should still be spanned though"),
                Type::Func(_) => return None,
            }
        }
        loop_abort!()
    }

    /// If the given `TypeId` is a `BuiltinType` returns `true`, `false` otherwise
    pub(super) fn check_builtin(&self, mut type_id: TypeId) -> bool {
        for _ in 0..chrn_utils::MAX_LOOPS {
            match &self.types[type_id].ty {
                Type::BuiltinTypeInfo(_) => return true,
                Type::Struct(_)
                | Type::Enum(_)
                | Type::Func(_)
                | Type::Alias(_)
                | Type::TypeDef(_)
                | Type::Boundaries(_)
                | Type::Unknown => return false,
                // Can builtins be deferred to?
                Type::Deferred(inner) => type_id = *inner,
            }
        }
        loop_abort!()
    }

    // TODO: Fix type metadata
    /// Returns `None` if type is `Unknown`, otherwise returns `Some`
    pub(super) fn get_name_id_from_type_id(&self, mut type_id: TypeId) -> Option<InternedId> {
        for _ in 0..chrn_utils::MAX_LOOPS {
            match &self.types[type_id].ty {
                Type::BuiltinTypeInfo(builtin_type) => {
                    return Some(builtin_type.ty.kind().name_id());
                }
                Type::Struct(struct_def) => return Some(self.symbols[struct_def.sym_id].name_id),
                Type::Enum(enum_def) => return Some(self.symbols[enum_def.sym_id].name_id),
                // Functions can't be declared
                Type::Alias(alias_def) => return Some(self.symbols[alias_def.sym_id].name_id),
                // WARN: Inconsistency
                Type::TypeDef(type_def) => return Some(type_def.name_id),
                Type::Func(func) => return Some(func.name_id),
                Type::Deferred(inner) => type_id = *inner,
                // Should the return type be String then?
                // This absolutely can't return a type id
                Type::Boundaries(boundary_flags) => return None,
                // Not classifying unknown as a known identifier since it may lead to mis-usage of
                // the identifier as though it really is the identifier of an actual declared type,
                // rather than rephrasing for the fact that the type itself is unknown.
                //
                // Phrases like "The type `Unknown`" sound wrong because it's not a type it's a state
                Type::Unknown => return None,
            }
        }
        loop_abort!()
    }

    pub(super) fn get_owner(&self, sym_id: SymbolId) -> ModuleId {
        match self.symbols[sym_id].sym_origin {
            SymbolOrigin::Module(mod_id) => mod_id,
            //FIX: This isn't exactly true
            SymbolOrigin::Compiler => self.intrinsic_registry.core_mod_id,
        }
    }

    /// Get's the `ScopeId` with no assumption of it existing.
    ///
    /// This method exists along with extract_scope_id due to cross module namespace checking not
    /// innately confirming whether or not it contains a particular `ScopeType`
    pub fn get_scope_id(&self, scope_type: ScopeType, owner: ModuleId) -> Option<ScopeId> {
        scopes::find_scope(self, scope_type, owner).map(|s| s.scope.scope_id)
    }

    /// Get's the `ScopeId` assuming that the scope already exists. Panics otherwise.
    ///
    /// This exists because if the current module has something like a typedef in the semantic stage,
    /// that means the parser itself already checked if it was legal grammar-wise.
    pub fn extract_scope_id(&self, scope_type: ScopeType, owner_id: ModuleId) -> ScopeId {
        scopes::find_scope(self, scope_type, owner_id)
            .expect("Either misuage of function, semantic broke, parser broke, or modules broke")
            .scope
            .scope_id
    }

    /// Get's scope using a `ScopeId`
    pub fn get_scope(&self, scope_id: ScopeId) -> &ScopeInfo {
        &self.scopes[scope_id]
    }

    /// Returns mutably borrowed `ScopeInfo` using a `ScopeId`
    pub fn get_scope_mut(&mut self, scope_id: ScopeId) -> &mut ScopeInfo {
        &mut self.scopes[scope_id]
    }

    /// Pushes new scope with given scope type and returns the `ScopeId`. If the scope already
    /// exists then it returns the existent `ScopeId`.
    pub fn push_scope(&mut self, scope_type: ScopeType, owner_id: ModuleId) -> ScopeId {
        if let Some(scope_info) = scopes::find_scope(self, scope_type, owner_id) {
            return scope_info.scope.scope_id;
        }

        let scope_id = ScopeId::new(self.scopes.len() as u16);
        // Beep
        let intrinsic_scope_opt: Option<ScopeId> = match scope_type {
            // Lazy
            ScopeType::Override => {
                if let Some(scope_id) = self.intrinsic_registry.override_scope_id {
                    Some(scope_id)
                } else {
                    let scope_id = Some(self.load_override_symbols());
                    self.intrinsic_registry.override_scope_id = scope_id;
                    scope_id
                }
            }
            ScopeType::Local
            | ScopeType::Neutral
            | ScopeType::Var
            | ScopeType::Nest
            | ScopeType::Complex
            | ScopeType::Compiler
            | ScopeType::Core => None,
        };

        let scope = Scope::new(scope_id, scope_type, false, intrinsic_scope_opt);
        let scope_info = ScopeInfo::new(scope, None, owner_id);
        self.scopes.push(scope_info);

        let owner_mod = &mut self.mods[owner_id];
        owner_mod.scopes.push(scope_id);
        // owner_mod.held_scopes |= scope_type.to_u8();

        scope_id
    }

    /// Loads the core module
    fn load_core(compiler: &mut ScriptCompiler) {
        let mut table = Table::new();

        //TODO: If namespace core exists as a module then should error earlier
        let core_name_id = InternedId::new(intern::INTERNED_CORE);
        let core_mod_id = ModuleId::new(compiler.mods.len() as u32);
        let core_scope_id = ScopeId::new(compiler.scopes.len() as u16);

        // Uses module only so that there are no possible borrow checker issues.
        let mut core_mod = Module::new(
            core_name_id,
            ModuleState::Loaded,
            core_mod_id,
            None,
            Vec::new(),
            None,
        );

        Self::load_core_types(compiler, &core_mod, &mut table);
        Self::load_core_funcs(compiler, &core_mod, &mut table);
        // Self::load_complex_constants(compiler, &mut core_mod, &mut table);
        // Self::load_override_constants(compiler, &mut core_mod, &mut table);

        // Exporting all created symbols from core
        for sym_id in table.interned_to_sym.values().copied() {
            core_mod.exports.push(sym_id);
        }

        // Done adding all of core
        let scope_id = ScopeId::new(compiler.scopes.len() as u16);
        let scope = Scope::with_table(scope_id, ScopeType::Core, None, true, table);
        let scope_info = ScopeInfo::new(scope, None, core_mod_id);

        compiler.scopes.push(scope_info);
        core_mod.scopes.push(scope_id);

        compiler.mods.push(core_mod);

        let core_import = Import::new(core_name_id, ImportKind::Core(core_mod_id), None);

        // Injecting core as an import and pushing it's scope so user modules can search it
        for user_mod in &mut compiler.mods.items {
            user_mod.imports.push(core_import.clone());
            user_mod.scopes.push(core_scope_id);
        }
    }

    /// Returns `true` if the type is unknown, false otherwise
    pub fn check_unknown(&self, mut type_id: TypeId) -> bool {
        // This limit is semi-random
        for _ in 0..chrn_utils::MAX_LOOPS {
            let ty = &self.types[type_id].ty;
            match ty {
                Type::Deferred(inner) => type_id = *inner,
                Type::Unknown => return true,
                _ => return false,
            }
        }
        loop_abort!()
    }

    /// Loads all compiler known directives
    fn load_directives(compiler: &mut ScriptCompiler) {
        // #warn | 0
        let sym_id = SymbolId::new(compiler.symbols.len() as u32);
        let directive_id = DirectiveId::new(compiler.directives.len() as u32);
        let interned_id = InternedId::new(intern::INTERNED_WARN);
        debug_assert_eq!(directive_id.id, 0, "There should not be any directives");

        let sym = Symbol::new(
            interned_id,
            sym_id,
            None,
            SymbolOrigin::Compiler,
            false,
            None,
            ScopeType::Compiler,
            SymbolKind::Directive(directive_id),
        );

        let directive = Directive::Warn;

        compiler.symbols.push(sym);
        compiler.directives.push(directive);

        // #ignore | 1
        let sym_id = SymbolId::new(compiler.symbols.len() as u32);
        let directive_id = DirectiveId::new(compiler.directives.len() as u32);
        let interned_id = InternedId::new(intern::INTERNED_IGNORE);

        let sym = Symbol::new(
            interned_id,
            sym_id,
            None,
            SymbolOrigin::Compiler,
            false,
            None,
            ScopeType::Compiler,
            SymbolKind::Directive(directive_id),
        );

        let directive = Directive::Ignore;

        compiler.symbols.push(sym);
        compiler.directives.push(directive);

        // #scient | 2
        let sym_id = SymbolId::new(compiler.symbols.len() as u32);
        let directive_id = DirectiveId::new(compiler.directives.len() as u32);
        let interned_id = InternedId::new(intern::INTERNED_SCIENT);

        let sym = Symbol::new(
            interned_id,
            sym_id,
            None,
            SymbolOrigin::Compiler,
            false,
            None,
            ScopeType::Compiler,
            SymbolKind::Directive(directive_id),
        );

        let directive = Directive::Type(TypeDirective::Scient);

        compiler.symbols.push(sym);
        compiler.directives.push(directive);

        // #hex | 3
        let sym_id = SymbolId::new(compiler.symbols.len() as u32);
        let directive_id = DirectiveId::new(compiler.directives.len() as u32);
        let interned_id = InternedId::new(intern::INTERNED_HEX);

        let sym = Symbol::new(
            interned_id,
            sym_id,
            None,
            SymbolOrigin::Compiler,
            false,
            None,
            ScopeType::Compiler,
            SymbolKind::Directive(directive_id),
        );

        let directive = Directive::Type(TypeDirective::Hex);

        compiler.symbols.push(sym);
        compiler.directives.push(directive);

        // #bin | 4
        let sym_id = SymbolId::new(compiler.symbols.len() as u32);
        let directive_id = DirectiveId::new(compiler.directives.len() as u32);
        let interned_id = InternedId::new(intern::INTERNED_BIN);

        let sym = Symbol::new(
            interned_id,
            sym_id,
            None,
            SymbolOrigin::Compiler,
            false,
            None,
            ScopeType::Compiler,
            SymbolKind::Directive(directive_id),
        );

        let directive = Directive::Type(TypeDirective::Bin);

        compiler.symbols.push(sym);
        compiler.directives.push(directive);

        // #octal | 5
        let sym_id = SymbolId::new(compiler.symbols.len() as u32);
        let directive_id = DirectiveId::new(compiler.directives.len() as u32);
        let interned_id = InternedId::new(intern::INTERNED_OCTAL);

        let sym = Symbol::new(
            interned_id,
            sym_id,
            None,
            SymbolOrigin::Compiler,
            false,
            None,
            ScopeType::Compiler,
            SymbolKind::Directive(directive_id),
        );

        let directive = Directive::Type(TypeDirective::Octal);

        compiler.symbols.push(sym);
        compiler.directives.push(directive);

        // #unicode | 6
        let sym_id = SymbolId::new(compiler.symbols.len() as u32);
        let directive_id = DirectiveId::new(compiler.directives.len() as u32);
        let interned_id = InternedId::new(intern::INTERNED_UNICODE);

        let sym = Symbol::new(
            interned_id,
            sym_id,
            None,
            SymbolOrigin::Compiler,
            false,
            None,
            ScopeType::Compiler,
            SymbolKind::Directive(directive_id),
        );

        let directive = Directive::Type(TypeDirective::Unicode);

        compiler.symbols.push(sym);
        compiler.directives.push(directive);
    }

    //TODO: There is an issue with how scopes are consumed right now which makes giving specific
    //scopes known constants difficult. Since there is no one source of data for a section to get
    //it's constants, it isn't possible to make it so if we are in a `complex->` section, it shows
    //language specific constants like RUST or JAVA which all for specifying behavior. All
    //scopes are locally owned and don't separate what declared can be used for all other scopes,
    //and which are just local.
    //
    // There should be more percise access level rules to where a variable declaration will allow
    // all other scopes to use it, but also have where it's declaration occurred be tied to it,
    // while also allowing for a section like `complex` to show the `RUST` constant only in it's
    // own scope.
    //
    // This would probably require pre-loading section symbols on-demand to where their
    // associated_scope is immediately attached to all the resolver stages. So maybe a
    // ScopeType::Global is needed.
    //
    // First lets focus on how pre-loading would work
    //
    // Ok what about, if not found in normal scope, search intrinsic, where now scopes carry
    // Option<ScopeId's> which allow for their intrinsics to be searched
    // /// Creates scope with the constants needed for a `complex` section to function then returns
    // /// it's `ScopeId`
    // fn load_complex_constants(&mut self) -> ScopeId {
    //     // IS it from core? The semantics are getting a little lost
    //     let core_mod_id = self.intrinsic_registry.core_mod_id;
    //     let scope_type = ScopeType::Complex;
    //
    //     let mut table = Table::new();
    //
    //     let default_val_name_id = InternedId::new(intern::INTERNED_DEFAULT_VALUE);
    //
    //     let opt_schema = OptionSchema::new(default_val_name_id, None);
    //     let field_opt_schemas = vec![opt_schema];
    //
    //     // table.interned_to_sym.insert(default_val_name_id, sym_id);
    //
    //     let cfg_schema = ConfigSchema::new(ConfigSchemaKind::Field, field_opt_schemas);
    //
    //     // let cfg_id = ConfigId::new(self.configs.len() as u32);
    //     // let sym = Symbol::new(
    //     //     default_val_name_id,
    //     //     sym_id,
    //     //     None,
    //     //     core_mod_id,
    //     //     false,
    //     //     None,
    //     //     scope_type,
    //     //     SymbolKind::Config(cfg_id),
    //     // );
    //
    //     self.configs.push(cfg_schema);
    //
    //     let scope_id = ScopeId::new(self.scopes.len());
    //     let scope = Scope::with_table(scope_id, scope_type, None, true, table);
    //     let scope_info = ScopeInfo::new(scope, None, core_mod_id);
    //     self.scopes.push(scope_info);
    //
    //     scope_id
    //     // Need to load configuration structures with known fields
    //     //
    //     // The conceptual idea is, ConfigDef holds config options, which are known, and may have
    //     // different options depending on the type.
    //     //
    //     // For searching against configuration that's known, we could have, SchemaKind, where it's
    //     // kind dictates what options should be accounted for. So, given a target identifier,
    //     // value, and kind of schema, what did we find.
    // }

    // const fn configs(kind: ConfigSchemaKind) -> &'static ConfigSchema {
    //     match kind {
    //         ConfigSchemaKind::Struct => lang::schemas::,
    //         ConfigSchemaKind::Enum => todo!(),
    //         ConfigSchemaKind::Field => todo!(),
    //     }
    //
    // }

    /// Creates scope with the constants needed for an `override` section to function then returns
    /// it's `ScopeId`
    fn load_override_symbols(&mut self) -> ScopeId {
        // IS it from core? The semantics are getting a little lost
        //
        // Saying it's not from core for now because !
        // let core_mod_id = self.intrinsic_registry.core_mod_id;
        let scope_type = ScopeType::Override;
        let override_scope_id = ScopeId::new(self.scopes.len() as u16);

        // Override intrisic scope's table which holes stuff like "RUST" and "JAVA" namespaces
        let mut table = Table::new();
        self.load_override_java_cfg(&mut table, override_scope_id);
        let scope = Scope::with_table(override_scope_id, scope_type, None, true, table);

        todo!("We want to return you");

        override_scope_id
    }

    fn load_override_java_cfg(&mut self, table: &mut Table, override_scope_id: ScopeId) {
        let name_id = InternedId::new(intern::INTERNED_JAVA_UPPER);
        let scope_type = ScopeType::Override;

        let sym_id = SymbolId::new(self.symbols.len() as u32);
        let cfg_root_id = ConfigRootId::new(self.cfgs.len() as u32);

        let java_symbol = Symbol::new(
            name_id,
            sym_id,
            None,
            SymbolOrigin::Compiler,
            false,
            None,
            scope_type,
            SymbolKind::Config(cfg_root_id),
        );

        table.interned_to_sym.insert(name_id, sym_id);
        self.symbols.push(java_symbol);
        // self.cfgs.push(val);

        todo!()
    }

    /// Helper to load all of core's functions and predicates
    fn load_core_funcs(compiler: &mut ScriptCompiler, core_mod: &Module, table: &mut Table) {
        let core_mod_id = core_mod.mod_id;

        // IsEmpty
        let type_id = TypeId::new(compiler.types.len() as u32);
        let is_empty_flags = TypeBoundaryFlags::COLLECTION;
        let interned_id = InternedId::new(intern::INTERNED_IS_EMPTY);

        let sym_id = SymbolId::new(compiler.symbols.len() as u32);
        let func_def = FuncDef::new(
            sym_id,
            interned_id,
            FuncKind::IsEmpty,
            false,
            is_empty_flags,
            vec![ArgConstraint::ArgCount(0)],
            true,
            TypeId::new(CORE_BOOL),
        );

        compiler
            .types
            .push(TypeInfo::new(Type::Func(func_def), core_mod_id));

        let symbol = Symbol::new(
            interned_id,
            sym_id,
            None,
            SymbolOrigin::Module(core_mod_id),
            false,
            None,
            ScopeType::Core,
            SymbolKind::Type(type_id),
        );

        compiler.symbols.push(symbol);
        table.interned_to_sym.insert(interned_id, sym_id);

        // IsWhitespace | CharacterMappable
        let type_id = TypeId::new(compiler.types.len() as u32);
        let ws_flags = TypeBoundaryFlags::CHARACTER_MAPPABLE;
        let interned_id = InternedId::new(intern::INTERNED_IS_WHITESPACE);

        let sym_id = SymbolId::new(compiler.symbols.len() as u32);
        let func_def = FuncDef::new(
            sym_id,
            interned_id,
            FuncKind::IsWhitespace,
            false,
            ws_flags,
            vec![ArgConstraint::ArgCount(0), ArgConstraint::CharacterMappable],
            true,
            TypeId::new(CORE_BOOL),
        );

        compiler
            .types
            .push(TypeInfo::new(Type::Func(func_def), core_mod_id));

        let symbol = Symbol::new(
            interned_id,
            sym_id,
            None,
            SymbolOrigin::Module(core_mod_id),
            false,
            None,
            ScopeType::Core,
            SymbolKind::Type(type_id),
        );

        compiler.symbols.push(symbol);
        table.interned_to_sym.insert(interned_id, sym_id);

        // Contains(String | char) CharacterMappable
        let type_id = TypeId::new(compiler.types.len() as u32);
        let contains_flags = TypeBoundaryFlags::CHARACTER_MAPPABLE;
        let interned_id = InternedId::new(intern::INTERNED_CONTAINS);

        let sym_id = SymbolId::new(compiler.symbols.len() as u32);
        let func_def = FuncDef::new(
            sym_id,
            interned_id,
            FuncKind::Contains,
            true,
            contains_flags,
            vec![ArgConstraint::ArgCount(1), ArgConstraint::CharacterMappable],
            true,
            TypeId::new(CORE_BOOL),
        );

        compiler
            .types
            .push(TypeInfo::new(Type::Func(func_def), core_mod_id));

        let symbol = Symbol::new(
            interned_id,
            sym_id,
            None,
            SymbolOrigin::Module(core_mod_id),
            false,
            None,
            ScopeType::Core,
            SymbolKind::Type(type_id),
        );

        compiler.symbols.push(symbol);
        table.interned_to_sym.insert(interned_id, sym_id);

        // StartsW(Value) | CharacterMappable
        let type_id = TypeId::new(compiler.types.len() as u32);
        let startsw_flags = TypeBoundaryFlags::CHARACTER_MAPPABLE;
        let interned_id = InternedId::new(intern::INTERNED_STARTSW);

        let sym_id = SymbolId::new(compiler.symbols.len() as u32);
        let func_def = FuncDef::new(
            sym_id,
            interned_id,
            FuncKind::StartsW,
            true,
            startsw_flags,
            vec![ArgConstraint::ArgCount(1), ArgConstraint::CharacterMappable],
            true,
            TypeId::new(CORE_BOOL),
        );

        compiler
            .types
            .push(TypeInfo::new(Type::Func(func_def), core_mod_id));

        let symbol = Symbol::new(
            interned_id,
            sym_id,
            None,
            SymbolOrigin::Module(core_mod_id),
            false,
            None,
            ScopeType::Core,
            SymbolKind::Type(type_id),
        );

        compiler.symbols.push(symbol);
        table.interned_to_sym.insert(interned_id, sym_id);

        // EndsW(Value) | CharacterMappable
        let sym_id = SymbolId::new(compiler.symbols.len() as u32);
        let type_id = TypeId::new(compiler.types.len() as u32);
        let endsw_flags = TypeBoundaryFlags::CHARACTER_MAPPABLE;
        let interned_id = InternedId::new(intern::INTERNED_ENDSW);

        let func_def = FuncDef::new(
            sym_id,
            interned_id,
            FuncKind::EndsW,
            true,
            // What about CharacterMappable? Do we really want to be judgemental here?
            // There we go
            endsw_flags,
            vec![ArgConstraint::ArgCount(1), ArgConstraint::CharacterMappable],
            true,
            TypeId::new(CORE_BOOL),
        );

        compiler
            .types
            .push(TypeInfo::new(Type::Func(func_def), core_mod_id));

        let symbol = Symbol::new(
            interned_id,
            sym_id,
            None,
            SymbolOrigin::Module(core_mod_id),
            false,
            None,
            ScopeType::Core,
            SymbolKind::Type(type_id),
        );

        compiler.symbols.push(symbol);
        table.interned_to_sym.insert(interned_id, sym_id);

        // Range(inclusive, exclusive) | Numeric | Ordering
        let type_id = TypeId::new(compiler.types.len() as u32);
        let range_flags = TypeBoundaryFlags::RANGED;
        let sym_id = SymbolId::new(compiler.symbols.len() as u32);
        let interned_id = InternedId::new(intern::INTERNED_RANGE);

        let func_def = FuncDef::new(
            sym_id,
            interned_id,
            FuncKind::Range,
            true,
            range_flags,
            vec![
                ArgConstraint::ArgCount(2),
                ArgConstraint::Numeric,
                ArgConstraint::MatchingArgumentTypes,
                ArgConstraint::SameTypeAsSelf,
            ],
            true,
            TypeId::new(CORE_BOOL),
        );

        compiler
            .types
            .push(TypeInfo::new(Type::Func(func_def), core_mod_id));

        let symbol = Symbol::new(
            interned_id,
            sym_id,
            None,
            SymbolOrigin::Module(core_mod_id),
            false,
            None,
            ScopeType::Core,
            SymbolKind::Type(type_id),
        );

        compiler.symbols.push(symbol);
        table.interned_to_sym.insert(interned_id, sym_id);

        // Equals(Comparable)
        let type_id = TypeId::new(compiler.types.len() as u32);
        let eq_flags = TypeBoundaryFlags::COMPARABLE;
        let interned_id = InternedId::new(intern::INTERNED_EQUALS);

        let sym_id = SymbolId::new(compiler.symbols.len() as u32);
        let func_def = FuncDef::new(
            sym_id,
            interned_id,
            FuncKind::Equals,
            true,
            eq_flags,
            vec![
                ArgConstraint::ArgCount(1),
                ArgConstraint::Comparable,
                ArgConstraint::SameTypeAsSelf,
            ],
            true,
            TypeId::new(CORE_BOOL),
        );

        compiler
            .types
            .push(TypeInfo::new(Type::Func(func_def), core_mod_id));

        let symbol = Symbol::new(
            interned_id,
            sym_id,
            None,
            SymbolOrigin::Module(core_mod_id),
            false,
            None,
            ScopeType::Core,
            SymbolKind::Type(type_id),
        );

        compiler.symbols.push(symbol);
        table.interned_to_sym.insert(interned_id, sym_id);
    }

    // --- Beep
    /// Helper to load all of core's types
    fn load_core_types(compiler: &mut ScriptCompiler, core_mod: &Module, table: &mut Table) {
        let core_mod_id = core_mod.mod_id;

        // -- Concrete types --

        let type_id = TypeId::new(compiler.types.len() as u32);
        let sym_id = SymbolId::new(compiler.symbols.len() as u32);
        let interned_id = InternedId::new(intern::INTERNED_I8);
        compiler.types.push(TypeInfo::new(
            Type::BuiltinTypeInfo(BuiltinTypeInfo::new(sym_id, BuiltinType::I8)),
            core_mod_id,
        ));
        let symbol = Symbol::new(
            interned_id,
            sym_id,
            None,
            SymbolOrigin::Module(core_mod_id),
            false,
            None,
            ScopeType::Core,
            SymbolKind::Type(type_id),
        );

        compiler.symbols.push(symbol);
        table.interned_to_sym.insert(interned_id, sym_id);

        let type_id = TypeId::new(compiler.types.len() as u32);
        let sym_id = SymbolId::new(compiler.symbols.len() as u32);
        let interned_id = InternedId::new(intern::INTERNED_U8);
        compiler.types.push(TypeInfo::new(
            Type::BuiltinTypeInfo(BuiltinTypeInfo::new(sym_id, BuiltinType::U8)),
            core_mod_id,
        ));
        let symbol = Symbol::new(
            interned_id,
            sym_id,
            None,
            SymbolOrigin::Module(core_mod_id),
            false,
            None,
            ScopeType::Core,
            SymbolKind::Type(type_id),
        );

        compiler.symbols.push(symbol);
        table.interned_to_sym.insert(interned_id, sym_id);

        let type_id = TypeId::new(compiler.types.len() as u32);
        let sym_id = SymbolId::new(compiler.symbols.len() as u32);
        let interned_id = InternedId::new(intern::INTERNED_I16);
        compiler.types.push(TypeInfo::new(
            Type::BuiltinTypeInfo(BuiltinTypeInfo::new(sym_id, BuiltinType::I16)),
            core_mod_id,
        ));
        let symbol = Symbol::new(
            interned_id,
            sym_id,
            None,
            SymbolOrigin::Module(core_mod_id),
            false,
            None,
            ScopeType::Core,
            SymbolKind::Type(type_id),
        );
        compiler.symbols.push(symbol);
        table.interned_to_sym.insert(interned_id, sym_id);

        let type_id = TypeId::new(compiler.types.len() as u32);
        let sym_id = SymbolId::new(compiler.symbols.len() as u32);
        let interned_id = InternedId::new(intern::INTERNED_U16);
        compiler.types.push(TypeInfo::new(
            Type::BuiltinTypeInfo(BuiltinTypeInfo::new(sym_id, BuiltinType::U16)),
            core_mod_id,
        ));
        let symbol = Symbol::new(
            interned_id,
            sym_id,
            None,
            SymbolOrigin::Module(core_mod_id),
            false,
            None,
            ScopeType::Core,
            SymbolKind::Type(type_id),
        );
        compiler.symbols.push(symbol);
        table.interned_to_sym.insert(interned_id, sym_id);

        let type_id = TypeId::new(compiler.types.len() as u32);
        let sym_id = SymbolId::new(compiler.symbols.len() as u32);
        let interned_id = InternedId::new(intern::INTERNED_F16);
        compiler.types.push(TypeInfo::new(
            Type::BuiltinTypeInfo(BuiltinTypeInfo::new(sym_id, BuiltinType::F16)),
            core_mod_id,
        ));
        let symbol = Symbol::new(
            interned_id,
            sym_id,
            None,
            SymbolOrigin::Module(core_mod_id),
            false,
            None,
            ScopeType::Core,
            SymbolKind::Type(type_id),
        );
        compiler.symbols.push(symbol);
        table.interned_to_sym.insert(interned_id, sym_id);

        let type_id = TypeId::new(compiler.types.len() as u32);
        let sym_id = SymbolId::new(compiler.symbols.len() as u32);
        let interned_id = InternedId::new(intern::INTERNED_I32);
        compiler.types.push(TypeInfo::new(
            Type::BuiltinTypeInfo(BuiltinTypeInfo::new(sym_id, BuiltinType::I32)),
            core_mod_id,
        ));
        let symbol = Symbol::new(
            interned_id,
            sym_id,
            None,
            SymbolOrigin::Module(core_mod_id),
            false,
            None,
            ScopeType::Core,
            SymbolKind::Type(type_id),
        );
        compiler.symbols.push(symbol);
        table.interned_to_sym.insert(interned_id, sym_id);

        let type_id = TypeId::new(compiler.types.len() as u32);
        let sym_id = SymbolId::new(compiler.symbols.len() as u32);
        let interned_id = InternedId::new(intern::INTERNED_U32);
        compiler.types.push(TypeInfo::new(
            Type::BuiltinTypeInfo(BuiltinTypeInfo::new(sym_id, BuiltinType::U32)),
            core_mod_id,
        ));
        let symbol = Symbol::new(
            interned_id,
            sym_id,
            None,
            SymbolOrigin::Module(core_mod_id),
            false,
            None,
            ScopeType::Core,
            SymbolKind::Type(type_id),
        );
        compiler.symbols.push(symbol);
        table.interned_to_sym.insert(interned_id, sym_id);

        let type_id = TypeId::new(compiler.types.len() as u32);
        let sym_id = SymbolId::new(compiler.symbols.len() as u32);
        let interned_id = InternedId::new(intern::INTERNED_F32);
        compiler.types.push(TypeInfo::new(
            Type::BuiltinTypeInfo(BuiltinTypeInfo::new(sym_id, BuiltinType::F32)),
            core_mod_id,
        ));
        let symbol = Symbol::new(
            interned_id,
            sym_id,
            None,
            SymbolOrigin::Module(core_mod_id),
            false,
            None,
            ScopeType::Core,
            SymbolKind::Type(type_id),
        );
        compiler.symbols.push(symbol);
        table.interned_to_sym.insert(interned_id, sym_id);

        let type_id = TypeId::new(compiler.types.len() as u32);
        let sym_id = SymbolId::new(compiler.symbols.len() as u32);
        let interned_id = InternedId::new(intern::INTERNED_I64);
        compiler.types.push(TypeInfo::new(
            Type::BuiltinTypeInfo(BuiltinTypeInfo::new(sym_id, BuiltinType::I64)),
            core_mod_id,
        ));
        let symbol = Symbol::new(
            interned_id,
            sym_id,
            None,
            SymbolOrigin::Module(core_mod_id),
            false,
            None,
            ScopeType::Core,
            SymbolKind::Type(type_id),
        );
        compiler.symbols.push(symbol);
        table.interned_to_sym.insert(interned_id, sym_id);

        let type_id = TypeId::new(compiler.types.len() as u32);
        let sym_id = SymbolId::new(compiler.symbols.len() as u32);
        let interned_id = InternedId::new(intern::INTERNED_U64);
        compiler.types.push(TypeInfo::new(
            Type::BuiltinTypeInfo(BuiltinTypeInfo::new(sym_id, BuiltinType::U64)),
            core_mod_id,
        ));
        let symbol = Symbol::new(
            interned_id,
            sym_id,
            None,
            SymbolOrigin::Module(core_mod_id),
            false,
            None,
            ScopeType::Core,
            SymbolKind::Type(type_id),
        );
        compiler.symbols.push(symbol);
        table.interned_to_sym.insert(interned_id, sym_id);

        let type_id = TypeId::new(compiler.types.len() as u32);
        let sym_id = SymbolId::new(compiler.symbols.len() as u32);
        let interned_id = InternedId::new(intern::INTERNED_F64);
        compiler.types.push(TypeInfo::new(
            Type::BuiltinTypeInfo(BuiltinTypeInfo::new(sym_id, BuiltinType::F64)),
            core_mod_id,
        ));
        let symbol = Symbol::new(
            interned_id,
            sym_id,
            None,
            SymbolOrigin::Module(core_mod_id),
            false,
            None,
            ScopeType::Core,
            SymbolKind::Type(type_id),
        );
        compiler.symbols.push(symbol);
        table.interned_to_sym.insert(interned_id, sym_id);

        let type_id = TypeId::new(compiler.types.len() as u32);
        let sym_id = SymbolId::new(compiler.symbols.len() as u32);
        let interned_id = InternedId::new(intern::INTERNED_I128);
        compiler.types.push(TypeInfo::new(
            Type::BuiltinTypeInfo(BuiltinTypeInfo::new(sym_id, BuiltinType::I128)),
            core_mod_id,
        ));
        let symbol = Symbol::new(
            interned_id,
            sym_id,
            None,
            SymbolOrigin::Module(core_mod_id),
            false,
            None,
            ScopeType::Core,
            SymbolKind::Type(type_id),
        );
        compiler.symbols.push(symbol);
        table.interned_to_sym.insert(interned_id, sym_id);

        let type_id = TypeId::new(compiler.types.len() as u32);
        let sym_id = SymbolId::new(compiler.symbols.len() as u32);
        let interned_id = InternedId::new(intern::INTERNED_U128);
        compiler.types.push(TypeInfo::new(
            Type::BuiltinTypeInfo(BuiltinTypeInfo::new(sym_id, BuiltinType::U128)),
            core_mod_id,
        ));
        let symbol = Symbol::new(
            interned_id,
            sym_id,
            None,
            SymbolOrigin::Module(core_mod_id),
            false,
            None,
            ScopeType::Core,
            SymbolKind::Type(type_id),
        );
        compiler.symbols.push(symbol);
        table.interned_to_sym.insert(interned_id, sym_id);

        let type_id = TypeId::new(compiler.types.len() as u32);
        let sym_id = SymbolId::new(compiler.symbols.len() as u32);
        let interned_id = InternedId::new(intern::INTERNED_F128);
        compiler.types.push(TypeInfo::new(
            Type::BuiltinTypeInfo(BuiltinTypeInfo::new(sym_id, BuiltinType::F128)),
            core_mod_id,
        ));
        let symbol = Symbol::new(
            interned_id,
            sym_id,
            None,
            SymbolOrigin::Module(core_mod_id),
            false,
            None,
            ScopeType::Core,
            SymbolKind::Type(type_id),
        );
        compiler.symbols.push(symbol);
        table.interned_to_sym.insert(interned_id, sym_id);

        let type_id = TypeId::new(compiler.types.len() as u32);
        let sym_id = SymbolId::new(compiler.symbols.len() as u32);
        let interned_id = InternedId::new(intern::INTERNED_SIZED);
        compiler.types.push(TypeInfo::new(
            Type::BuiltinTypeInfo(BuiltinTypeInfo::new(sym_id, BuiltinType::Sized)),
            core_mod_id,
        ));
        let symbol = Symbol::new(
            interned_id,
            sym_id,
            None,
            SymbolOrigin::Module(core_mod_id),
            false,
            None,
            ScopeType::Core,
            SymbolKind::Type(type_id),
        );
        compiler.symbols.push(symbol);
        table.interned_to_sym.insert(interned_id, sym_id);

        let type_id = TypeId::new(compiler.types.len() as u32);
        let sym_id = SymbolId::new(compiler.symbols.len() as u32);
        let interned_id = InternedId::new(intern::INTERNED_UNSIZED);
        compiler.types.push(TypeInfo::new(
            Type::BuiltinTypeInfo(BuiltinTypeInfo::new(sym_id, BuiltinType::Unsized)),
            core_mod_id,
        ));
        let symbol = Symbol::new(
            interned_id,
            sym_id,
            None,
            SymbolOrigin::Module(core_mod_id),
            false,
            None,
            ScopeType::Core,
            SymbolKind::Type(type_id),
        );
        compiler.symbols.push(symbol);
        table.interned_to_sym.insert(interned_id, sym_id);

        let type_id = TypeId::new(compiler.types.len() as u32);
        let sym_id = SymbolId::new(compiler.symbols.len() as u32);
        let interned_id = InternedId::new(intern::INTERNED_STR);
        compiler.types.push(TypeInfo::new(
            Type::BuiltinTypeInfo(BuiltinTypeInfo::new(sym_id, BuiltinType::Str)),
            core_mod_id,
        ));
        let symbol = Symbol::new(
            interned_id,
            sym_id,
            None,
            SymbolOrigin::Module(core_mod_id),
            false,
            None,
            ScopeType::Core,
            SymbolKind::Type(type_id),
        );
        compiler.symbols.push(symbol);
        table.interned_to_sym.insert(interned_id, sym_id);

        let type_id = TypeId::new(compiler.types.len() as u32);
        let sym_id = SymbolId::new(compiler.symbols.len() as u32);
        let interned_id = InternedId::new(intern::INTERNED_CHAR);
        compiler.types.push(TypeInfo::new(
            Type::BuiltinTypeInfo(BuiltinTypeInfo::new(sym_id, BuiltinType::Char)),
            core_mod_id,
        ));
        let symbol = Symbol::new(
            interned_id,
            sym_id,
            None,
            SymbolOrigin::Module(core_mod_id),
            false,
            None,
            ScopeType::Core,
            SymbolKind::Type(type_id),
        );
        compiler.symbols.push(symbol);
        table.interned_to_sym.insert(interned_id, sym_id);

        let type_id = TypeId::new(compiler.types.len() as u32);
        let sym_id = SymbolId::new(compiler.symbols.len() as u32);
        let interned_id = InternedId::new(intern::INTERNED_NIL);
        compiler.types.push(TypeInfo::new(
            Type::BuiltinTypeInfo(BuiltinTypeInfo::new(sym_id, BuiltinType::Nil)),
            core_mod_id,
        ));
        let symbol = Symbol::new(
            interned_id,
            sym_id,
            None,
            SymbolOrigin::Module(core_mod_id),
            false,
            None,
            ScopeType::Core,
            SymbolKind::Type(type_id),
        );
        compiler.symbols.push(symbol);
        table.interned_to_sym.insert(interned_id, sym_id);

        let type_id = TypeId::new(compiler.types.len() as u32);
        let sym_id = SymbolId::new(compiler.symbols.len() as u32);
        let interned_id = InternedId::new(intern::INTERNED_BOOL);
        compiler.types.push(TypeInfo::new(
            Type::BuiltinTypeInfo(BuiltinTypeInfo::new(sym_id, BuiltinType::Bool)),
            core_mod_id,
        ));
        let symbol = Symbol::new(
            interned_id,
            sym_id,
            None,
            SymbolOrigin::Module(core_mod_id),
            false,
            None,
            ScopeType::Core,
            SymbolKind::Type(type_id),
        );
        compiler.symbols.push(symbol);
        table.interned_to_sym.insert(interned_id, sym_id);

        let type_id = TypeId::new(compiler.types.len() as u32);
        let sym_id = SymbolId::new(compiler.symbols.len() as u32);
        let interned_id = InternedId::new(intern::INTERNED_BIGINT);
        compiler.types.push(TypeInfo::new(
            Type::BuiltinTypeInfo(BuiltinTypeInfo::new(sym_id, BuiltinType::BigInt)),
            core_mod_id,
        ));
        let symbol = Symbol::new(
            interned_id,
            sym_id,
            None,
            SymbolOrigin::Module(core_mod_id),
            false,
            None,
            ScopeType::Core,
            SymbolKind::Type(type_id),
        );
        compiler.symbols.push(symbol);
        table.interned_to_sym.insert(interned_id, sym_id);

        let type_id = TypeId::new(compiler.types.len() as u32);
        let sym_id = SymbolId::new(compiler.symbols.len() as u32);
        let interned_id = InternedId::new(intern::INTERNED_BIGFLOAT);
        compiler.types.push(TypeInfo::new(
            Type::BuiltinTypeInfo(BuiltinTypeInfo::new(sym_id, BuiltinType::BigFloat)),
            core_mod_id,
        ));
        let symbol = Symbol::new(
            interned_id,
            sym_id,
            None,
            SymbolOrigin::Module(core_mod_id),
            false,
            None,
            ScopeType::Core,
            SymbolKind::Type(type_id),
        );
        compiler.symbols.push(symbol);
        table.interned_to_sym.insert(interned_id, sym_id);

        let type_id = TypeId::new(compiler.types.len() as u32);
        let sym_id = SymbolId::new(compiler.symbols.len() as u32);
        let interned_id = InternedId::new(intern::INTERNED_RUNTIME);
        compiler.types.push(TypeInfo::new(
            Type::BuiltinTypeInfo(BuiltinTypeInfo::new(sym_id, BuiltinType::Runtime)),
            core_mod_id,
        ));
        let symbol = Symbol::new(
            interned_id,
            sym_id,
            None,
            SymbolOrigin::Module(core_mod_id),
            false,
            None,
            ScopeType::Core,
            SymbolKind::Type(type_id),
        );

        compiler.symbols.push(symbol);
        table.interned_to_sym.insert(interned_id, sym_id);

        compiler
            .types
            .push(TypeInfo::new(Type::Unknown, core_mod_id));

        // -- Type constraints --
        let type_id = TypeId::new(compiler.types.len() as u32);
        compiler.types.push(TypeInfo::new(
            Type::Boundaries(TypeBoundaryFlags::RANGED),
            core_mod_id,
        ));

        let sym_id = SymbolId::new(compiler.symbols.len() as u32);
        let interned_id = InternedId::new(intern::INTERNED_RANGED);
        let symbol = Symbol::new(
            interned_id,
            sym_id,
            None,
            SymbolOrigin::Module(core_mod_id),
            false,
            None,
            ScopeType::Core,
            SymbolKind::Type(type_id),
        );

        compiler.symbols.push(symbol);
        table.interned_to_sym.insert(interned_id, sym_id);

        let type_id = TypeId::new(compiler.types.len() as u32);
        compiler.types.push(TypeInfo::new(
            Type::Boundaries(TypeBoundaryFlags::CHARACTER_MAPPABLE),
            core_mod_id,
        ));

        let sym_id = SymbolId::new(compiler.symbols.len() as u32);
        let interned_id = InternedId::new(intern::INTERNED_CHARACTER_MAPPABLE);
        let symbol = Symbol::new(
            interned_id,
            sym_id,
            None,
            SymbolOrigin::Module(core_mod_id),
            false,
            None,
            ScopeType::Core,
            SymbolKind::Type(type_id),
        );

        compiler.symbols.push(symbol);
        table.interned_to_sym.insert(interned_id, sym_id);

        let type_id = TypeId::new(compiler.types.len() as u32);
        compiler.types.push(TypeInfo::new(
            Type::Boundaries(TypeBoundaryFlags::COLLECTION),
            core_mod_id,
        ));

        let sym_id = SymbolId::new(compiler.symbols.len() as u32);
        let interned_id = InternedId::new(intern::INTERNED_COLLECTION);
        let symbol = Symbol::new(
            interned_id,
            sym_id,
            None,
            SymbolOrigin::Module(core_mod_id),
            false,
            None,
            ScopeType::Core,
            SymbolKind::Type(type_id),
        );

        compiler.symbols.push(symbol);
        table.interned_to_sym.insert(interned_id, sym_id);

        let type_id = TypeId::new(compiler.types.len() as u32);
        compiler.types.push(TypeInfo::new(
            Type::Boundaries(TypeBoundaryFlags::HAS_LEN),
            core_mod_id,
        ));

        let sym_id = SymbolId::new(compiler.symbols.len() as u32);
        let interned_id = InternedId::new(intern::INTERNED_HAS_LEN);
        let symbol = Symbol::new(
            interned_id,
            sym_id,
            None,
            SymbolOrigin::Module(core_mod_id),
            false,
            None,
            ScopeType::Core,
            SymbolKind::Type(type_id),
        );

        compiler.symbols.push(symbol);
        table.interned_to_sym.insert(interned_id, sym_id);

        let type_id = TypeId::new(compiler.types.len() as u32);
        compiler.types.push(TypeInfo::new(
            Type::Boundaries(TypeBoundaryFlags::INTEGER),
            core_mod_id,
        ));

        let sym_id = SymbolId::new(compiler.symbols.len() as u32);
        let interned_id = InternedId::new(intern::INTERNED_INTEGER);
        let symbol = Symbol::new(
            interned_id,
            sym_id,
            None,
            SymbolOrigin::Module(core_mod_id),
            false,
            None,
            ScopeType::Core,
            SymbolKind::Type(type_id),
        );

        compiler.symbols.push(symbol);
        table.interned_to_sym.insert(interned_id, sym_id);

        // Numeric
        let type_id = TypeId::new(compiler.types.len() as u32);
        compiler.types.push(TypeInfo::new(
            Type::Boundaries(TypeBoundaryFlags::NUMERIC),
            core_mod_id,
        ));

        let sym_id = SymbolId::new(compiler.symbols.len() as u32);
        let interned_id = InternedId::new(intern::INTERNED_NUMERIC);
        let symbol = Symbol::new(
            interned_id,
            sym_id,
            None,
            SymbolOrigin::Module(core_mod_id),
            false,
            None,
            ScopeType::Core,
            SymbolKind::Type(type_id),
        );

        compiler.symbols.push(symbol);
        table.interned_to_sym.insert(interned_id, sym_id);

        let type_id = TypeId::new(compiler.types.len() as u32);
        compiler.types.push(TypeInfo::new(
            Type::Boundaries(TypeBoundaryFlags::SIGNED_INTEGER),
            core_mod_id,
        ));

        let sym_id = SymbolId::new(compiler.symbols.len() as u32);
        let interned_id = InternedId::new(intern::INTERNED_SIGNED_INTEGER);
        let symbol = Symbol::new(
            interned_id,
            sym_id,
            None,
            SymbolOrigin::Module(core_mod_id),
            false,
            None,
            ScopeType::Core,
            SymbolKind::Type(type_id),
        );

        compiler.symbols.push(symbol);
        table.interned_to_sym.insert(interned_id, sym_id);

        let type_id = TypeId::new(compiler.types.len() as u32);
        compiler.types.push(TypeInfo::new(
            Type::Boundaries(TypeBoundaryFlags::UNSIGNED_INTEGER),
            core_mod_id,
        ));

        let sym_id = SymbolId::new(compiler.symbols.len() as u32);
        let interned_id = InternedId::new(intern::INTERNED_UNSIGNED_INTEGER);
        let symbol = Symbol::new(
            interned_id,
            sym_id,
            None,
            SymbolOrigin::Module(core_mod_id),
            false,
            None,
            ScopeType::Core,
            SymbolKind::Type(type_id),
        );

        compiler.symbols.push(symbol);
        table.interned_to_sym.insert(interned_id, sym_id);

        let type_id = TypeId::new(compiler.types.len() as u32);
        compiler.types.push(TypeInfo::new(
            Type::Boundaries(TypeBoundaryFlags::FLOAT),
            core_mod_id,
        ));

        let sym_id = SymbolId::new(compiler.symbols.len() as u32);
        let interned_id = InternedId::new(intern::INTERNED_FLOAT);
        let symbol = Symbol::new(
            interned_id,
            sym_id,
            None,
            SymbolOrigin::Module(core_mod_id),
            false,
            None,
            ScopeType::Core,
            SymbolKind::Type(type_id),
        );

        compiler.symbols.push(symbol);
        table.interned_to_sym.insert(interned_id, sym_id);

        let type_id = TypeId::new(compiler.types.len() as u32);
        compiler.types.push(TypeInfo::new(
            Type::Boundaries(TypeBoundaryFlags::ORDERED),
            core_mod_id,
        ));

        let sym_id = SymbolId::new(compiler.symbols.len() as u32);
        let interned_id = InternedId::new(intern::INTERNED_ORDERED);
        let symbol = Symbol::new(
            interned_id,
            sym_id,
            None,
            SymbolOrigin::Module(core_mod_id),
            false,
            None,
            ScopeType::Core,
            SymbolKind::Type(type_id),
        );

        compiler.symbols.push(symbol);
        table.interned_to_sym.insert(interned_id, sym_id);

        let type_id = TypeId::new(compiler.types.len() as u32);
        compiler.types.push(TypeInfo::new(
            Type::Boundaries(TypeBoundaryFlags::COMPARABLE),
            core_mod_id,
        ));

        let sym_id = SymbolId::new(compiler.symbols.len() as u32);
        let interned_id = InternedId::new(intern::INTERNED_COMPARABLE);
        let symbol = Symbol::new(
            interned_id,
            sym_id,
            None,
            SymbolOrigin::Module(core_mod_id),
            false,
            None,
            ScopeType::Core,
            SymbolKind::Type(type_id),
        );

        compiler.symbols.push(symbol);
        table.interned_to_sym.insert(interned_id, sym_id);
    }
}

impl MemoryCost for ScriptCompiler {
    fn cost(&self) -> usize {
        let bind_cost = size_of::<Bind>();
        let mod_metadata_cost = todo!();
        // dbg!(mod_metadata_cost);
        usize::MAX;
        todo!()
    }
}

// /// All modules found during compilation
// pub mods: Vec<Module>,
// /// Type table which contains every module's stored types
// pub types: Vec<TypeInfo>,
// /// All values that were cached
// pub values: Vec<ValueInfo>,
// /// All expressions that were found
// pub exprs: Vec<ResolvedExpr>,
// /// All symbols that were found
// pub symbols: Vec<Symbol>,
// /// All symbols considered a "member" of another. This is here to serve the same purpose of a
// /// collection that would be considered fields, but more general since the language is small
// /// scale and would likely not benefit much from such a wide variety of collections.
// pub members: Vec<MemberSymbolKind>,
// /// All variables that were found
// pub variables: Vec<VarDef>,
// /// All user defined configuration. Is considered it's own class instead of a type since it
// /// behaves uniquely
// pub configs: Vec<ConfigDefRoot>,
// /// All directives that were found
// pub directives: Vec<Directive>,
// /// Scope arena
// pub scopes: Vec<ScopeInfo>,
// /// Information regarding intrinsic data such as core's `ModuleId`
// pub intrinsic_registry: IntrinsicRegistry,
// /// The current stage the compiler is in
// pub resolver_state: ResolverState,
