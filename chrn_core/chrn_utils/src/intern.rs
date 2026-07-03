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
pub const INTERNED_DEFAULT_VALUE: u32 = 64;
pub const INTERNED_WARN: u32 = 65;
pub const INTERNED_IGNORE: u32 = 66;
pub const INTERNED_SCIENT: u32 = 67;
pub const INTERNED_HEX: u32 = 68;
pub const INTERNED_BIN: u32 = 69;
pub const INTERNED_OCTAL: u32 = 70;

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

/// Simple interner used for the chrn language
#[derive(Debug)]
pub struct Intern {
    // Um
    id_map: HashMap<String, u32>,
    path_map: HashMap<PathBuf, u32>,
    stored_strs: Vec<String>,
    stored_paths: Vec<PathBuf>,
    // Maybe not
    pos: usize,
}

pub const INTERNER_PRELOAD_SIZE: usize = (INTERNED_JAVA_UPPER + 1) as usize;

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

        // Pre-loading every language special string literals which includes keywords and types.
        interner.id_map.insert("self".to_string(), INTERNED_SELF);
        interner.stored_strs.push("self".to_string());
        interner
            .id_map
            .insert("struct".to_string(), INTERNED_STRUCT);
        interner.stored_strs.push("struct".to_string());
        interner.id_map.insert("enum".to_string(), INTERNED_ENUM);
        interner.stored_strs.push("enum".to_string());
        interner
            .id_map
            .insert("import".to_string(), INTERNED_IMPORT);
        interner.stored_strs.push("import".to_string());
        interner
            .id_map
            .insert("export".to_string(), INTERNED_EXPORT);
        interner.stored_strs.push("export".to_string());
        interner.id_map.insert("bind".to_string(), INTERNED_BIND);
        interner.stored_strs.push("bind".to_string());
        interner.id_map.insert("alias".to_string(), INTERNED_ALIAS);
        interner.stored_strs.push("alias".to_string());
        interner.id_map.insert("let".to_string(), INTERNED_LET);
        interner.stored_strs.push("let".to_string());
        interner
            .id_map
            .insert("change".to_string(), INTERNED_CHANGE);
        interner.stored_strs.push("change".to_string());
        interner.id_map.insert("as".to_string(), INTERNED_AS);
        interner.stored_strs.push("as".to_string());
        interner.id_map.insert("var".to_string(), INTERNED_VAR);
        interner.stored_strs.push("var".to_string());
        interner.id_map.insert("nest".to_string(), INTERNED_NEST);
        interner.stored_strs.push("nest".to_string());
        interner
            .id_map
            .insert("complex".to_string(), INTERNED_COMPLEX);
        interner.stored_strs.push("complex".to_string());
        interner
            .id_map
            .insert("override".to_string(), INTERNED_OVERRIDE);
        interner.stored_strs.push("override".to_string());
        interner.id_map.insert("true".to_string(), INTERNED_TRUE);
        interner.stored_strs.push("true".to_string());
        interner.id_map.insert("false".to_string(), INTERNED_FALSE);
        interner.stored_strs.push("false".to_string());
        interner
            .id_map
            .insert("IsEmpty".to_string(), INTERNED_IS_EMPTY);
        interner.stored_strs.push("IsEmpty".to_string());
        interner
            .id_map
            .insert("IsWhitespace".to_string(), INTERNED_IS_WHITESPACE);
        interner.stored_strs.push("IsWhitespace".to_string());
        interner.id_map.insert("Range".to_string(), INTERNED_RANGE);
        interner.stored_strs.push("Range".to_string());
        interner
            .id_map
            .insert("StartsW".to_string(), INTERNED_STARTSW);
        interner.stored_strs.push("StartsW".to_string());
        interner.id_map.insert("EndsW".to_string(), INTERNED_ENDSW);
        interner.stored_strs.push("EndsW".to_string());
        interner
            .id_map
            .insert("Contains".to_string(), INTERNED_CONTAINS);
        interner.stored_strs.push("Contains".to_string());
        interner
            .id_map
            .insert("Equals".to_string(), INTERNED_EQUALS);
        interner.stored_strs.push("Equals".to_string());
        interner.id_map.insert("i8".to_string(), INTERNED_I8);
        interner.stored_strs.push("i8".to_string());
        interner.id_map.insert("u8".to_string(), INTERNED_U8);
        interner.stored_strs.push("u8".to_string());
        interner.id_map.insert("i16".to_string(), INTERNED_I16);
        interner.stored_strs.push("i16".to_string());
        interner.id_map.insert("u16".to_string(), INTERNED_U16);
        interner.stored_strs.push("u16".to_string());

        interner.id_map.insert("f16".to_string(), INTERNED_F16);
        interner.stored_strs.push("f16".to_string());

        interner.id_map.insert("i32".to_string(), INTERNED_I32);
        interner.stored_strs.push("i32".to_string());
        interner.id_map.insert("u32".to_string(), INTERNED_U32);
        interner.stored_strs.push("u32".to_string());
        interner.id_map.insert("f32".to_string(), INTERNED_F32);
        interner.stored_strs.push("f32".to_string());
        interner.id_map.insert("i64".to_string(), INTERNED_I64);
        interner.stored_strs.push("i64".to_string());
        interner.id_map.insert("u64".to_string(), INTERNED_U64);
        interner.stored_strs.push("u64".to_string());
        interner.id_map.insert("f64".to_string(), INTERNED_F64);
        interner.stored_strs.push("f64".to_string());
        interner.id_map.insert("i128".to_string(), INTERNED_I128);
        interner.stored_strs.push("i128".to_string());
        interner.id_map.insert("u128".to_string(), INTERNED_U128);
        interner.stored_strs.push("u128".to_string());
        interner.id_map.insert("f128".to_string(), INTERNED_F128);
        interner.stored_strs.push("f128".to_string());
        interner.id_map.insert("sized".to_string(), INTERNED_SIZED);
        interner.stored_strs.push("sized".to_string());
        interner
            .id_map
            .insert("unsized".to_string(), INTERNED_UNSIZED);
        interner.stored_strs.push("unsized".to_string());
        interner.id_map.insert("bool".to_string(), INTERNED_BOOL);
        interner.stored_strs.push("bool".to_string());
        interner.id_map.insert("nil".to_string(), INTERNED_NIL);
        interner.stored_strs.push("nil".to_string());
        interner.id_map.insert("char".to_string(), INTERNED_CHAR);
        interner.stored_strs.push("char".to_string());
        interner.id_map.insert("str".to_string(), INTERNED_STR);
        interner.stored_strs.push("str".to_string());
        interner
            .id_map
            .insert("BigInt".to_string(), INTERNED_BIGINT);
        interner.stored_strs.push("BigInt".to_string());
        interner
            .id_map
            .insert("BigFloat".to_string(), INTERNED_BIGFLOAT);
        interner.stored_strs.push("BigFloat".to_string());
        interner.id_map.insert("List".to_string(), INTERNED_LIST);
        interner.stored_strs.push("List".to_string());
        interner.id_map.insert("Set".to_string(), INTERNED_SET);
        interner.stored_strs.push("Set".to_string());
        interner.id_map.insert("Map".to_string(), INTERNED_MAP);
        interner.stored_strs.push("Map".to_string());
        interner.id_map.insert("Tuple".to_string(), INTERNED_TUPLE);
        interner.stored_strs.push("Tuple".to_string());
        interner
            .id_map
            .insert("Runtime".to_string(), INTERNED_RUNTIME);
        interner.stored_strs.push("Runtime".to_string());
        interner.id_map.insert("core".to_string(), INTERNED_CORE);
        interner.stored_strs.push("core".to_string());
        interner.id_map.insert("in".to_string(), INTERNED_IN);
        interner.stored_strs.push("in".to_string());
        interner
            .id_map
            .insert("Ranged".to_string(), INTERNED_RANGED);
        interner.stored_strs.push("Ranged".to_string());
        interner
            .id_map
            .insert("CharacterMappable".to_string(), INTERNED_CHARACTER_MAPPABLE);
        interner.stored_strs.push("CharacterMappable".to_string());
        interner
            .id_map
            .insert("Collection".to_string(), INTERNED_COLLECTION);
        interner.stored_strs.push("Collection".to_string());
        interner
            .id_map
            .insert("HasLen".to_string(), INTERNED_HAS_LEN);
        interner.stored_strs.push("HasLen".to_string());
        interner
            .id_map
            .insert("Integer".to_string(), INTERNED_INTEGER);
        interner.stored_strs.push("Integer".to_string());
        interner
            .id_map
            .insert("Numeric".to_string(), INTERNED_NUMERIC);
        interner.stored_strs.push("Numeric".to_string());
        interner
            .id_map
            .insert("SignedInteger".to_string(), INTERNED_SIGNED_INTEGER);
        interner.stored_strs.push("SignedInteger".to_string());
        interner
            .id_map
            .insert("UnsignedInteger".to_string(), INTERNED_UNSIGNED_INTEGER);
        interner.stored_strs.push("UnsignedInteger".to_string());
        interner.id_map.insert("Float".to_string(), INTERNED_FLOAT);
        interner.stored_strs.push("Float".to_string());
        interner
            .id_map
            .insert("Ordered".to_string(), INTERNED_ORDERED);
        interner.stored_strs.push("Ordered".to_string());
        interner
            .id_map
            .insert("Comparable".to_string(), INTERNED_COMPARABLE);
        interner.stored_strs.push("Comparable".to_string());
        interner
            .id_map
            .insert("JAVA".to_string(), INTERNED_JAVA_UPPER);
        interner.stored_strs.push("JAVA".to_string());
        interner
            .id_map
            .insert("default_value".to_string(), INTERNED_DEFAULT_VALUE);
        interner.stored_strs.push("default_value".to_string());
        interner.id_map.insert("warn".to_string(), INTERNED_WARN);
        interner.stored_strs.push("warn".to_string());
        interner
            .id_map
            .insert("ignore".to_string(), INTERNED_IGNORE);
        interner.stored_strs.push("ignore".to_string());
        interner
            .id_map
            .insert("scient".to_string(), INTERNED_SCIENT);
        interner.stored_strs.push("scient".to_string());
        interner.id_map.insert("hex".to_string(), INTERNED_HEX);
        interner.stored_strs.push("hex".to_string());
        interner.id_map.insert("bin".to_string(), INTERNED_BIN);
        interner.stored_strs.push("bin".to_string());
        interner.id_map.insert("octal".to_string(), INTERNED_OCTAL);
        interner.stored_strs.push("octal".to_string());

        interner.pos = interner.stored_strs.len();

        interner
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
