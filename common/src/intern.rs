use std::{collections::HashMap, path::Path};

// MAKE THE MACRO PLEASE
const INTRINSICS_ARRAY: [&str; 37] = [
    // primitives
    "i8", // 0
    "u8",
    "i16",
    "u16",
    "f16", // 4
    "i32",
    "u32", // 6
    "f32",
    "i64", // 8
    "u64",
    "f64", // 10
    "i128",
    "u128", // 12
    "f128",
    "sized", // 14
    "unsized",
    "char", // 16
    "str",
    "bool", // 18
    "nil",
    "BigInt", // 20
    "BigFloat",
    "List",
    "Map",
    "Set", // 24
    // structures
    "struct",
    "enum", // 26
    // Section names
    "bind",
    "var", // 28
    "nest",
    "complex", // 30
    // Directives
    "IsEmpty",
    "IsWhitespace", // 32
    "Range",
    "StartsW", // 34
    "EndsW",
    "Contains", // 36
];

pub struct Intern {
    map: HashMap<String, u32>,
    stored: Vec<String>,
    // Maybe not
    // stored_paths: Vec<OsString>
    pos: usize,
}

//TODO: CONCERNING INTRINSIC VALUES
impl Intern {
    pub fn init() -> Intern {
        let mut interner = Intern {
            map: HashMap::with_capacity(INTRINSICS_ARRAY.len()),
            stored: Vec::with_capacity(INTRINSICS_ARRAY.len()),
            pos: INTRINSICS_ARRAY.len(),
        };

        // TODO: Is this ok?
        for (id, keyword) in INTRINSICS_ARRAY.iter().enumerate() {
            interner.map.insert(keyword.to_string(), id as u32);
            interner.stored.push(keyword.to_string());
        }

        interner
    }

    pub fn intern(&mut self, s: &str) -> u32 {
        if let Some(id) = self.map.get(s) {
            return *id;
        }

        let id = self.pos as u32;
        self.pos += 1;

        let new_str = s.to_string();

        self.map.insert(new_str.clone(), id);
        self.stored.push(new_str);

        id
    }

    pub fn is_keyword(&self, id: usize) -> bool {
        id < INTRINSICS_ARRAY.len()
    }

    // Primitive being used loosely here...
    // SO IF I PUT ZERO, IT SCREAMS. BUT IF I USE ZERO UNDER A WRAPPER, ITS OK. RIGHT.
    pub fn is_primitive(&self, id: usize) -> bool {
        id >= usize::MIN && id <= 24
    }

    // TODO: Make unit test for this
    pub fn is_section(&self, id: u32) -> bool {
        if id >= 27 && id <= 30 {
            return true;
        }

        false
    }

    pub fn search(&self, index: usize) -> &str {
        &self.stored[index]
    }

    pub fn search_path(&self, index: usize) -> &Path {
        todo!()
    }
}
