mod helpers;
pub mod reporter;
pub mod script_compiler_store;
pub mod script_compiler_summary;
#[cfg(test)]
mod tests;

use chrn_utils::{
    arena::Arena,
    budget::mem_cost::MemoryCost,
    id_types::{
        ConfigRootId, DirectiveId, ExprId, ImplId, ImplMemberId, InternedId, MemberId, ModuleId,
        ScopeId, SymbolId, TypeId, ValueId, VariableId,
    },
    intern,
    source_map::source_span::SourceSpan,
};
use lang::{
    directives::{Directive, TypeDirective},
    types::{boundaries::TypeBoundaryFlags, builtins::BuiltinType},
    values::ValueInfo,
};

use crate::{
    lookup::scopes::{
        self,
        scopes_concepts::{AssociatedScopeKind, IntrinsicRegistry, Scope, ScopeInfo, ScopeType},
    },
    module::module_concepts::{Bind, Import, ImportKind, Module, ModuleState},
    resolvers::resolver_state::ResolverState,
    script_compiler::helpers::{
        compiler_helpers,
        core_helpers::{self, CoreFunc},
        extern_helpers::{self, ExternKind},
    },
    semantic::hir::{
        hir_concepts::{BuiltinTypeInfo, Table, Type, TypeInfo},
        hir_exprs::ResolvedExpr,
        hir_impls::{
            ConfigDefMember, ConfigRootComplex, ConfigRootKind, ConfigRootOverride, ImplHir,
            ImplHirKind, ImplMemberKind, OptionAssignmentMember, OptionAssignmentRoot,
        },
        hir_symbols::{
            AliasDef, EnumDef, FieldRepre, FuncDef, MemberSymbolKind, StructDef, Symbol,
            SymbolKind, SymbolOrigin, TypeDef, VarDef, VariableState, VariantRepre,
        },
    },
    walk_type_id_deferred,
};

// Should this be in utils?
/// Script compiler that holds all essential data for incremental updates through resolution
pub struct ScriptCompiler {
    /// Optional bind statement that is obtained from the main module
    // Maybe the module should keep it's bind info rather than give it to the compiler so that the
    // information isn't lossy and contextual
    pub bind: Option<Bind>,
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
    /// impls!
    pub impls: Arena<ImplHir, ImplId>,
    /// All symbols considered a "member" of another. This is here to serve the same purpose of a
    /// collection that would be considered fields, but more general since the language is small
    /// scale and would likely not benefit much from such a wide variety of collections.
    pub sym_members: Arena<MemberSymbolKind, MemberId>,
    /// Impl members
    pub impl_members: Arena<ImplMemberKind, ImplMemberId>,
    /// All variables that were found
    pub variables: Arena<VarDef, VariableId>,
    /// All user defined config. Is considered it's own class instead of a type since it
    /// behaves uniquely
    pub cfgs: Arena<ConfigRootKind, ConfigRootId>,
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
// This particular type has no identifier because it's not a real type beyond being a signifier.
pub const CORE_UNKNOWN: u32 = 23;
// pub const CORE_CHARACTER_MAPPABLE: u32 = 24;

// --  DIRECTIVE CONSTANTS --
pub const DIRECTIVE_WARN_IDX: usize = 0;
pub const DIRECTIVE_IGNORE_IDX: usize = 1;
pub const DIRECTIVE_SCIENT_IDX: usize = 2;
pub const DIRECTIVE_HEX_IDX: usize = 3;
pub const DIRECTIVE_BIN_IDX: usize = 4;
pub const DIRECTIVE_OCTAL_IDX: usize = 5;
pub const DIRECTIVE_UNICODE_IDX: usize = 6;

// Ok maybe put this somewhere else bestie
// NOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOO
// Ok :3
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
        let core_mod_id = ModuleId::new(mods.len() as u32);
        let intrinsic_registry = IntrinsicRegistry::new(core_mod_id, None);

        // For capacity (em-dash). An alias replaces the import's name rather than adding a second
        // identifier, so each import is worth exactly one symbol.
        let mut import_len = 0;
        for imports in mods.iter().map(|m| &m.imports) {
            import_len += imports.len();
        }
        // (Could be hallucinating a bit here)

        // `load_core` adds the implicit core module and one core import to every module, including
        // the implicit core module itself.
        let mod_count = mods.len() + 1;
        let import_count = import_len + mod_count;
        // - 1 because core doesn't give itself core, but is semantically counted in the `mod_count`
        // + 1
        let mod_addition = mod_count + import_count - 1;
        let scope_capacity = mod_count + 1;

        let mut compiler = ScriptCompiler {
            bind,
            mods,
            // + 1 for `Type::Unknown` since it's not in either array
            types: Arena::with_capacity(
                core_helpers::CORE_BUILTIN_TYPES_DATASET.len()
                    + core_helpers::CORE_BOUNDARIES_DATASET.len()
                    + core_helpers::CORE_FUNCS_DATASET.len()
                    + 1,
            ),
            values: Arena::new(),
            exprs: Arena::new(),
            // Not sure if these should be put in their own separate constants comprised of their
            // own semantics or composed like this. Lets do nothing!
            symbols: Arena::with_capacity(
                core_helpers::CORE_BUILTIN_TYPES_DATASET.len()
                    + core_helpers::CORE_BOUNDARIES_DATASET.len()
                    + core_helpers::CORE_FUNCS_DATASET.len()
                    + mod_addition
                    + compiler_helpers::DIRECTIVES_DATASET.len(),
            ),
            sym_members: Arena::new(),
            impls: Arena::new(),
            impl_members: Arena::new(),
            variables: Arena::new(),
            cfgs: Arena::new(),
            // ignore this
            scopes: Arena::with_capacity(scope_capacity),
            directives: Arena::with_capacity(compiler_helpers::DIRECTIVES_DATASET.len()),
            //TEST:
            intrinsic_registry,
            resolver_state: ResolverState::NAMESPACE,
        };
        // Should this lazy load the section intrinsics though?
        // Yuppy
        compiler.load_core();
        compiler.load_directives();
        compiler.create_module_symbols();

        compiler
    }

    /// Creates the symbols needed for modules to be able to access to access their imports
    ///
    /// This is done by going through each module and injecting the module symbols found during the
    /// initial module dependency graph stage.
    fn create_module_symbols(&mut self) {
        // Loops through all modules, registering themselves as a symbol to themselves, iterating
        // through their imports to then inject those symbols as modules that can be looked up

        // So, if we have main AND other
        // It registers "main" as a module symbol so usage such as "main::MainType" can be used
        // It then registers a symbol for "other" so that the same "other::OtherType" semantics can
        // be done
        // If there is an alias, that is also ensured to be pushed as a symbol connected to the
        // module "other"
        for i in 0..self.mods.len() {
            let current_mod_id = ModuleId::new(i as u32);
            let module = &self.mods[current_mod_id];

            // Avoiding borrow issues by storing the ids
            let current_mod_name_id = module.name_id;
            let current_mod_id = module.mod_id;

            // Pushing the module symbol inside of itself. So if we're indexing module `main`, we
            // would be pushing `main` inside of itself, once, as a known symbol.
            let sym_id = SymbolId::new(self.symbols.len() as u32);
            let sym = Symbol::new(
                current_mod_name_id,
                sym_id,
                None,
                SymbolOrigin::Module(current_mod_id),
                true,
                Some(AssociatedScopeKind::Module(current_mod_id)),
                ScopeType::Compiler,
                SymbolKind::Namespace,
            );

            // Module symbols go into the neutral scope because, uh
            // Um
            let scope_id = self.push_scope(ScopeType::Compiler, current_mod_id);
            let scope = &mut self.get_scope_mut(scope_id).scope;
            scope
                .table
                .interned_to_sym
                .insert(current_mod_name_id, sym_id);
            self.symbols.push(sym);

            // Re-borrowing for iteration
            let module = &self.mods[current_mod_id];

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

                // An alias renames the import, so exactly one identifier is set per import.
                let import_ident_id = import
                    .sp_alias_id
                    .map(|i| i.inner)
                    .unwrap_or(import.name_id);

                let import_sym_id = SymbolId::new(self.symbols.len() as u32);
                // Pushing any imports found within the given module
                let sym = Symbol::new(
                    import_ident_id,
                    import_sym_id,
                    None,
                    SymbolOrigin::Module(current_mod_id),
                    true,
                    Some(AssociatedScopeKind::Module(mod_id)),
                    ScopeType::Compiler,
                    SymbolKind::Namespace,
                );

                // Module symbols go into the neutral scope because, uh
                // Um
                let scope_id = self.push_scope(ScopeType::Compiler, current_mod_id);

                let table = &mut self.get_scope_mut(scope_id).scope.table;
                //WARN: This may not be the best layer to deal with this issue, but this is intended
                //to filter out conflicting identifiers by default.
                if !table.interned_to_sym.contains_key(&import_ident_id) {
                    table.interned_to_sym.insert(import_ident_id, import_sym_id);
                    self.symbols.push(sym);
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

    pub(super) fn get_cfg_root_override(&self, impl_id: ImplId) -> &ConfigRootOverride {
        match &self.impls[impl_id] {
            impl_hir => match &impl_hir.kind {
                ImplHirKind::Config(cfg_id) => match &self.cfgs[*cfg_id] {
                    ConfigRootKind::Complex(_) => unreachable!(),
                    ConfigRootKind::Override(overrid) => overrid,
                },
            },
        }
    }

    pub(super) fn get_cfg_root_override_mut(&mut self, impl_id: ImplId) -> &mut ConfigRootOverride {
        match &self.impls[impl_id] {
            impl_hir => match &impl_hir.kind {
                ImplHirKind::Config(cfg_id) => match &mut self.cfgs[*cfg_id] {
                    ConfigRootKind::Override(overrid) => overrid,
                    ConfigRootKind::Complex(_) => unreachable!(),
                },
            },
        }
    }

    pub(super) fn get_cfg_root_complex(&self, impl_id: ImplId) -> &ConfigRootComplex {
        match &self.impls[impl_id] {
            impl_hir => match &impl_hir.kind {
                ImplHirKind::Config(cfg_id) => match &self.cfgs[*cfg_id] {
                    ConfigRootKind::Complex(complex) => complex,
                    ConfigRootKind::Override(_) => unreachable!(),
                },
            },
        }
    }

    pub(super) fn get_cfg_root_complex_mut(&mut self, impl_id: ImplId) -> &mut ConfigRootComplex {
        match &self.impls[impl_id] {
            impl_hir => match &impl_hir.kind {
                ImplHirKind::Config(cfg_id) => match &mut self.cfgs[*cfg_id] {
                    ConfigRootKind::Complex(complex) => complex,
                    ConfigRootKind::Override(_) => unreachable!(),
                },
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
        match &self.sym_members[member_id] {
            MemberSymbolKind::Field(field_repre) => field_repre,
            MemberSymbolKind::Variant(_) => unreachable!(),
        }
    }

    /// Assumes the member symbol given is a field
    pub(super) fn get_field_mut(&mut self, member_id: MemberId) -> &mut FieldRepre {
        match &mut self.sym_members[member_id] {
            MemberSymbolKind::Field(field_repre) => field_repre,
            _ => unreachable!(),
        }
    }

    /// Assumes the member symbol given is a variant
    pub(super) fn get_variant(&self, member_id: MemberId) -> &VariantRepre {
        match &self.sym_members[member_id] {
            MemberSymbolKind::Variant(variant_repre) => variant_repre,
            _ => unreachable!(),
        }
    }

    /// Assumes the member symbol given is a variant
    pub(super) fn get_variant_mut(&mut self, member_id: MemberId) -> &mut VariantRepre {
        match &mut self.sym_members[member_id] {
            MemberSymbolKind::Variant(variant_repre) => variant_repre,
            _ => unreachable!(),
        }
    }

    /// Assumes the impl member given is a config member
    pub fn get_cfg_def_member(&self, impl_member_id: ImplMemberId) -> &ConfigDefMember {
        match &self.impl_members[impl_member_id] {
            ImplMemberKind::ConfigDefMember(cfg_def_member) => cfg_def_member,
            _ => unreachable!(),
        }
    }

    /// Assumes the impl member given is a config member
    pub fn get_cfg_def_member_mut(&mut self, impl_member_id: ImplMemberId) -> &mut ConfigDefMember {
        match &mut self.impl_members[impl_member_id] {
            ImplMemberKind::ConfigDefMember(cfg_def_member) => cfg_def_member,
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
    pub(super) fn get_opt_assignment_root(
        &self,
        impl_member_id: ImplMemberId,
    ) -> &OptionAssignmentRoot {
        match &self.impl_members[impl_member_id] {
            ImplMemberKind::OptAssignmentRoot(opt_root) => opt_root,
            _ => unreachable!(),
        }
    }

    /// Assumes the member symbol given is a field
    pub(super) fn get_opt_assignment_root_mut(
        &mut self,
        impl_member_id: ImplMemberId,
    ) -> &mut OptionAssignmentRoot {
        match &mut self.impl_members[impl_member_id] {
            ImplMemberKind::OptAssignmentRoot(opt_root) => opt_root,
            _ => unreachable!(),
        }
    }

    /// Assumes the member symbol given is a field
    pub(super) fn get_opt_assignment_member(
        &self,
        impl_member_id: ImplMemberId,
    ) -> &OptionAssignmentMember {
        match &self.impl_members[impl_member_id] {
            ImplMemberKind::OptAssignmentMember(opt_member) => opt_member,
            _ => unreachable!(),
        }
    }

    /// Assumes the member symbol given is a field
    pub(super) fn get_opt_assignment_member_mut(
        &mut self,
        impl_member_id: ImplMemberId,
    ) -> &mut OptionAssignmentMember {
        match &mut self.impl_members[impl_member_id] {
            ImplMemberKind::OptAssignmentMember(opt_member) => opt_member,
            _ => unreachable!(),
        }
    }

    // Maybe return option?
    /// Assumes the symbol given has a `TypeId` attached. Will return a `TypeId` of `Unknown` if
    /// the `SymbolKind` is unknown.
    pub(super) fn extract_type_id(&self, sym_id: SymbolId) -> TypeId {
        match &self.symbols[sym_id] {
            sym => match &sym.kind {
                SymbolKind::Type(type_id) => *type_id,
                SymbolKind::Variable(var_id) => match self.variables[*var_id].state {
                    VariableState::ReservedTypeSlot(type_id) => type_id,
                    VariableState::Known(val_id) => self.values[val_id].type_id,
                },
                SymbolKind::ExternType | SymbolKind::Namespace | SymbolKind::Directive(_) => {
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
                SymbolKind::ExternType | SymbolKind::Directive(_) | SymbolKind::Namespace => None,
            },
        }
    }

    /// Attempts to get a `SymbolId` out of a `TypeId`
    pub(super) fn get_sym_id_from_type_id(&self, mut type_id: TypeId) -> Option<SymbolId> {
        let checked = walk_type_id_deferred!(&self.types, type_id);
        match &self.types[checked.inner].ty {
            Type::Struct(struct_def) => struct_def.sym_id.into(),
            Type::Enum(enum_def) => enum_def.sym_id.into(),
            Type::Func(func_def) => func_def.sym_id.into(),
            Type::Alias(alias_def) => alias_def.sym_id.into(),
            Type::TypeDef(type_def) => type_def.sym_id.into(),
            Type::BuiltinTypeInfo(info) => Some(info.sym_id),
            Type::Boundaries(_) | Type::Unknown => None,
            Type::Deferred(_) => unreachable!(),
        }
    }

    /// Attempts to get a `TypeId` out of the given `MemberId` if possible
    pub(super) fn get_type_id_from_member_id(&self, member_id: MemberId) -> Option<TypeId> {
        match &self.sym_members[member_id] {
            MemberSymbolKind::Field(field_repre) => Some(field_repre.type_id),
            MemberSymbolKind::Variant(variant_repre) => variant_repre.type_id,
        }
    }

    pub(super) fn get_span_from_sym_id(&self, sym_id: SymbolId) -> Option<SourceSpan> {
        match &self.symbols[sym_id].kind {
            SymbolKind::Type(type_id) => self.get_span_from_type_id(*type_id),
            SymbolKind::Variable(var_id) => Some(self.variables[*var_id].name_span),
            // SymbolKind::Config(cfg_id) => Some(self.cfgs[*cfg_id].name_span),
            SymbolKind::ExternType | SymbolKind::Namespace | SymbolKind::Directive(_) => None,
        }
    }

    pub(super) fn get_span_from_member_id(&self, member_id: MemberId) -> SourceSpan {
        match &self.sym_members[member_id] {
            MemberSymbolKind::Field(field_repre) => field_repre.name_span,
            MemberSymbolKind::Variant(variant_repre) => variant_repre.name_span,
        }
    }

    //TODO: Not
    pub(super) fn get_span_from_type_id(&self, mut type_id: TypeId) -> Option<SourceSpan> {
        let checked = walk_type_id_deferred!(&self.types, type_id);
        match &self.types[checked.inner].ty {
            Type::Struct(struct_def) => struct_def.name_span.into(),
            Type::Enum(enum_def) => enum_def.name_span.into(),
            // Functions can't be declared
            Type::Alias(alias_def) => alias_def.name_span.into(),
            Type::TypeDef(type_def) => type_def.name_span.into(),
            Type::BuiltinTypeInfo(_) => None,
            Type::Func(_) => None,
            // Type spanning needs to be reasoned about first
            // But still generally is the same as below where you can't just type stray boundaries
            Type::Boundaries(_) => todo!(),
            //NOTE: The issue is that if something is unknown, then it must be inside something like
            //a struct or enum. You can't really just declare an unknown type, since at that point
            //it wouldn't be seen as a type anyways.
            Type::Unknown => todo!("Should still be spanned though"),
            Type::Deferred(_) => unreachable!(),
        }
    }

    /// If the given `TypeId` is a `BuiltinType` returns `true`, `false` otherwise
    pub(super) fn check_builtin(&self, mut type_id: TypeId) -> bool {
        let checked = walk_type_id_deferred!(&self.types, type_id);
        match &self.types[checked.inner].ty {
            Type::BuiltinTypeInfo(_) => true,
            Type::Struct(_)
            | Type::Enum(_)
            | Type::Func(_)
            | Type::Alias(_)
            | Type::TypeDef(_)
            | Type::Boundaries(_)
            | Type::Unknown => false,
            // Can builtins be deferred to?
            // I believe so yes.
            // Thank you
            Type::Deferred(_) => unreachable!(),
        }
    }

    // TODO: Fix type metadata
    /// Returns `None` if type `TypeBoundaryFlags` is found and there's more
    /// than one boundary encoded, otherwise returns `Some`
    pub(super) fn get_name_id_from_type_id(&self, mut type_id: TypeId) -> Option<InternedId> {
        let checked = walk_type_id_deferred!(&self.types, type_id);

        match &self.types[checked.inner].ty {
            Type::BuiltinTypeInfo(builtin_type) => builtin_type.ty.kind().name_id().into(),
            Type::Struct(struct_def) => self.symbols[struct_def.sym_id].name_id.into(),
            Type::Enum(enum_def) => self.symbols[enum_def.sym_id].name_id.into(),
            // Functions can't be declared
            Type::Alias(alias_def) => self.symbols[alias_def.sym_id].name_id.into(),
            // WARN: Inconsistency
            Type::TypeDef(type_def) => type_def.name_id.into(),
            Type::Func(func) => func.name_id.into(),
            // Should the return type be String then?
            // This absolutely can't return a type id
            Type::Boundaries(boundary_flags) => boundary_flags.name_id().into(),
            // Not classifying unknown as a known identifier since it may lead to mis-usage of
            // the identifier as though it really is the identifier of an actual declared type,
            // rather than rephrasing for the fact that the type itself is unknown.
            //
            // Phrases like "The type `Unknown`" sound wrong because it's not a type it's a state
            Type::Unknown => InternedId::new(intern::INTERNED_UNKNOWN).into(),
            Type::Deferred(_) => unreachable!(),
        }
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
        scopes::find_scope_in_mod(self, scope_type, owner).map(|s| s.scope.scope_id)
    }

    /// Get's the `ScopeId` assuming that the scope already exists. Panics otherwise.
    ///
    /// This exists because if the current module has something like a typedef in the semantic stage,
    /// that means the parser itself already checked if it was legal grammar-wise.
    pub fn extract_scope_id(&self, scope_type: ScopeType, owner_id: ModuleId) -> ScopeId {
        scopes::find_scope_in_mod(self, scope_type, owner_id)
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
        if let Some(scope_info) = scopes::find_scope_in_mod(self, scope_type, owner_id) {
            return scope_info.scope.scope_id;
        }

        // Beep
        let intrinsic_scope_opt: Option<ScopeId> = match scope_type {
            // Lazy
            ScopeType::Override => self.load_override_symbols().into(),
            ScopeType::Local
            | ScopeType::Neutral
            | ScopeType::Var
            | ScopeType::Nest
            | ScopeType::Complex
            | ScopeType::Compiler
            | ScopeType::Core => None,
        };

        let scope_id = ScopeId::new(self.scopes.len() as u16);
        let scope = Scope::new(scope_id, scope_type, false, intrinsic_scope_opt);
        let scope_info = ScopeInfo::new(scope, None, owner_id);
        self.scopes.push(scope_info);

        // Giving module the scope id so that it can have it searched in `scopes::` operations
        let owner_mod = &mut self.mods[owner_id];
        owner_mod.scopes.push(scope_id);
        // owner_mod.held_scopes |= scope_type.to_u8();

        scope_id
    }

    /// Returns `true` if the type is unknown, false otherwise
    pub fn check_unknown(&self, mut type_id: TypeId) -> bool {
        // This limit is semi-random
        let checked = walk_type_id_deferred!(self.types, type_id);
        match self.types[checked.inner].ty {
            Type::Unknown => true,
            _ => false,
        }
    }

    // -- STARTUP --

    /// Loads the core module
    fn load_core(&mut self) {
        // Ignore this
        let mut table = Table::with_capacities(
            0,
            // Does not include + 1 because `Unknown` has no identifier given
            core_helpers::CORE_BUILTIN_TYPES_DATASET.len()
                + core_helpers::CORE_BOUNDARIES_DATASET.len()
                + core_helpers::CORE_FUNCS_DATASET.len(),
        );

        //TODO: If namespace core exists as a module then should error earlier
        let core_name_id = InternedId::new(intern::INTERNED_CORE);
        let core_mod_id = ModuleId::new(self.mods.len() as u32);
        let core_scope_id = ScopeId::new(self.scopes.len() as u16);

        // Uses module only so that there are no possible borrow checker issues.
        let mut core_mod = Module::new(
            core_name_id,
            ModuleState::Loaded,
            core_mod_id,
            None,
            Vec::new(),
            None,
        );

        self.load_core_types(core_mod.mod_id, &mut table);
        self.load_core_funcs(core_mod.mod_id, &mut table);

        core_mod.exports.reserve_exact(table.interned_to_sym.len());

        // Exporting all created symbols from core
        for sym_id in table.interned_to_sym.values().copied() {
            core_mod.exports.push(sym_id);
        }

        // Done adding all of core
        let scope_id = ScopeId::new(self.scopes.len() as u16);
        let scope = Scope::with_table(scope_id, ScopeType::Core, None, true, table);
        let scope_info = ScopeInfo::new(scope, None, core_mod_id);

        self.scopes.push(scope_info);
        core_mod.scopes.push(scope_id);

        self.mods.push(core_mod);

        let core_import = Import::new(core_name_id, ImportKind::Core(core_mod_id), None);

        // Injecting core as an import and pushing it's scope so user modules can search it
        for user_mod in &mut self.mods.items {
            user_mod.imports.push(core_import.clone());
            user_mod.scopes.push(core_scope_id);
        }
    }

    /// Loads all compiler known directives
    fn load_directives(&mut self) {
        for (name_id, directive) in compiler_helpers::DIRECTIVES_DATASET {
            self.register_directive(name_id, directive);
        }
        debug_assert_eq!(
            compiler_helpers::DIRECTIVES_DATASET.len(),
            self.directives.len()
        );
    }

    fn register_directive(&mut self, interned_id: InternedId, directive: Directive) {
        let sym_id = SymbolId::new(self.symbols.len() as u32);
        let directive_id = directive_to_id(&directive);
        debug_assert_eq!(directive_id.id, self.directives.len() as u32);

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

        self.symbols.push(sym);
        self.directives.push(directive);
    }

    /// Creates scope with the constants needed for an `override` section to function then returns
    /// it's `ScopeId`
    ///
    /// The idea behind this is that the `impl` for JAVA, and all other symbols are customized on
    /// the user-end. By default, these symbols are just namespaces that have content inside, like
    /// "types" and their own specific java::int and so on.
    ///
    /// If the override instrinsic `ScopeId` already exists then this will just return that scope id.
    fn load_override_symbols(&mut self) -> ScopeId {
        //NOTE: Just in case this is called without knowledge of if the override scope exists.
        if let Some(inner) = self.intrinsic_registry.override_scope_id {
            return inner;
        }

        // IS it from core? The semantics are getting a little lost
        //
        // Saying it's not from core for now because !
        let core_mod_id = self.intrinsic_registry.core_mod_id;
        let scope_id = ScopeId::new(self.scopes.len() as u16);

        // Override intrisic scope's table which holes stuff like "RUST" and "JAVA" namespaces
        let scope = Scope::new(scope_id, ScopeType::Override, true, None);
        self.scopes.push(ScopeInfo::new(scope, None, core_mod_id));

        self.register_all_extern_namespaces(scope_id);

        // -- FINAL --
        // Pushing the intrinsic scope
        // let override_scope_id = ScopeId::new(self.scopes.len() as u16);
        // let scope = Scope::with_table(override_scope_id, scope_type, None, true, override_table);
        // self.scopes.push(ScopeInfo::new(scope, None, core_mod_id));

        // ????

        scope_id
    }

    /// Entry point to the recursive registeration of all `override` external type namespace symbols
    fn register_all_extern_namespaces(&mut self, current_scope_id: ScopeId) {
        // iterative version was a bit verbose..
        // let mut stack: Vec<ExternFrame> = Vec::with_capacity(10);

        for extern_kind in extern_helpers::ALL_EXTERN_NAMESPACES_DATASET {
            self.register_extern_namespace(current_scope_id, extern_kind);
        }
    }

    /// Uses dataset from `extern_helpers.rs` of all `ExternType` intrinsics to push scopes and
    /// symbols recursively
    fn register_extern_namespace(
        &mut self,
        current_scope_id: ScopeId,
        extern_kinds: &[ExternKind],
    ) {
        for kind in extern_kinds {
            match kind {
                ExternKind::Namespace(extern_namespace) => {
                    // Creating namespace as a symbol with the identifier associated first.
                    let sym_id = SymbolId::new(self.symbols.len() as u32);
                    let scope_id = ScopeId::new(self.scopes.len() as u16);

                    let sym = Symbol::new(
                        extern_namespace.name_id,
                        sym_id,
                        None,
                        SymbolOrigin::Compiler,
                        true,
                        AssociatedScopeKind::Scope(scope_id).into(),
                        ScopeType::Compiler,
                        SymbolKind::Namespace,
                    );

                    // Putting the found namespace into the current table's scope before recursively
                    // descending into new scope
                    let current_table = &mut self.scopes[current_scope_id].scope.table;
                    self.symbols.push(sym);
                    current_table
                        .interned_to_sym
                        .insert(extern_namespace.name_id, sym_id);

                    let scope = Scope::new(scope_id, ScopeType::Compiler, true, None);

                    // Pushing it's scope so that future scope instantiations are aligned with len()
                    self.scopes.push(ScopeInfo::new(
                        scope,
                        sym_id.into(),
                        self.intrinsic_registry.core_mod_id,
                    ));

                    self.register_extern_namespace(scope_id, extern_namespace.syms);
                }
                ExternKind::Symbol(interned_id) => {
                    let sym_id = SymbolId::new(self.symbols.len() as u32);
                    let sym = Symbol::new(
                        *interned_id,
                        sym_id,
                        None,
                        SymbolOrigin::Compiler,
                        true,
                        None,
                        ScopeType::Compiler,
                        SymbolKind::ExternType,
                    );

                    let current_table = &mut self.scopes[current_scope_id].scope.table;
                    self.symbols.push(sym);
                    current_table.interned_to_sym.insert(*interned_id, sym_id);
                }
            }
        }
    }

    /// Helper to load all of core's functions and predicates
    fn load_core_funcs(&mut self, core_mod_id: ModuleId, table: &mut Table) {
        for core_func in &core_helpers::CORE_FUNCS_DATASET {
            self.register_core_func(core_func, table, core_mod_id);
        }
    }

    /// Registers a single core function and pushes it
    fn register_core_func(
        &mut self,
        core_func: &CoreFunc,
        table: &mut Table,
        core_mod_id: ModuleId,
    ) {
        let type_id = TypeId::new(self.types.len() as u32);
        let sym_id = SymbolId::new(self.symbols.len() as u32);
        let name_id = InternedId::new(core_func.name);

        let func_def = FuncDef::new(
            sym_id,
            name_id,
            core_func.kind,
            core_func.is_callable,
            core_func.type_constraints,
            core_func.arg_constraints.to_vec(),
            core_func.affects_type_constraint,
            TypeId::new(core_func.ret_type),
        );

        self.types
            .push(TypeInfo::new(Type::Func(func_def), core_mod_id));

        let sym = Symbol::new(
            name_id,
            sym_id,
            None,
            SymbolOrigin::Module(core_mod_id),
            false,
            None,
            ScopeType::Core,
            SymbolKind::Type(type_id),
        );

        self.symbols.push(sym);
        table.interned_to_sym.insert(name_id, sym_id);
    }

    // Make &mut self?
    // --- Beep
    /// Helper to load all of core's types
    fn load_core_types(&mut self, core_mod_id: ModuleId, table: &mut Table) {
        // -- Concrete types --
        for (interned, ty, core_id) in core_helpers::CORE_BUILTIN_TYPES_DATASET.iter().cloned() {
            debug_assert_eq!(self.types.len() as u32, core_id);
            let interned_id = InternedId::new(interned);
            self.register_builtin(interned_id, ty, table, core_mod_id);
        }

        debug_assert_eq!(self.types.len() as u32, CORE_UNKNOWN);
        // Is a special cookie because you're not supposed to be able to instantiate an `Unknown`
        // type on purpose. This is just a compiler internal type.
        self.types.push(TypeInfo::new(Type::Unknown, core_mod_id));

        // -- Boundaries --
        for (interned, flags) in core_helpers::CORE_BOUNDARIES_DATASET.iter().cloned() {
            let interned_id = InternedId::new(interned);
            self.register_boundary(interned_id, flags, table, core_mod_id);
        }
    }

    /// Registers a single core builtin-type and pushes it
    fn register_builtin(
        &mut self,
        name_id: InternedId,
        builtin_ty: BuiltinType,
        table: &mut Table,
        core_mod_id: ModuleId,
    ) {
        let type_id = TypeId::new(self.types.len() as u32);
        let sym_id = SymbolId::new(self.symbols.len() as u32);

        self.types.push(TypeInfo::new(
            Type::BuiltinTypeInfo(BuiltinTypeInfo::new(sym_id, builtin_ty)),
            core_mod_id,
        ));
        let sym = Symbol::new(
            name_id,
            sym_id,
            None,
            SymbolOrigin::Module(core_mod_id),
            false,
            None,
            ScopeType::Core,
            SymbolKind::Type(type_id),
        );

        self.symbols.push(sym);
        table.interned_to_sym.insert(name_id, sym_id);
    }

    /// Registers a single core boundary type and pushes it
    fn register_boundary(
        &mut self,
        name_id: InternedId,
        flags: TypeBoundaryFlags,
        table: &mut Table,
        core_mod_id: ModuleId,
    ) {
        let type_id = TypeId::new(self.types.len() as u32);
        let sym_id = SymbolId::new(self.symbols.len() as u32);

        self.types
            .push(TypeInfo::new(Type::Boundaries(flags), core_mod_id));
        let sym = Symbol::new(
            name_id,
            sym_id,
            None,
            SymbolOrigin::Module(core_mod_id),
            false,
            None,
            ScopeType::Core,
            SymbolKind::Type(type_id),
        );

        self.symbols.push(sym);
        table.interned_to_sym.insert(name_id, sym_id);
    }
}

impl MemoryCost for ScriptCompiler {
    fn cost(&self) -> usize {
        let bind_cost = size_of::<Bind>();
        let mod_metadata_cost = todo!();
        // dbg!(mod_metadata_cost);
        todo!()
    }
}
