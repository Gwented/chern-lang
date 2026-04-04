use std::{
    collections::HashMap,
    ffi::OsStr,
    path::{Path, PathBuf},
};

use crate::keywords;

// MAKE THE MACRO PLEASE
// What macro. What is a macro? What is hygiene?

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

impl Intern {
    pub fn init() -> Intern {
        let mut interner = Intern {
            id_map: HashMap::with_capacity(keywords::KEYWORDS_ARRAY.len()),
            stored_strs: Vec::with_capacity(keywords::KEYWORDS_ARRAY.len()),
            path_map: HashMap::new(),
            stored_paths: Vec::new(),
            pos: keywords::KEYWORDS_ARRAY.len(),
        };

        for (id, keyword) in keywords::KEYWORDS_ARRAY.iter().enumerate() {
            interner.id_map.insert(keyword.to_string(), id as u32);
            interner.stored_strs.push(keyword.to_string());
        }

        interner
    }

    pub fn intern(&mut self, s: &str) -> u32 {
        if let Some(id) = self.id_map.get(s) {
            return *id;
        }

        let id = self.stored_strs.len() as u32;
        self.pos += 1;

        let new_str = s.to_string();

        self.id_map.insert(new_str.clone(), id);
        self.stored_strs.push(new_str);

        id
    }

    //
    pub fn intern_path(&mut self, s: &Path) -> u32 {
        if let Some(id) = self.path_map.get(s) {
            return *id;
        }

        let id = self.stored_paths.len() as u32;
        self.pos += 1;

        let new_path = s.to_path_buf();

        self.path_map.insert(new_path.clone(), id);
        self.stored_paths.push(new_path);

        id
    }

    pub fn search(&self, index: usize) -> &str {
        &self.stored_strs[index]
    }

    pub fn search_path(&self, index: usize) -> &Path {
        &self.stored_paths[index]
    }
}
