use chrn_utils::intern::Intern;
use compilation::{
    script_compiler::ScriptCompiler,
    semantic::hir::{Symbol, SymbolKind, Type},
};

use crate::dump_settings::DumpSettings;

pub struct PrintContext<'a> {
    compiler: &'a ScriptCompiler,
    indent: usize,
    interner: &'a Intern,
}

impl PrintContext<'_> {
    pub(crate) fn new<'a>(
        compiler: &'a ScriptCompiler,
        indent: usize,
        interner: &'a Intern,
    ) -> PrintContext<'a> {
        PrintContext {
            compiler,
            indent,
            interner,
        }
    }

    pub(crate) fn increase_indent(&mut self) {
        self.indent += 4;
    }

    pub(crate) fn decrease_indent(&mut self) {
        self.indent -= 4;
    }
}

// TEST:
// More like stringifying

pub fn print_env(compiler: &ScriptCompiler, settings: &DumpSettings, interner: &Intern) -> String {
    let mut ctx = PrintContext::new(compiler, 0, interner);
    todo!()
}

pub fn print_symbol(
    compiler: &ScriptCompiler,
    sym: &Symbol,
    settings: &DumpSettings,
    interner: &Intern,
) -> String {
    let ident = interner.search(sym.name_id);
    let access_level = if sym.is_priv { "private" } else { "public" };
    // Maybe can sort by declaration module normally
    let full_str = match sym.kind {
        SymbolKind::Type(type_id) => {
            let ty_info = &compiler.types[type_id.id as usize];
            let ty_str = format_type(compiler, &ty_info.ty, interner);

            let mod_owner = &compiler.mods[sym.owner.id];
            let owner_name = interner.search(mod_owner.name_id);

            let decl_scope = sym.scope_origin;
            sym.scope_origin;
            println!("{ident}: {ty_str}");
            // let associated_scope_str = if let Some(inner) = sym.associated_scope {
            //     match inner {
            //         AssociatedScopeKind::Module(module_id) => todo!(),
            //         // A type should not have an inner module scope.
            //         AssociatedScopeKind::Scope(scope_id) => unreachable!(),
            //     }
            // } else {
            //     "".into()
            // };

            todo!();
        }
        SymbolKind::Val(val_id) => todo!(),
        SymbolKind::ReservedTypeSlot(type_id) => "Unknown".to_string(),
        SymbolKind::Module(mod_id) => todo!(),
        SymbolKind::Config(cfg_id) => todo!(),
    };

    full_str
}

fn format_type(compiler: &ScriptCompiler, ty: &Type, interner: &Intern) -> String {
    match ty {
        Type::BuiltinType(builtin_type) => todo!(),
        Type::Struct(struct_def) => {
            todo!()
        }
        Type::Enum(enum_def) => todo!(),
        Type::Func(func_def) => todo!(),
        Type::Alias(alias_def) => todo!(),
        Type::TypeDef(type_def) => todo!(),
        Type::Constrained(type_constraint_flags) => todo!(),
        Type::Deferred(type_id) => todo!(),
        Type::Unknown => todo!(),
    }
}

fn format_type_args() {}

fn format_type_constraints() {}
