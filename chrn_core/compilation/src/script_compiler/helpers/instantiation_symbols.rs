use chrn_utils::id_types::{AstId, InternedId, SymbolId};
use lang::{types::builtins::BuiltinType, values::Value};

use crate::{
    lookup::scopes::scopes_concepts::{AssociatedScopeKind, ScopeType},
    semantic::hir::hir_symbols::{Symbol, SymbolKind, SymbolOrigin},
};

/// Abstraction to allow for `Symbol` to be made procedurally in a composed manner
#[derive(Debug)]
pub struct InstantiationSymbolBase {
    pub name_id: InternedId,
    pub sym_origin: SymbolOrigin,
    pub scope_origin: ScopeType,
    pub is_priv: bool,
    pub kind: InstantiationSymbolKind,
}

impl InstantiationSymbolBase {
    pub const fn new(
        name_id: InternedId,
        sym_origin: SymbolOrigin,
        scope_origin: ScopeType,
        is_priv: bool,
        kind: InstantiationSymbolKind,
    ) -> Self {
        Self {
            name_id,
            sym_origin,
            scope_origin,
            is_priv,
            kind,
        }
    }

    /// Helper to convert to symbol using the already present metadata
    pub fn to_sym(
        &self,
        sym_id: SymbolId,
        ast_id: Option<AstId>,
        associated_scope: Option<AssociatedScopeKind>,
        kind: SymbolKind,
    ) -> Symbol {
        Symbol::new(
            self.name_id,
            sym_id,
            ast_id,
            self.sym_origin,
            self.is_priv,
            associated_scope,
            self.scope_origin,
            kind,
        )
    }
}

/// Abstraction to allow for `SymbolKind` to be made procedurally in a composed manner
#[derive(Debug)]
pub enum InstantiationSymbolKind {
    Namespace(&'static [InstantiationSymbolBase]),
    Variable(InstantiationVariable),
    ExternType,
}

#[derive(Debug)]
pub struct InstantiationVariable {
    // It needs a symbol to exist so I'd presume this is ok since the symbol would have the
    // identifier already
    // pub name_id: InternedId,
    pub ty: InstiationType,
    pub val: InstiationValue,
}

impl InstantiationVariable {
    pub const fn new(ty: InstiationType, val: InstiationValue) -> Self {
        Self { ty, val }
    }
}

/// Abstraction to allow for `Type` to be made procedurally in a composed manner
#[derive(Debug, Clone)]
pub enum InstiationType {
    BuiltinType(BuiltinType),
}

/// Abstraction to allow for `Value` to be made procedurally in a composed manner
#[derive(Debug)]
pub enum InstiationValue {
    I64(i64),
    F64(f64),
    Bool(bool),
    Char(char),
    // Function!
    // Func(SymbolId),
    Tuple(Vec<InstiationValue>),
    Array(Vec<InstiationValue>),
    Str(InternedId),
}

impl InstiationValue {
    /// Converts itself to the `Value` type
    pub const fn to_val(&self) -> Value {
        match self {
            InstiationValue::I64(val) => Value::I64(*val),
            InstiationValue::F64(val) => Value::F64(*val),
            InstiationValue::Bool(b) => Value::Bool(*b),
            InstiationValue::Char(c) => Value::Char(*c),
            InstiationValue::Str(id) => Value::InternedStr(*id),
            _ => unreachable!(),
            // InstiationValue::Tuple(instiation_values) => todo!(),
            // InstiationValue::Array(instiation_values) => todo!(),
        }
    }
}
