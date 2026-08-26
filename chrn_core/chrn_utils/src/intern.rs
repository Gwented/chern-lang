use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use crate::id_types::{InternedId, PathId};

// Um
pub const INTERNED_SELF: u32 = 0;
pub const INTERNED_STRUCT: u32 = 1;
pub const INTERNED_ENUM: u32 = 2;
pub const INTERNED_IMPORT: u32 = 3;
pub const INTERNED_EXPORT: u32 = 4;
pub const INTERNED_BIND: u32 = 5;
pub const INTERNED_ALIAS: u32 = 6;
pub const INTERNED_LET: u32 = 7;
pub const INTERNED_CHANGE: u32 = 8;
pub const INTERNED_AS: u32 = 9;
pub const INTERNED_VAR: u32 = 10;
pub const INTERNED_NEST: u32 = 11;
pub const INTERNED_COMPLEX: u32 = 12;
pub const INTERNED_OVERRIDE: u32 = 13;
pub const INTERNED_TRUE: u32 = 14;
pub const INTERNED_FALSE: u32 = 15;
pub const INTERNED_IS_EMPTY: u32 = 16;
pub const INTERNED_IS_WHITESPACE: u32 = 17;
pub const INTERNED_RANGE: u32 = 18;
pub const INTERNED_STARTSW: u32 = 19;
pub const INTERNED_ENDSW: u32 = 20;
pub const INTERNED_CONTAINS: u32 = 21;
pub const INTERNED_EQUALS: u32 = 22;
pub const INTERNED_I8: u32 = 23;
pub const INTERNED_U8: u32 = 24;
pub const INTERNED_I16: u32 = 25;
pub const INTERNED_U16: u32 = 26;
pub const INTERNED_F16: u32 = 27;
pub const INTERNED_I32: u32 = 28;
pub const INTERNED_U32: u32 = 29;
pub const INTERNED_F32: u32 = 30;
pub const INTERNED_I64: u32 = 31;
pub const INTERNED_U64: u32 = 32;
pub const INTERNED_F64: u32 = 33;
pub const INTERNED_I128: u32 = 34;
pub const INTERNED_U128: u32 = 35;
pub const INTERNED_F128: u32 = 36;
pub const INTERNED_SIZED: u32 = 37;
pub const INTERNED_UNSIZED: u32 = 38;
pub const INTERNED_BOOL: u32 = 39;
pub const INTERNED_NIL: u32 = 40;
pub const INTERNED_CHAR: u32 = 41;
pub const INTERNED_STR: u32 = 42;
pub const INTERNED_BIGINT: u32 = 43;
pub const INTERNED_BIGFLOAT: u32 = 44;
pub const INTERNED_LIST: u32 = 45;
pub const INTERNED_SET: u32 = 46;
pub const INTERNED_MAP: u32 = 47;
pub const INTERNED_TUPLE: u32 = 48;
pub const INTERNED_RUNTIME: u32 = 49;
pub const INTERNED_CORE: u32 = 50;
pub const INTERNED_IN: u32 = 51;
pub const INTERNED_RANGED: u32 = 52;
pub const INTERNED_CHARACTER_MAPPABLE: u32 = 53;
pub const INTERNED_COLLECTION: u32 = 54;
pub const INTERNED_HAS_LEN: u32 = 55;
pub const INTERNED_INTEGER: u32 = 56;
pub const INTERNED_NUMERIC: u32 = 57;
pub const INTERNED_SIGNED_INTEGER: u32 = 58;
pub const INTERNED_UNSIGNED_INTEGER: u32 = 59;
pub const INTERNED_FLOAT: u32 = 60;
pub const INTERNED_ORDERED: u32 = 61;
pub const INTERNED_COMPARABLE: u32 = 62;
pub const INTERNED_JAVA_UPPER: u32 = 63;
pub const INTERNED_DEFAULT_VAL: u32 = 64;
pub const INTERNED_WARN: u32 = 65;
pub const INTERNED_IGNORE: u32 = 66;
pub const INTERNED_SCIENT: u32 = 67;
pub const INTERNED_HEX: u32 = 68;
pub const INTERNED_BIN: u32 = 69;
pub const INTERNED_OCTAL: u32 = 70;
pub const INTERNED_IDENTS: u32 = 71;
pub const INTERNED_CASES: u32 = 72;
pub const INTERNED_JAVA_LOWER: u32 = 73;
pub const INTERNED_INT: u32 = 74;
pub const INTERNED_UNICODE: u32 = 75;
pub const INTERNED_UNKNOWN: u32 = 76;
pub const INTERNED_TYPES_LOWER: u32 = 77;
pub const INTERNED_MAX_UPPER: u32 = 78;
pub const INTERNED_MIN_UPPER: u32 = 79;
pub const INTERNED_FOR: u32 = 80;

// Collection,
// CharacterMappable,
// HasLen,
// Ranged,
// Ordered,
// Comparable,
// Numeric,
// Integer,
// Float,
// Bool,
// Str,
// Char,
// Nil,
// MAKE THE MACRO PLEASE
// What macro. What is a macro? What is hygiene?

/// Every compiler known interned string, paired with its id. Order must match the ids.
pub static PRELOADED_STRINGS: [(&str, u32); INTERNER_PRELOAD_SIZE] = [
    ("self", INTERNED_SELF),
    ("struct", INTERNED_STRUCT),
    ("enum", INTERNED_ENUM),
    ("import", INTERNED_IMPORT),
    ("export", INTERNED_EXPORT),
    ("bind", INTERNED_BIND),
    ("alias", INTERNED_ALIAS),
    ("let", INTERNED_LET),
    ("change", INTERNED_CHANGE),
    ("as", INTERNED_AS),
    ("var", INTERNED_VAR),
    ("nest", INTERNED_NEST),
    ("complex", INTERNED_COMPLEX),
    ("override", INTERNED_OVERRIDE),
    ("true", INTERNED_TRUE),
    ("false", INTERNED_FALSE),
    ("IsEmpty", INTERNED_IS_EMPTY),
    ("IsWhitespace", INTERNED_IS_WHITESPACE),
    ("Range", INTERNED_RANGE),
    ("StartsW", INTERNED_STARTSW),
    ("EndsW", INTERNED_ENDSW),
    ("Contains", INTERNED_CONTAINS),
    ("Equals", INTERNED_EQUALS),
    ("i8", INTERNED_I8),
    ("u8", INTERNED_U8),
    ("i16", INTERNED_I16),
    ("u16", INTERNED_U16),
    ("f16", INTERNED_F16),
    ("i32", INTERNED_I32),
    ("u32", INTERNED_U32),
    ("f32", INTERNED_F32),
    ("i64", INTERNED_I64),
    ("u64", INTERNED_U64),
    ("f64", INTERNED_F64),
    ("i128", INTERNED_I128),
    ("u128", INTERNED_U128),
    ("f128", INTERNED_F128),
    ("sized", INTERNED_SIZED),
    ("unsized", INTERNED_UNSIZED),
    ("bool", INTERNED_BOOL),
    ("nil", INTERNED_NIL),
    ("char", INTERNED_CHAR),
    ("str", INTERNED_STR),
    ("BigInt", INTERNED_BIGINT),
    ("BigFloat", INTERNED_BIGFLOAT),
    ("List", INTERNED_LIST),
    ("Set", INTERNED_SET),
    ("Map", INTERNED_MAP),
    ("Tuple", INTERNED_TUPLE),
    ("Runtime", INTERNED_RUNTIME),
    ("core", INTERNED_CORE),
    ("in", INTERNED_IN),
    ("Ranged", INTERNED_RANGED),
    ("CharacterMappable", INTERNED_CHARACTER_MAPPABLE),
    ("Collection", INTERNED_COLLECTION),
    ("HasLen", INTERNED_HAS_LEN),
    ("Integer", INTERNED_INTEGER),
    ("Numeric", INTERNED_NUMERIC),
    ("SignedInteger", INTERNED_SIGNED_INTEGER),
    ("UnsignedInteger", INTERNED_UNSIGNED_INTEGER),
    ("Float", INTERNED_FLOAT),
    ("Ordered", INTERNED_ORDERED),
    ("Comparable", INTERNED_COMPARABLE),
    ("JAVA", INTERNED_JAVA_UPPER),
    ("default_val", INTERNED_DEFAULT_VAL),
    ("warn", INTERNED_WARN),
    ("ignore", INTERNED_IGNORE),
    ("scient", INTERNED_SCIENT),
    ("hex", INTERNED_HEX),
    ("bin", INTERNED_BIN),
    ("octal", INTERNED_OCTAL),
    ("idents", INTERNED_IDENTS),
    ("cases", INTERNED_CASES),
    ("java", INTERNED_JAVA_LOWER),
    ("int", INTERNED_INT),
    ("unicode", INTERNED_UNICODE),
    ("Unknown", INTERNED_UNKNOWN),
    ("types", INTERNED_TYPES_LOWER),
    ("MAX", INTERNED_MAX_UPPER),
    ("MIN", INTERNED_MIN_UPPER),
    ("for", INTERNED_FOR),
];

/// Interner used for the chrn language
#[derive(Debug)]
pub struct Intern {
    // Um
    id_map: HashMap<String, u32>,
    path_map: HashMap<PathBuf, u32>,
    // Is super solely for lib.rs tests
    pub(super) stored_strs: Vec<String>,
    stored_paths: Vec<PathBuf>,
    // Maybe not
    pos: usize,
}

pub const INTERNER_PRELOAD_SIZE: usize = (INTERNED_FOR + 1) as usize;

impl Intern {
    /// Creates interner that pre-loads itself with all defined interned string literals.
    pub fn init() -> Intern {
        let mut interner = Intern {
            id_map: HashMap::with_capacity(INTERNER_PRELOAD_SIZE),
            stored_strs: Vec::with_capacity(INTERNER_PRELOAD_SIZE),
            path_map: HashMap::new(),
            stored_paths: Vec::new(),
            pos: 0,
        };

        // Pre-loading every language required string literal
        for (s, id) in PRELOADED_STRINGS {
            interner.register(s, id);
        }

        interner.pos = interner.stored_strs.len();

        interner
    }

    /// Internal helper used for registering compiler known interned strings
    fn register(&mut self, s: &str, id: u32) {
        debug_assert_eq!(self.stored_strs.len() as u32, id);
        self.id_map.insert(s.to_string(), id);
        self.stored_strs.push(s.to_string());
    }

    pub fn intern(&mut self, s: &str) -> InternedId {
        if let Some(id) = self.id_map.get(s) {
            return InternedId::new(*id);
        }

        let id = self.stored_strs.len() as u32;
        self.pos += 1;

        let new_str = s.to_string();

        self.id_map.insert(new_str.clone(), id);
        self.stored_strs.push(new_str);

        InternedId::new(id)
    }

    /// Method for `self` to intern all of `other`'s stored strings and paths
    pub fn append(&mut self, other: &Intern) {
        for i in INTERNER_PRELOAD_SIZE..other.stored_strs.len() {
            let current = &other.stored_strs[i];
            self.intern(current);
        }

        for i in 0..other.stored_paths.len() {
            let current = &other.stored_paths[i];
            self.intern_path(current);
        }
    }

    pub fn intern_path(&mut self, s: &Path) -> PathId {
        if let Some(id) = self.path_map.get(s) {
            return PathId::new(*id);
        }

        let id = self.stored_paths.len() as u32;
        self.pos += 1;

        let new_path = s.to_path_buf();

        self.path_map.insert(new_path.clone(), id);
        self.stored_paths.push(new_path);

        PathId::new(id)
    }

    pub fn search(&self, interned_id: InternedId) -> &str {
        &self.stored_strs[interned_id.id as usize]
    }

    pub fn search_idx(&self, idx: usize) -> &str {
        &self.stored_strs[idx]
    }

    pub fn try_search_str(&self, s: &str) -> Option<InternedId> {
        self.id_map.get(s).map(|id| InternedId::new(*id))
    }

    pub fn search_path(&self, path_id: PathId) -> &Path {
        &self.stored_paths[path_id.id as usize]
    }

    pub fn search_direct_path(&self, path: &Path) -> Option<&Path> {
        if let Some(id) = self.path_map.get(path) {
            return Some(self.search_path(PathId::new(*id)));
        }

        None
    }
}
