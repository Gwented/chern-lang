use std::env;
use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};

use super::helpers::*;
use crate::config_loader::ConfigLoader;
use crate::modules::mod_finder::ModuleFinder;
use crate::modules::{ImportKind, Module, ModuleState, extract_all_modules, extract_main};
use crate::script_compiler::reporter::Reporter;
use chrn_utils::{
    chrn_config::ChrnConfig,
    id_types::{InternedId, ModuleId, PathId, SourceRegionId},
    intern::Intern,
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Creates a unique temporary directory for a test. The caller should clean up
/// with `remove_dir_all` when done.
fn create_temp_dir(label: &str) -> PathBuf {
    let dir = env::temp_dir().join(format!("chrn_modules_test_{}", label));
    _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("failed to create temp dir");
    dir
}

/// Writes `content` to `path`, creating parent directories as needed.
fn write_file(path: &Path, content: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("failed to create parent dirs");
    }
    fs::write(path, content).expect("failed to write file");
}

/// Creates a `.chrn` file and returns its canonical path.
fn create_chrn_file(dir: &Path, name: &str, content: &str) -> PathBuf {
    let path = dir.join(name);
    write_file(&path, content);
    fs::canonicalize(&path).expect("failed to canonicalize path")
}

/// Returns the module at the given index from a compiler, or panics if out of
/// range.  Index 0 is the first user module; the core module is always last.
fn get_user_mod_count(compiler: &crate::script_compiler::ScriptCompiler) -> usize {
    // All modules except the implicit core module at the end
    compiler.mods.len().saturating_sub(1)
}

/// Finds a module in the compiler by name.  Returns `None` if not found.
fn find_module_by_name<'a>(
    compiler: &'a crate::script_compiler::ScriptCompiler,
    interner: &Intern,
    name: &str,
) -> Option<&'a Module> {
    let name_id = interner.try_search_str(name)?;
    for i in 0..compiler.mods.len() {
        let m = &compiler.mods[ModuleId::new(i as u32)];
        if m.name_id == name_id {
            return Some(m);
        }
    }
    None
}

/// Creates a simple script file with no imports.  Returns its canonical path.
fn make_simple_script(dir: &Path) -> PathBuf {
    create_chrn_file(dir, "main.chrn", "let x = 5\nlet y = 10\n")
}

/// Creates a script file that imports a sub-module. `sub_path` must be the
/// *canonical* absolute path to the sub file (import statements use absolute
/// paths so the working directory is irrelevant).
fn make_importing_script(dir: &Path, sub_canonical: &Path) -> PathBuf {
    let import_stmt = format!("import \"{}\"\n", sub_canonical.display());
    create_chrn_file(dir, "main.chrn", &format!("{}let x = 5\n", import_stmt))
}

/// Creates a sub-module script with simple content.
fn make_sub_script(dir: &Path) -> PathBuf {
    create_chrn_file(dir, "sub.chrn", "let y = 42\n")
}

/// Creates a file with `@def` / `@end` boundaries.
fn make_def_script(dir: &Path) -> PathBuf {
    create_chrn_file(dir, "def_test.chrn", "serial_data\n@def\nlet x = 1\n@end\n")
}

/// Creates a file with a broken `@def` (no matching `@end`).
fn make_broken_script(dir: &Path) -> PathBuf {
    create_chrn_file(dir, "broken.chrn", "@def\nlet x = 1\n")
}

/// Creates a file with a `bind` statement pointing to a real file.
fn make_bind_script(dir: &Path, bind_target: &Path) -> PathBuf {
    let bind_stmt = format!("bind \"{}\"\n", bind_target.display());
    create_chrn_file(dir, "with_bind.chrn", &format!("{}let x = 5\n", bind_stmt))
}

fn mod_id_for_path(reserved: &[(PathId, ModuleId)], path_id: PathId) -> Option<ModuleId> {
    reserved
        .iter()
        .find(|(p, _)| *p == path_id)
        .map(|(_, m)| *m)
}

// ===========================================================================
// ModuleFinder unit tests
// ===========================================================================

/// `ModuleFinder` should return zero imports and no bind when the src text
/// contains neither.
#[test]
fn modfinder_no_imports_or_bind() {
    let mut interner = Intern::init();
    let cfg = ChrnConfig::default();
    let path_id = interner.intern_path(Path::new("dummy.chrn"));
    let region_id = SourceRegionId::new(0);

    let region = ConfigLoader::new(
        region_id,
        Cursor::new("let x = 5\nlet y = 10\n"),
        path_id,
        &cfg,
        &interner,
    )
    .load_config()
    .expect_success();

    let mut seen: Vec<(PathId, ModuleId)> = vec![(path_id, ModuleId::new(0))];
    let (bind, imports, diags) = ModuleFinder::new(
        &region.src_bytes,
        &cfg,
        &mut seen,
        &region,
        region.script_start,
        region.serial_start,
    )
    .collect_imports(&mut interner);

    assert!(bind.is_none(), "no bind expected, got {:?}", bind);
    assert!(
        imports.is_empty(),
        "no imports expected, got {}",
        imports.len()
    );
    assert!(diags.is_empty(), "no diagnostics expected, got {:?}", diags);
    // `seen` should have grown no new entries beyond the initial main module
    assert_eq!(seen.len(), 1, "seen must not gain entries with no imports");
}

/// `ModuleFinder` should find a single import with a valid path.
#[test]
fn modfinder_single_import() {
    let dir = create_temp_dir("modfinder_single_import");
    let sub_path = make_sub_script(&dir);
    let sub_canonical = sub_path.to_string_lossy();

    let mut interner = Intern::init();
    let cfg = ChrnConfig::default();
    let path_id = interner.intern_path(Path::new("main.chrn"));
    let region_id = SourceRegionId::new(0);

    let src = format!("import \"{}\"\nlet x = 5\n", sub_canonical);
    let region = ConfigLoader::new(region_id, src.as_bytes(), path_id, &cfg, &interner)
        .load_config()
        .expect_success();

    let mut seen: Vec<(PathId, ModuleId)> = vec![(path_id, ModuleId::new(0))];
    let (bind, imports, diags) = ModuleFinder::new(
        &region.src_bytes,
        &cfg,
        &mut seen,
        &region,
        region.script_start,
        region.serial_start,
    )
    .collect_imports(&mut interner);

    assert!(bind.is_none(), "no bind expected");
    assert!(diags.is_empty(), "no diagnostics expected, got {:?}", diags);
    assert_eq!(imports.len(), 1, "exactly one import expected");

    let import = &imports[0];
    let sub_path_id = interner.intern_path(&sub_path);
    let expected_mod_id = ModuleId::new(1);
    assert_eq!(import.mod_id, expected_mod_id, "sub gets module id 1");
    // Verify import kind carries the correct path id and span.
    // Source is `import "PATH"\nlet x = 5\n`; module finder's `parse_import` starts
    // at the opening `"` (byte 7), advances past it, so the span covers the path
    // without quotes:  start = 8,  end = 8 + path.len().
    let import_span = match &import.kind {
        ImportKind::Source(sp_path_id) => {
            assert_eq!(
                sp_path_id.inner, sub_path_id,
                "import must carry the correct sub path id"
            );
            let expected_start = 8;
            let expected_end = expected_start + sub_canonical.len() as u32;
            assert_eq!(
                sp_path_id.span.start, expected_start,
                "import span must start at byte 8 (right after opening \"), got {}",
                sp_path_id.span.start
            );
            assert_eq!(
                sp_path_id.span.end,
                expected_end,
                "import span must end at byte {} (right after path of length {}), got {}",
                expected_end,
                sub_canonical.len(),
                sp_path_id.span.end
            );
            sp_path_id.span
        }
        ImportKind::Core => panic!("expected Source import kind, got Core"),
    };
    // Verify the span slices out the exact path from src bytes
    let span_bytes = &region.src_bytes[import_span.start as usize..import_span.end as usize];
    let span_str = String::from_utf8_lossy(span_bytes);
    assert_eq!(
        span_str, sub_canonical,
        "import span must extract the exact path string from src bytes"
    );

    // The `seen` vector must contain both the main and the sub module
    assert_eq!(seen.len(), 2, "seen must contain main + sub entry");
    assert!(
        seen.iter().any(|(p, _)| *p == path_id),
        "seen must contain main path"
    );
    assert!(
        seen.iter().any(|(p, _)| *p == sub_path_id),
        "seen must contain sub path"
    );

    _ = fs::remove_dir_all(&dir);
}

/// `ModuleFinder` should find two distinct imports.
#[test]
fn modfinder_multiple_imports() {
    let dir = create_temp_dir("modfinder_multi_import");
    let sub1_path = make_sub_script(&dir);
    let sub2_path = create_chrn_file(&dir, "another.chrn", "let z = 99\n");

    let sub1_str = sub1_path.to_string_lossy();
    let sub2_str = sub2_path.to_string_lossy();

    let mut interner = Intern::init();
    let cfg = ChrnConfig::default();
    let path_id = interner.intern_path(Path::new("main.chrn"));
    let region_id = SourceRegionId::new(0);

    let src = format!(
        "import \"{}\"\nimport \"{}\"\nlet x = 5\n",
        sub1_str, sub2_str
    );
    let region = ConfigLoader::new(region_id, src.as_bytes(), path_id, &cfg, &interner)
        .load_config()
        .expect_success();

    let mut seen: Vec<(PathId, ModuleId)> = vec![(path_id, ModuleId::new(0))];
    let (bind, imports, diags) = ModuleFinder::new(
        &region.src_bytes,
        &cfg,
        &mut seen,
        &region,
        region.script_start,
        region.serial_start,
    )
    .collect_imports(&mut interner);

    assert!(bind.is_none());
    assert!(diags.is_empty(), "no diagnostics expected, got {:?}", diags);
    assert_eq!(imports.len(), 2, "exactly two imports expected");
    assert_eq!(seen.len(), 3, "main + 2 subs in seen");

    // Each import should have a distinct mod_id
    assert_ne!(
        imports[0].mod_id, imports[1].mod_id,
        "each import gets a unique mod_id"
    );

    _ = fs::remove_dir_all(&dir);
}

/// `ModuleFinder` should capture the `as` alias when present.
#[test]
fn modfinder_import_with_alias() {
    let dir = create_temp_dir("modfinder_alias");
    let sub_path = make_sub_script(&dir);
    let sub_canonical = sub_path.to_string_lossy();

    let mut interner = Intern::init();
    let cfg = ChrnConfig::default();
    let path_id = interner.intern_path(Path::new("main.chrn"));
    let region_id = SourceRegionId::new(0);

    let src = format!("import \"{}\" as my_mod\nlet x = 5\n", sub_canonical);
    let region = ConfigLoader::new(region_id, src.as_bytes(), path_id, &cfg, &interner)
        .load_config()
        .expect_success();

    let mut seen: Vec<(PathId, ModuleId)> = vec![(path_id, ModuleId::new(0))];
    let (_, imports, diags) = ModuleFinder::new(
        &region.src_bytes,
        &cfg,
        &mut seen,
        &region,
        region.script_start,
        region.serial_start,
    )
    .collect_imports(&mut interner);

    assert!(diags.is_empty(), "no diagnostics expected, got {:?}", diags);
    assert_eq!(imports.len(), 1);
    let import = &imports[0];
    assert!(import.alias_id.is_some(), "import should have an alias_id");
    let alias_name = interner.search(import.alias_id.unwrap());
    assert_eq!(alias_name, "my_mod", "alias should be 'my_mod'");

    _ = fs::remove_dir_all(&dir);
}

/// `ModuleFinder` should find a `bind` statement.
#[test]
fn modfinder_bind() {
    let dir = create_temp_dir("modfinder_bind");
    let bind_target = create_chrn_file(&dir, "target.bin", "not actually a binary\n");
    let bind_canonical = bind_target.to_string_lossy();

    let mut interner = Intern::init();
    let cfg = ChrnConfig::default();
    let path_id = interner.intern_path(Path::new("main.chrn"));
    let region_id = SourceRegionId::new(0);

    let src = format!("bind \"{}\"\nlet x = 5\n", bind_canonical);
    let region = ConfigLoader::new(region_id, src.as_bytes(), path_id, &cfg, &interner)
        .load_config()
        .expect_success();

    let mut seen: Vec<(PathId, ModuleId)> = vec![(path_id, ModuleId::new(0))];
    let (bind, imports, diags) = ModuleFinder::new(
        &region.src_bytes,
        &cfg,
        &mut seen,
        &region,
        region.script_start,
        region.serial_start,
    )
    .collect_imports(&mut interner);

    assert!(diags.is_empty(), "no diagnostics expected");
    assert!(imports.is_empty(), "no imports expected");
    assert!(bind.is_some(), "bind should be present");
    assert_eq!(
        bind.as_ref().unwrap().path_id,
        interner.intern_path(&bind_target),
        "bind path must match target"
    );

    _ = fs::remove_dir_all(&dir);
}

/// A backslash in an import path must produce an error diagnostic (only '/' is
/// allowed as a path separator).
#[test]
fn modfinder_backslash_error() {
    let dir = create_temp_dir("modfinder_backslash");
    let mut interner = Intern::init();
    let cfg = ChrnConfig::default();
    let path_id = interner.intern_path(Path::new("main.chrn"));
    let region_id = SourceRegionId::new(0);

    // Use a backslash in the import path — even though the real file exists,
    // the backslash triggers the error *before* canonicalization is attempted.
    let src = r#"import "bad\path" "#;
    let region = ConfigLoader::new(region_id, src.as_bytes(), path_id, &cfg, &interner)
        .load_config()
        .expect_success();

    let mut seen: Vec<(PathId, ModuleId)> = vec![(path_id, ModuleId::new(0))];
    let (_bind, imports, diags) = ModuleFinder::new(
        &region.src_bytes,
        &cfg,
        &mut seen,
        &region,
        region.script_start,
        region.serial_start,
    )
    .collect_imports(&mut interner);

    assert!(
        imports.is_empty(),
        "no valid import must be produced for a backslash path"
    );
    assert!(!diags.is_empty(), "backslash should produce a diagnostic");
    let has_backslash_msg = diags.iter().any(|d| d.core_msg.contains("'/'"));
    assert!(
        has_backslash_msg,
        "diagnostic should mention that only '/' is allowed"
    );

    _ = fs::remove_dir_all(&dir);
}

/// An import written inside a line comment must be ignored.
#[test]
fn modfinder_import_inside_line_comment() {
    let mut interner = Intern::init();
    let cfg = ChrnConfig::default();
    let path_id = interner.intern_path(Path::new("dummy.chrn"));
    let region_id = SourceRegionId::new(0);

    let src = Cursor::new(b"// import \"sub.chrn\"\nlet x = 5\n");
    let region = ConfigLoader::new(region_id, src, path_id, &cfg, &interner)
        .load_config()
        .expect_success();

    let mut seen: Vec<(PathId, ModuleId)> = vec![(path_id, ModuleId::new(0))];
    let (_bind, imports, diags) = ModuleFinder::new(
        &region.src_bytes,
        &cfg,
        &mut seen,
        &region,
        region.script_start,
        region.serial_start,
    )
    .collect_imports(&mut interner);

    assert!(
        imports.is_empty(),
        "import inside line comment must be ignored"
    );
    assert!(diags.is_empty(), "no diagnostics expected");
}

/// An import written inside a multi-line comment must be ignored.
#[test]
fn modfinder_import_inside_block_comment() {
    let mut interner = Intern::init();
    let cfg = ChrnConfig::default();
    let path_id = interner.intern_path(Path::new("dummy.chrn"));
    let region_id = SourceRegionId::new(0);

    let src = Cursor::new(b"/* import \"sub.chrn\" */\nlet x = 5\n");
    let region = ConfigLoader::new(region_id, src, path_id, &cfg, &interner)
        .load_config()
        .expect_success();

    let mut seen: Vec<(PathId, ModuleId)> = vec![(path_id, ModuleId::new(0))];
    let (_bind, imports, diags) = ModuleFinder::new(
        &region.src_bytes,
        &cfg,
        &mut seen,
        &region,
        region.script_start,
        region.serial_start,
    )
    .collect_imports(&mut interner);

    assert!(
        imports.is_empty(),
        "import inside block comment must be ignored"
    );
    assert!(diags.is_empty(), "no diagnostics expected");
}

/// Duplicate imports to the same path should reuse the same module id via the
/// `seen` vector.
#[test]
fn modfinder_duplicate_import_path_reuses_mod_id() {
    let dir = create_temp_dir("modfinder_dup");
    let sub_path = make_sub_script(&dir);
    let sub_canonical = sub_path.to_string_lossy();

    let mut interner = Intern::init();
    let cfg = ChrnConfig::default();
    let path_id = interner.intern_path(Path::new("main.chrn"));
    let region_id = SourceRegionId::new(0);

    // Two imports to the same path
    let src = format!(
        "import \"{}\"\nimport \"{}\"\nlet x = 5\n",
        sub_canonical, sub_canonical
    );
    let region = ConfigLoader::new(region_id, src.as_bytes(), path_id, &cfg, &interner)
        .load_config()
        .expect_success();

    let mut seen: Vec<(PathId, ModuleId)> = vec![(path_id, ModuleId::new(0))];
    let (_bind, imports, diags) = ModuleFinder::new(
        &region.src_bytes,
        &cfg,
        &mut seen,
        &region,
        region.script_start,
        region.serial_start,
    )
    .collect_imports(&mut interner);

    assert!(diags.is_empty(), "no diagnostics expected, got {:?}", diags);
    assert_eq!(imports.len(), 2, "both imports should be recorded");

    // Both imports must reference the same module id
    assert_eq!(
        imports[0].mod_id, imports[1].mod_id,
        "duplicate path imports must share the same mod_id"
    );
    // `seen` must have only two entries (main + sub)
    assert_eq!(seen.len(), 2, "seen must not grow beyond main + sub");

    _ = fs::remove_dir_all(&dir);
}

/// Non-existent paths in an import statement must produce a diagnostic and the
/// import must be dropped.
#[test]
fn modfinder_nonexistent_path() {
    let mut interner = Intern::init();
    let cfg = ChrnConfig::default();
    let path_id = interner.intern_path(Path::new("dummy.chrn"));
    let region_id = SourceRegionId::new(0);

    let src = Cursor::new(b"import \"/does/not/exist.chrn\"\nlet x = 5\n");
    let region = ConfigLoader::new(region_id, src, path_id, &cfg, &interner)
        .load_config()
        .expect_success();

    let mut seen: Vec<(PathId, ModuleId)> = vec![(path_id, ModuleId::new(0))];
    let (_bind, imports, diags) = ModuleFinder::new(
        &region.src_bytes,
        &cfg,
        &mut seen,
        &region,
        region.script_start,
        region.serial_start,
    )
    .collect_imports(&mut interner);

    assert!(
        imports.is_empty(),
        "import for non-existent path must be dropped"
    );
    assert!(
        !diags.is_empty(),
        "non-existent path must produce a diagnostic"
    );
}

/// `ModuleFinder` should handle a string inside the src that contains
/// something that looks like `import` but is inside quotes.
#[test]
fn modfinder_import_inside_string_is_not_parsed() {
    let mut interner = Intern::init();
    let cfg = ChrnConfig::default();
    let path_id = interner.intern_path(Path::new("dummy.chrn"));
    let region_id = SourceRegionId::new(0);

    let src = Cursor::new(b"\"this has import inside a string\" let x = 5\n");
    let region = ConfigLoader::new(region_id, src, path_id, &cfg, &interner)
        .load_config()
        .expect_success();

    let mut seen: Vec<(PathId, ModuleId)> = vec![(path_id, ModuleId::new(0))];
    let (_bind, imports, diags) = ModuleFinder::new(
        &region.src_bytes,
        &cfg,
        &mut seen,
        &region,
        region.script_start,
        region.serial_start,
    )
    .collect_imports(&mut interner);

    assert!(
        imports.is_empty(),
        "'import' inside a string must not be parsed as an import"
    );
    assert!(diags.is_empty(), "no diagnostics expected");
}

// ===========================================================================
// extract_main tests
// ===========================================================================

/// `extract_main` on a simple script file with no imports must return a
/// `Module` with `Loaded` state, no imports, no bind, and a correctly
/// initialised `ModuleGraph`.
#[test]
fn extract_main_simple_script() {
    let dir = create_temp_dir("extract_main_simple");
    let main_path = make_simple_script(&dir);
    let cfg = ChrnConfig::default();

    let (main_mod, graph, mut interner, diags) =
        extract_main(&main_path, &cfg).expect("extract_main must succeed");

    // ---- Module assertions ----
    assert_eq!(
        main_mod.state,
        ModuleState::Loaded,
        "main module should be Loaded"
    );
    assert_eq!(main_mod.mod_id, ModuleId::new(0), "main gets mod_id 0");
    let main_name = interner.search(main_mod.name_id);
    assert_eq!(
        main_name, "main",
        "module name should be 'main' (file stem)"
    );
    assert!(
        main_mod.imports.is_empty(),
        "no imports expected in simple script"
    );
    assert!(main_mod.bind.is_none(), "no bind expected in simple script");
    assert!(
        main_mod.region_id.is_some(),
        "main module should have a region_id"
    );

    // ---- Graph assertions ----
    // reserved_mod_ids must contain main's path -> mod_id 0
    let main_path_id = interner.intern_path(&main_path);
    let main_entry = mod_id_for_path(graph.reserved_mod_ids(), main_path_id);
    assert_eq!(main_entry, Some(ModuleId::new(0)));
    assert_eq!(graph.reserved_mod_ids().len(), 1, "only main in reserved");
    // `seen` must contain main's path id
    assert_eq!(graph.seen().len(), 1, "only main in seen");
    assert!(graph.seen().contains(&main_path_id));
    // `other_mods` starts empty
    // assert!(graph.other_mods().is_empty(), "other_mods should be empty");
    // region_arena must have one entry
    assert_eq!(graph.region_arena().len(), 1, "one region pushed");

    assert!(diags.is_empty(), "no diagnostics expected");

    _ = fs::remove_dir_all(&dir);
}

/// `extract_main` on a file with a valid import should attach the import to
/// the main module and register the sub-path in `reserved_mod_ids`.
#[test]
fn extract_main_with_import() {
    let dir = create_temp_dir("extract_main_import");
    let sub_path = make_sub_script(&dir);
    let main_path = make_importing_script(&dir, &sub_path);
    let cfg = ChrnConfig::default();

    let (main_mod, graph, mut interner, diags) =
        extract_main(&main_path, &cfg).expect("extract_main must succeed");

    assert_eq!(main_mod.state, ModuleState::Loaded);
    // Wait for import - there should be one import found
    assert_eq!(
        main_mod.imports.len(),
        1,
        "main should have 1 import from the sub module"
    );

    // The import should have the correct module id (1)
    let import = &main_mod.imports[0];
    assert_eq!(import.mod_id, ModuleId::new(1), "sub gets mod_id 1");
    let sub_path_id = interner.intern_path(&sub_path);
    let sub_path_str = sub_path.to_string_lossy();
    // Verify import kind and extract span for content check.
    // Source is `import "PATH"\nlet x = 5\n`; span covers path without quotes:
    // start = 8, end = 8 + path.len().
    let import_span = match &import.kind {
        ImportKind::Source(sp_path_id) => {
            assert_eq!(
                sp_path_id.inner, sub_path_id,
                "import must carry the correct sub path id"
            );
            let expected_start = 8;
            let expected_end = expected_start + sub_path_str.len() as u32;
            assert_eq!(
                sp_path_id.span.start, expected_start,
                "import span must start at byte 8 (right after opening \"), got {}",
                sp_path_id.span.start
            );
            assert_eq!(
                sp_path_id.span.end,
                expected_end,
                "import span must end at byte {} (after path of length {}), got {}",
                expected_end,
                sub_path_str.len(),
                sp_path_id.span.end
            );
            sp_path_id.span
        }
        ImportKind::Core => panic!("expected Source import kind, got Core"),
    };
    // Verify the span slices out the exact path from src bytes
    let region = &graph.region_arena()[main_mod.region_id.expect("main mod has region_id")];
    let span_bytes = &region.src_bytes[import_span.start as usize..import_span.end as usize];
    let span_str = String::from_utf8_lossy(span_bytes);
    assert_eq!(
        span_str, sub_path_str,
        "import span must extract the exact path string from src bytes"
    );

    // reserved_mod_ids must have entries for both main and sub
    assert_eq!(graph.reserved_mod_ids().len(), 2, "main + sub in reserved");
    let main_path_id = interner.intern_path(&main_path);
    assert!(
        graph
            .reserved_mod_ids()
            .iter()
            .any(|(p, _)| *p == main_path_id),
        "main path must be in reserved"
    );
    assert!(
        graph
            .reserved_mod_ids()
            .iter()
            .any(|(p, _)| *p == sub_path_id),
        "sub path must be in reserved"
    );

    // `seen` must contain main
    assert!(graph.seen().contains(&main_path_id));
    assert_eq!(
        graph.seen().len(),
        1,
        "only main in seen (sub not visited yet)"
    );

    assert!(diags.is_empty(), "no diagnostics expected, got {:?}", diags);

    _ = fs::remove_dir_all(&dir);
}

/// `extract_main` on a file with a broken `@def` (no `@end`) must produce a
/// module with `ModuleState::BrokenRegion` and a diagnostic.
#[test]
fn extract_main_broken_config() {
    let dir = create_temp_dir("extract_main_broken");
    let main_path = make_broken_script(&dir);
    let cfg = ChrnConfig::default();

    let (main_mod, graph, _interner, diags) = extract_main(&main_path, &cfg)
        .expect("extract_main should still return Ok with BrokenRegion");

    assert_eq!(
        main_mod.state,
        ModuleState::BrokenRegion,
        "broken config should result in BrokenRegion state"
    );
    assert_eq!(main_mod.mod_id, ModuleId::new(0));
    assert!(
        main_mod.region_id.is_some(),
        "even a broken region gets a region_id"
    );
    // The region was pushed even for broken config
    assert_eq!(
        graph.region_arena().len(),
        1,
        "broken region still pushed into arena"
    );

    // There should be at least one diagnostic about the missing @end
    assert!(!diags.is_empty(), "broken config must produce diagnostics");
    let has_missing_end = diags.iter().any(|d| d.core_msg.contains("@end"));
    assert!(has_missing_end, "diagnostic should mention missing @end");

    _ = fs::remove_dir_all(&dir);
}

/// `extract_main` on a non-existent path should return `Err(ModuleInitError)`.
#[test]
fn extract_main_missing_file() {
    let dir = create_temp_dir("extract_main_missing");
    let non_existent = dir.join("does_not_exist.chrn");
    let cfg = ChrnConfig::default();

    let result = extract_main(&non_existent, &cfg);
    assert!(
        result.is_err(),
        "non-existent file should produce ModuleInitError"
    );

    _ = fs::remove_dir_all(&dir);
}

/// `extract_main` on a file with a `bind` statement must capture the bind in
/// the returned module.
#[test]
fn extract_main_with_bind() {
    let dir = create_temp_dir("extract_main_bind");
    let bind_target = create_chrn_file(&dir, "bind_target.bin", "content\n");
    let main_path = make_bind_script(&dir, &bind_target);
    let cfg = ChrnConfig::default();

    let (main_mod, _graph, mut interner, diags) =
        extract_main(&main_path, &cfg).expect("extract_main must succeed");

    assert_eq!(main_mod.state, ModuleState::Loaded);
    assert!(
        main_mod.bind.is_some(),
        "bind should be present in the module"
    );
    let bind = main_mod.bind.as_ref().unwrap();
    let expected_path_id = interner.intern_path(&bind_target);
    assert_eq!(
        bind.path_id, expected_path_id,
        "bind path must match the target file"
    );

    assert!(diags.is_empty(), "no diagnostics expected");

    // Also verify that bind is the only thing found (no imports)
    assert!(
        main_mod.imports.is_empty(),
        "no imports expected in bind-only script"
    );

    _ = fs::remove_dir_all(&dir);
}

/// `extract_main` on a file with an `@def`/`@end` block must set the region's
/// script_start and serial_start correctly.
#[test]
fn extract_main_with_at_def_block() {
    let dir = create_temp_dir("extract_main_def");
    let main_path = make_def_script(&dir);
    let cfg = ChrnConfig::default();

    let (main_mod, graph, _interner, diags) =
        extract_main(&main_path, &cfg).expect("extract_main must succeed");

    assert_eq!(main_mod.state, ModuleState::Loaded);

    // The region for main should have the correct script and serial offsets.
    // File content from `make_def_script`:  "serial_data\n@def\nlet x = 1\n@end\n"
    //   bytes 0-11  = "serial_data\n"  (12 bytes of serial data)
    //   bytes 12-16 = "@def\n"         (5 bytes)
    //   bytes 17-26 = "let x = 1\n"    (10 bytes of script)
    //   bytes 27-30 = "@end"           (4 bytes, the \n at byte 31 is trailing)
    // So: script_start = 12 (offset of @def), serial_start = 31 (offset after @end).
    let region = &graph.region_arena()[main_mod.region_id.unwrap()];
    assert_eq!(
        region.script_start, 12,
        "script_start should be at the @def directive (byte 12), got {}",
        region.script_start
    );
    assert_eq!(
        region.serial_start,
        Some(31),
        "serial_start should be after @end (byte 31), got {:?}",
        region.serial_start
    );

    assert!(diags.is_empty(), "no diagnostics expected");

    // Verify the region bytes contain @def and @end
    let src_str = String::from_utf8_lossy(&region.src_bytes);
    assert!(src_str.contains("@def"), "region bytes must contain @def");
    assert!(src_str.contains("@end"), "region bytes must contain @end");

    _ = fs::remove_dir_all(&dir);
}

// ===========================================================================
// extract_all_modules tests  (full pipeline)
// ===========================================================================

/// `extract_all_modules` with a single module (no imports) should produce a
/// `ScriptCompiler` containing exactly one user module plus the core module.
#[test]
fn extract_all_modules_single() {
    let dir = create_temp_dir("extract_all_single");
    let main_path = make_simple_script(&dir);
    let cfg = ChrnConfig::default();
    let mut reporter = Reporter::new(100);

    let (compiler, _store, diags) = extract_all_modules(&main_path, cfg, &mut reporter)
        .expect("extract_all_modules must succeed");

    // Number of user modules = total mods - 1 (core module is injected last)
    let user_count = get_user_mod_count(&compiler);
    assert_eq!(user_count, 1, "expected 1 user module");

    // The single user module must be loaded
    let main_mod = &compiler.mods[ModuleId::new(0)];
    assert_eq!(
        main_mod.state,
        ModuleState::Loaded,
        "single module should be Loaded"
    );

    // Core module must exist
    let core_mod_id = ModuleId::new(compiler.mods.len() as u32 - 1);
    let core_mod = &compiler.mods[core_mod_id];
    assert_eq!(
        core_mod.name_id,
        InternedId::new(chrn_utils::intern::INTERNED_CORE),
        "last module must be core"
    );

    assert!(diags.is_empty(), "no diagnostics expected, got {:?}", diags);

    _ = fs::remove_dir_all(&dir);
}

/// `extract_all_modules` with an importing main module must resolve the sub
/// module and include it in the compiler.
#[test]
fn extract_all_modules_with_sub_module() {
    let dir = create_temp_dir("extract_all_sub");
    let sub_path = make_sub_script(&dir);
    let main_path = make_importing_script(&dir, &sub_path);
    let cfg = ChrnConfig::default();
    let mut reporter = Reporter::new(100);

    let (compiler, store, diags) = extract_all_modules(&main_path, cfg, &mut reporter)
        .expect("extract_all_modules must succeed");

    let user_count = get_user_mod_count(&compiler);
    assert_eq!(user_count, 2, "expected 2 user modules (main + sub)");

    // Find the sub module by name
    let interner = &store.interner;
    let sub_mod = find_module_by_name(&compiler, interner, "sub")
        .expect("sub module must be present in compiler");
    assert_eq!(
        sub_mod.state,
        ModuleState::Loaded,
        "sub module must be Loaded"
    );
    assert!(sub_mod.region_id.is_some(), "sub module must have a region");

    // Main module must still be Loaded
    let main_mod =
        find_module_by_name(&compiler, interner, "main").expect("main module must be present");
    assert_eq!(main_mod.state, ModuleState::Loaded);

    assert!(diags.is_empty(), "no diagnostics expected, got {:?}", diags);

    _ = fs::remove_dir_all(&dir);
}

/// When a sub-module has a broken config, the pipeline must still complete but
/// the sub-module should have `BrokenRegion` state and diagnostics should be
/// emitted.
#[test]
fn extract_all_modules_submodule_broken() {
    let dir = create_temp_dir("extract_all_broken_sub");
    let sub_path = make_broken_script(&dir);
    let main_path = make_importing_script(&dir, &sub_path);
    let cfg = ChrnConfig::default();
    let mut reporter = Reporter::new(100);

    let (compiler, store, diags) = extract_all_modules(&main_path, cfg, &mut reporter)
        .expect("extract_all_modules must succeed");

    let interner = &store.interner;

    // Main module stays Loaded
    let main_mod =
        find_module_by_name(&compiler, interner, "main").expect("main module must be present");
    assert_eq!(
        main_mod.state,
        ModuleState::Loaded,
        "main must remain Loaded when sub is broken"
    );

    // Sub module must be BrokenRegion
    let sub_mod = find_module_by_name(&compiler, interner, "broken")
        .expect("sub (broken) module must be present");
    assert_eq!(
        sub_mod.state,
        ModuleState::BrokenRegion,
        "sub module with broken config must be BrokenRegion"
    );

    // Diagnostics must be emitted (at least the missing @end diagnostic)
    assert!(
        !diags.is_empty(),
        "broken sub-module must produce diagnostics"
    );
    let has_end_diag = diags
        .iter()
        .any(|d| d.core_msg.contains("@end") || d.core_msg.contains("broken"));
    assert!(
        has_end_diag,
        "at least one diagnostic should mention @end or 'broken'"
    );

    _ = fs::remove_dir_all(&dir);
}

/// When the main module imports a file that does not exist, the import should
/// be dropped and a diagnostic emitted, but `extract_all_modules` should still
/// return Ok with the main module.
#[test]
fn extract_all_modules_import_to_nonexistent() {
    let dir = create_temp_dir("extract_all_nonexistent_import");
    let main_path = create_chrn_file(
        &dir,
        "main.chrn",
        "import \"/definitely/does/not/exist.chrn\"\nlet x = 5\n",
    );
    let cfg = ChrnConfig::default();
    let mut reporter = Reporter::new(100);

    let (compiler, store, diags) = extract_all_modules(&main_path, cfg, &mut reporter)
        .expect("extract_all_modules must succeed even with bad import");

    // Main module should still be present
    let interner = &store.interner;
    let main_mod =
        find_module_by_name(&compiler, interner, "main").expect("main module must be present");
    assert_eq!(
        main_mod.state,
        ModuleState::Loaded,
        "main module should be Loaded despite bad import"
    );

    // The main module may or may not have an import entry (ModuleFinder
    // drops imports with non-existent paths, so the import is absent).
    // Either way, only 1 user module should exist.
    assert_eq!(
        get_user_mod_count(&compiler),
        1,
        "only main module should exist"
    );

    // Diagnostics must be emitted about the missing import
    assert!(
        !diags.is_empty(),
        "non-existent import must produce diagnostics"
    );

    _ = fs::remove_dir_all(&dir);
}

/// When two imports point to the same sub-module path, `extract_all_modules`
/// must produce only one sub-module in the compiler (deduplicated by the
/// `reserved_mod_ids` registration mechanism).
#[test]
fn extract_all_modules_duplicate_import() {
    let dir = create_temp_dir("extract_all_dup");
    let sub_path = make_sub_script(&dir);
    let sub_canonical = sub_path.to_string_lossy();

    let main_content = format!(
        "import \"{}\"\nimport \"{}\"\nlet x = 5\n",
        sub_canonical, sub_canonical
    );
    let main_path = create_chrn_file(&dir, "main.chrn", &main_content);
    let cfg = ChrnConfig::default();
    let mut reporter = Reporter::new(100);

    let (compiler, store, diags) = extract_all_modules(&main_path, cfg, &mut reporter)
        .expect("extract_all_modules must succeed");

    let interner = &store.interner;

    // Only 2 user modules (main + sub), not 3
    assert_eq!(
        get_user_mod_count(&compiler),
        2,
        "deduplication: expected 2 user modules, not 3"
    );

    // The sub module exists
    let sub_mod =
        find_module_by_name(&compiler, interner, "sub").expect("sub module must be present");
    assert_eq!(sub_mod.state, ModuleState::Loaded);

    assert!(diags.is_empty(), "no diagnostics expected, got {:?}", diags);

    _ = fs::remove_dir_all(&dir);
}

/// A chain of imports (main -> middle -> leaf) must resolve all three modules.
#[test]
fn extract_all_modules_import_chain() {
    let dir = create_temp_dir("extract_all_chain");

    // leaf.chrn (no imports)
    let leaf_path = create_chrn_file(&dir, "leaf.chrn", "let z = 99\n");
    // middle.chrn imports leaf
    let leaf_canonical = leaf_path.to_string_lossy();
    let middle_content = format!("import \"{}\"\nlet y = leaf::z\n", leaf_canonical);
    let middle_path = create_chrn_file(&dir, "middle.chrn", &middle_content);
    // main.chrn imports middle
    let middle_canonical = middle_path.to_string_lossy();
    let main_content = format!("import \"{}\"\nlet x = middle::y\n", middle_canonical);
    let main_path = create_chrn_file(&dir, "main.chrn", &main_content);

    let cfg = ChrnConfig::default();
    let mut reporter = Reporter::new(100);

    let (compiler, store, diags) = extract_all_modules(&main_path, cfg, &mut reporter)
        .expect("extract_all_modules must succeed");

    let interner = &store.interner;

    // 3 user modules: main, middle, leaf
    assert_eq!(
        get_user_mod_count(&compiler),
        3,
        "expected 3 user modules (main, middle, leaf)"
    );

    // Each module must be Loaded
    for name in &["main", "middle", "leaf"] {
        let m = find_module_by_name(&compiler, interner, name)
            .unwrap_or_else(|| panic!("module '{}' must be present", name));
        assert_eq!(
            m.state,
            ModuleState::Loaded,
            "module '{}' should be Loaded",
            name
        );
    }

    assert!(diags.is_empty(), "no diagnostics expected, got {:?}", diags);

    _ = fs::remove_dir_all(&dir);
}

/// A deep chain of imports (main -> a -> b -> c) must resolve all four
/// modules, verifying that recursion through multiple levels works correctly.
#[test]
fn extract_all_modules_4_deep_chain() {
    let dir = create_temp_dir("extract_all_4chain");

    // c.chrn (leaf, no imports)
    let c_path = create_chrn_file(&dir, "c.chrn", "let w = 1\n");
    // b.chrn imports c
    let c_canonical = c_path.to_string_lossy();
    let b_content = format!("import \"{}\"\nlet v = c::w\n", c_canonical);
    let b_path = create_chrn_file(&dir, "b.chrn", &b_content);
    // a.chrn imports b
    let b_canonical = b_path.to_string_lossy();
    let a_content = format!("import \"{}\"\nlet u = b::v\n", b_canonical);
    let a_path = create_chrn_file(&dir, "a.chrn", &a_content);
    // main.chrn imports a
    let a_canonical = a_path.to_string_lossy();
    let main_content = format!("import \"{}\"\nlet x = a::u\n", a_canonical);
    let main_path = create_chrn_file(&dir, "main.chrn", &main_content);

    let cfg = ChrnConfig::default();
    let mut reporter = Reporter::new(100);

    let (compiler, store, diags) = extract_all_modules(&main_path, cfg, &mut reporter)
        .expect("extract_all_modules must succeed with 4-deep chain");

    let interner = &store.interner;

    // 4 user modules: main, a, b, c
    assert_eq!(
        get_user_mod_count(&compiler),
        4,
        "expected 4 user modules (main, a, b, c)"
    );

    // Each module must be Loaded
    for name in &["main", "a", "b", "c"] {
        let m = find_module_by_name(&compiler, interner, name)
            .unwrap_or_else(|| panic!("module '{}' must be present in 4-deep chain", name));
        assert_eq!(
            m.state,
            ModuleState::Loaded,
            "module '{}' should be Loaded in 4-deep chain",
            name
        );
        assert!(
            m.region_id.is_some(),
            "module '{}' must have a region_id in 4-deep chain",
            name
        );
    }

    assert!(diags.is_empty(), "no diagnostics expected, got {:?}", diags);

    _ = fs::remove_dir_all(&dir);
}

/// Diamond dependency: main imports both a and b, and both a and b import the
/// same shared module (shared).  Verifies deduplication — shared is resolved
/// only once.
#[test]
fn extract_all_modules_diamond_dependency() {
    let dir = create_temp_dir("extract_all_diamond");

    // shared.chrn (no imports)
    let shared_path = create_chrn_file(&dir, "shared.chrn", "let val = 42\n");
    let shared_canonical = shared_path.to_string_lossy();

    // a.chrn imports shared
    let a_content = format!("import \"{}\"\nlet a_val = shared::val\n", shared_canonical);
    let a_path = create_chrn_file(&dir, "a.chrn", &a_content);

    // b.chrn imports shared
    let b_content = format!("import \"{}\"\nlet b_val = shared::val\n", shared_canonical);
    let b_path = create_chrn_file(&dir, "b.chrn", &b_content);

    // main.chrn imports a and b
    let a_canonical = a_path.to_string_lossy();
    let b_canonical = b_path.to_string_lossy();
    let main_content = format!(
        "import \"{}\"\nimport \"{}\"\nlet x = a::a_val + b::b_val\n",
        a_canonical, b_canonical
    );
    let main_path = create_chrn_file(&dir, "main.chrn", &main_content);

    let cfg = ChrnConfig::default();
    let mut reporter = Reporter::new(100);

    let (compiler, store, diags) = extract_all_modules(&main_path, cfg, &mut reporter)
        .expect("extract_all_modules must succeed with diamond deps");

    let interner = &store.interner;

    // 4 user modules: main, a, b, shared — shared must NOT be duplicated
    assert_eq!(
        get_user_mod_count(&compiler),
        4,
        "expected 4 user modules (main, a, b, shared)"
    );

    for name in &["main", "a", "b", "shared"] {
        let m = find_module_by_name(&compiler, interner, name)
            .unwrap_or_else(|| panic!("module '{}' must be present in diamond", name));
        assert_eq!(
            m.state,
            ModuleState::Loaded,
            "module '{}' should be Loaded in diamond",
            name
        );
    }

    // Verify shared appears only once
    let shared_mod = find_module_by_name(&compiler, interner, "shared").unwrap();
    let shared_mod_id = shared_mod.mod_id;
    // Count how many imports reference this id
    let mut ref_count = 0;
    for i in 0..compiler.mods.len() {
        let m = &compiler.mods[ModuleId::new(i as u32)];
        for imp in &m.imports {
            if imp.mod_id == shared_mod_id {
                ref_count += 1;
            }
        }
    }
    assert!(
        ref_count >= 2,
        "shared module should be referenced by at least two imports (a and b), got {}",
        ref_count
    );

    assert!(diags.is_empty(), "no diagnostics expected, got {:?}", diags);

    _ = fs::remove_dir_all(&dir);
}

/// Fan-out: main imports three separate modules with no inter-dependencies
/// among them.  All four modules must be resolved.
#[test]
fn extract_all_modules_fan_out_3_imports() {
    let dir = create_temp_dir("extract_all_fanout");

    let mod1_path = create_chrn_file(&dir, "alpha.chrn", "let a = 10\n");
    let mod2_path = create_chrn_file(&dir, "beta.chrn", "let b = 20\n");
    let mod3_path = create_chrn_file(&dir, "gamma.chrn", "let c = 30\n");

    let m1 = mod1_path.to_string_lossy();
    let m2 = mod2_path.to_string_lossy();
    let m3 = mod3_path.to_string_lossy();

    let main_content = format!(
        "import \"{}\"\nimport \"{}\"\nimport \"{}\"\nlet sum = alpha::a + beta::b + gamma::c\n",
        m1, m2, m3
    );
    let main_path = create_chrn_file(&dir, "main.chrn", &main_content);

    let cfg = ChrnConfig::default();
    let mut reporter = Reporter::new(100);

    let (compiler, store, diags) = extract_all_modules(&main_path, cfg, &mut reporter)
        .expect("extract_all_modules must succeed with 3-way fan-out");

    let interner = &store.interner;

    // 4 user modules: main, alpha, beta, gamma
    assert_eq!(
        get_user_mod_count(&compiler),
        4,
        "expected 4 user modules (main, alpha, beta, gamma)"
    );

    for name in &["main", "alpha", "beta", "gamma"] {
        let m = find_module_by_name(&compiler, interner, name)
            .unwrap_or_else(|| panic!("module '{}' must be present in fan-out", name));
        assert_eq!(
            m.state,
            ModuleState::Loaded,
            "module '{}' should be Loaded in fan-out",
            name
        );
        // Note: leaf modules may have pipeline-injected imports (e.g. core)
        // so we only verify they are Loaded — not their import count.
    }

    // Main module should have 3 user imports + 1 implicit core import = 4 total
    let main_mod = find_module_by_name(&compiler, interner, "main").unwrap();
    assert_eq!(
        main_mod.imports.len(),
        4,
        "main module should have 4 imports (3 user + 1 implicit core) in fan-out, got {}",
        main_mod.imports.len()
    );
    // Verify at least 3 of them are Source imports (the user imports)
    let user_import_count = main_mod
        .imports
        .iter()
        .filter(|i| matches!(i.kind, ImportKind::Source(..)))
        .count();
    assert_eq!(
        user_import_count, 3,
        "main should have 3 Source (user) imports in fan-out, got {}",
        user_import_count
    );

    assert!(diags.is_empty(), "no diagnostics expected, got {:?}", diags);

    _ = fs::remove_dir_all(&dir);
}

/// Complex overlapping imports: main -> a, main -> b, a -> shared, b -> shared,
/// b -> c, c -> shared.  Five unique modules (main, a, b, c, shared) with
/// shared referenced from three sides.
#[test]
fn extract_all_modules_complex_overlap() {
    let dir = create_temp_dir("extract_all_complex");

    // shared.chrn (leaf)
    let shared_path = create_chrn_file(&dir, "shared.chrn", "let base = 100\n");
    let shared_canonical = shared_path.to_string_lossy();

    // c.chrn imports shared
    let c_canonical = shared_canonical.clone();
    let c_content = format!("import \"{}\"\nlet c_val = shared::base\n", c_canonical);
    let c_path = create_chrn_file(&dir, "c.chrn", &c_content);

    // a.chrn imports shared
    let a_canonical = shared_canonical.clone();
    let a_content = format!("import \"{}\"\nlet a_val = shared::base\n", a_canonical);
    let a_path = create_chrn_file(&dir, "a.chrn", &a_content);

    // b.chrn imports shared AND c
    let b_shared = shared_canonical.clone();
    let c_for_b = c_path.to_string_lossy();
    let b_content = format!(
        "import \"{}\"\nimport \"{}\"\nlet b_val = shared::base + c::c_val\n",
        b_shared, c_for_b
    );
    let b_path = create_chrn_file(&dir, "b.chrn", &b_content);

    // main.chrn imports a and b
    let a_for_main = a_path.to_string_lossy();
    let b_for_main = b_path.to_string_lossy();
    let main_content = format!(
        "import \"{}\"\nimport \"{}\"\nlet x = a::a_val + b::b_val\n",
        a_for_main, b_for_main
    );
    let main_path = create_chrn_file(&dir, "main.chrn", &main_content);

    let cfg = ChrnConfig::default();
    let mut reporter = Reporter::new(100);

    let (compiler, store, diags) = extract_all_modules(&main_path, cfg, &mut reporter)
        .expect("extract_all_modules must succeed with complex overlap");

    let interner = &store.interner;

    // 5 user modules: main, a, b, c, shared
    assert_eq!(
        get_user_mod_count(&compiler),
        5,
        "expected 5 user modules (main, a, b, c, shared)"
    );

    for name in &["main", "a", "b", "c", "shared"] {
        let m = find_module_by_name(&compiler, interner, name)
            .unwrap_or_else(|| panic!("module '{}' must be present in complex overlap", name));
        assert_eq!(
            m.state,
            ModuleState::Loaded,
            "module '{}' should be Loaded in complex overlap",
            name
        );
    }

    // Verify deduplication: shared must appear only once
    let shared_mod = find_module_by_name(&compiler, interner, "shared").unwrap();
    let shared_mod_id = shared_mod.mod_id;
    let mut shared_ref_count = 0;
    for i in 0..compiler.mods.len() {
        let m = &compiler.mods[ModuleId::new(i as u32)];
        for imp in &m.imports {
            if imp.mod_id == shared_mod_id {
                shared_ref_count += 1;
            }
        }
    }
    assert!(
        shared_ref_count >= 3,
        "shared module should be referenced by at least three imports (a, b, c), got {}",
        shared_ref_count
    );

    assert!(diags.is_empty(), "no diagnostics expected, got {:?}", diags);

    _ = fs::remove_dir_all(&dir);
}

// ===========================================================================
// ImportKind span() accessor tests
// ===========================================================================

/// Tests that ImportKind::Source carries a SourceSpan that is preserved
/// through the pipeline.
#[test]
fn import_kind_span_preserved() {
    let dir = create_temp_dir("import_span");
    let sub_path = make_sub_script(&dir);
    let main_path = make_importing_script(&dir, &sub_path);
    let cfg = ChrnConfig::default();

    let sub_path_str = sub_path.to_string_lossy();

    let (main_mod, graph, _interner, diags) =
        extract_main(&main_path, &cfg).expect("extract_main must succeed");

    assert!(!main_mod.imports.is_empty(), "at least one import expected");
    let import = &main_mod.imports[0];
    // Extract the span from ImportKind via pattern matching
    let span = match &import.kind {
        ImportKind::Source(sp_path_id) => sp_path_id.span,
        ImportKind::Core => panic!("expected Source import kind, got Core"),
    };
    // The span should cover the import path without quotes.
    // Source is `import "PATH"\nlet x = 5\n`; span starts at byte 8, ends at 8 + path.len().
    let expected_start = 8;
    let expected_end = expected_start + sub_path_str.len() as u32;
    assert_eq!(
        span.start, expected_start,
        "import span must start at byte 8 (right after opening \"), got {}",
        span.start
    );
    assert_eq!(
        span.end,
        expected_end,
        "import span must end at byte {} (after path of length {}), got {}",
        expected_end,
        sub_path_str.len(),
        span.end
    );
    // Verify the span slices out the exact path from src bytes
    let region = &graph.region_arena()[main_mod.region_id.expect("main mod has region_id")];
    let span_bytes = &region.src_bytes[span.start as usize..span.end as usize];
    let span_str = String::from_utf8_lossy(span_bytes);
    assert_eq!(
        span_str, sub_path_str,
        "import span must extract the exact path string from src bytes, got '{}'",
        span_str
    );
    assert!(diags.is_empty(), "no diagnostics expected");

    _ = fs::remove_dir_all(&dir);
}

// ===========================================================================
// ModuleGraph state assertions
// ===========================================================================

/// After `extract_main`, the `ModuleGraph` should have:
/// - `region_arena` of length 1
/// - `reserved_mod_ids` containing only the main module
/// - `other_mods` empty
/// - `seen` containing only the main path id
#[test]
fn module_graph_initial_state() {
    let dir = create_temp_dir("mod_graph_init");
    let main_path = make_simple_script(&dir);
    let cfg = ChrnConfig::default();

    let (_main_mod, graph, mut interner, diags) =
        extract_main(&main_path, &cfg).expect("extract_main must succeed");

    let main_path_id = interner.intern_path(&main_path);

    assert_eq!(
        graph.region_arena().len(),
        1,
        "region_arena must have 1 region"
    );
    assert_eq!(
        graph.reserved_mod_ids().len(),
        1,
        "reserved_mod_ids must have 1 entry"
    );
    assert_eq!(
        graph.reserved_mod_ids()[0],
        (main_path_id, ModuleId::new(0)),
        "reserved_mod_ids[0] must be (main_path, mod_id 0)"
    );
    // assert!(
    //     graph.other_mods().is_empty(),
    //     "other_mods must be empty initially"
    // );
    assert_eq!(graph.seen().len(), 1, "seen must have 1 entry");
    assert!(
        graph.seen().contains(&main_path_id),
        "seen must contain main"
    );
    assert!(diags.is_empty());

    _ = fs::remove_dir_all(&dir);
}

/// After `extract_all_modules`, the returned main module's `mod_id` in the
/// compiler must be 0 (re-assigned during the pipeline).
#[test]
fn main_module_id_is_zero_in_compiler() {
    let dir = create_temp_dir("main_mod_id_zero");
    let main_path = make_simple_script(&dir);
    let cfg = ChrnConfig::default();
    let mut reporter = Reporter::default();

    let (compiler, _, _) = extract_all_modules(&main_path, cfg, &mut reporter)
        .expect("extract_all_modules must succeed");

    let main_mod = &compiler.mods[ModuleId::new(0)];
    assert_eq!(
        main_mod.mod_id,
        ModuleId::new(0),
        "main module must have mod_id 0 after re-assignment"
    );

    _ = fs::remove_dir_all(&dir);
}

// ===========================================================================
// Edge cases
// ===========================================================================

/// An empty file must produce a Loaded module with no imports and no bind.
#[test]
fn extract_main_empty_file() {
    let dir = create_temp_dir("extract_main_empty");
    let main_path = create_chrn_file(&dir, "empty.chrn", "");
    let cfg = ChrnConfig::default();

    let (main_mod, _graph, _interner, diags) =
        extract_main(&main_path, &cfg).expect("extract_main must succeed on empty file");

    assert_eq!(
        main_mod.state,
        ModuleState::Loaded,
        "empty file should be Loaded"
    );
    assert!(main_mod.imports.is_empty(), "empty file has no imports");
    assert!(main_mod.bind.is_none(), "empty file has no bind");
    assert!(diags.is_empty(), "empty file produces no diagnostics");

    _ = fs::remove_dir_all(&dir);
}

/// A file whose name is not valid UTF-8 should produce a terminal error. This
/// is tested by using a path that can't produce a file_prefix().
#[test]
fn extract_main_invalid_utf8_filename() {
    let dir = create_temp_dir("extract_main_invalid_utf8");
    // Create a file with a name that is not valid UTF-8 on unix
    #[cfg(unix)]
    {
        use std::ffi::OsStr;
        use std::os::unix::ffi::OsStrExt;
        let invalid_name = OsStr::from_bytes(&[0xFF, 0xFE]);
        let file_path = dir.join(invalid_name);
        fs::write(&file_path, "let x = 5\n").expect("failed to write invalid UTF-8 file");
        let cfg = ChrnConfig::default();

        let result = extract_main(&file_path, &cfg);
        assert!(
            result.is_err(),
            "file with invalid UTF-8 name should produce Err"
        );
    }

    // On other platforms we skip this test as the file system may not
    // allow non-UTF-8 file names.

    _ = fs::remove_dir_all(&dir);
}
