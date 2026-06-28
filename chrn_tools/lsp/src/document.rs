//! # document
//!
//! Provides static documentation tables and the [`Document`] type for hover content.
//!
//! Three tables are exposed as `pub static` slices:
//!
//! | Constant              | Indexed by                  | Length |
//! |-----------------------|-----------------------------|--------|
//! | [`KEYWORD_DOCS`]      | `Keyword as usize`          | 13     |
//! | [`BUILTIN_TYPE_DOCS`] | `BuiltinTypeKind as usize`  | 27     |
//! | [`FUNC_DOCS`]         | `FuncKind as usize`         | 7      |
//! | [`DIRECTIVE_DOCS`]    | key name (`&str`)           | 6      |
//!
//! ## Alignment invariant
//!
//! **The entries in each table MUST remain aligned with the discriminant values of
//! their respective enum.**  Adding a new keyword, builtin type, or intrinsic function
//! requires inserting the corresponding [`Document`] entry at the correct index and
//! updating the length in this comment.
//!
//! Accessor methods on [`Document`] (`keyword_docs`, `builtin_type_docs`, `func_docs`)
//! index directly into these arrays; an out-of-bounds index will panic at runtime.

use compilation::semantic::hir::hir_concepts::FuncKind;
use lang::keywords::Keyword;
use lang::types::builtins::BuiltinTypeKind;

/// 60 Dashes
pub static HOVER_DASHES: &str = "------------------------------------------------------------";

/// A structured documentation entry for a language construct.
/// Separates the name and description from the rendered presentation,
/// enabling both direct lookup by key and formatted hover output.
pub struct Document {
    /// The name of the construct (e.g. "struct", "BigInt", "i32")
    pub key: &'static str,
    /// A brief description of the construct
    pub description: &'static str,
    /// An optional code example shown in hover popups
    pub example: Option<&'static str>,
}

impl Document {
    /// Composes a markdown hover string from the document's fields.
    pub fn compose(&self) -> String {
        let header = format!("**{}** — {}", self.key, self.description);
        match self.example {
            Some(example) => format!(
                "{}\n\n{}\n\n**Example:**\n{}",
                header, HOVER_DASHES, example
            ),
            None => header,
        }
    }

    /// Returns the document for a given keyword variant.
    pub fn keyword_docs(kw: Keyword) -> &'static Document {
        &KEYWORD_DOCS[kw as usize]
    }

    /// Returns the document for a given builtin type kind.
    pub fn builtin_type_docs(kind: BuiltinTypeKind) -> &'static Document {
        &BUILTIN_TYPE_DOCS[kind as usize]
    }

    /// Returns the document for a given intrinsic function kind.
    pub fn func_docs(kind: FuncKind) -> &'static Document {
        &FUNC_DOCS[kind as usize]
    }

    /// Returns the document for a directive by name, or `None` if unknown.
    pub fn directive_docs(name: &str) -> Option<&'static Document> {
        DIRECTIVE_DOCS.iter().find(|d| d.key == name)
    }
}

//  Keywords

// ── Keywords ─────────────────────────────────────────────────────────────────
//
// Indexed by `Keyword as usize`.  Variants must appear in the same order as the
// `Keyword` enum definition in `lang::keywords`.
//FIX: Key should be the interned id, or maybe a mixture of both depending on DECISIONS
/// Hover documentation for each Chern language keyword.
///
/// Indexed by [`Keyword`] discriminant via [`Document::keyword_docs`].
pub static KEYWORD_DOCS: [Document; 13] = [
    Document {
        key: "struct",
        description: "Defines a data structure",
        example: Some("```chrn\nnest->\n\tstruct Person {\n\t\tname: str\n\t\tage: u8\n\t}\n```"),
    },
    Document {
        key: "enum",
        description: "Defines an enum type",
        example: Some(
            "```chrn\nnest->\n\tenum Status {\n\t\tPending\n\t\tActive: Tuple<i32>\n\t\tCompleted\n\t}\n```",
        ),
    },
    Document {
        key: "import",
        description: "Imports other .chrn files",
        example: Some("```chrn\nimport \"definitions.chrn\"\nimport \"utils.chrn\" as u\n```"),
    },
    Document {
        key: "export",
        description: "Exports types for cross-module use",
        example: Some(
            "```chrn\nexport let CONST = 42\n\nexport struct Thing {\n\tthings: List<Thing>\n}\n\nexport enum State {\n\tReady\n}\n```",
        ),
    },
    Document {
        key: "bind",
        description: "Binds to external serialized file",
        example: Some("```chrn\nbind \"data.chrn\"\n```"),
    },
    Document {
        key: "alias",
        description: "Creates reusable predicate functions",
        example: Some(
            "```chrn\nalias Positive() = [Range(0.0, 100.0)]\n// Can also be exported\nexport alias ValidName() = [!IsEmpty, StartsW(\"chrn\")]\n```",
        ),
    },
    Document {
        key: "let",
        description: "Declares reusable values",
        example: Some(
            "```chrn\n@def\n\tlet count = 10\n\tlet name = \"chrning\"\n\tlet result = VALUE * 2\n// Can be used for any conditions\nvar->\n\tx: i32 [Equals(result)]\n@end\n```",
        ),
    },
    Document {
        key: "change",
        description: "Unimplemented",
        example: Some("```chrn\n// Not yet implemented\n```"),
    },
    Document {
        key: "as",
        description: "Aliases imported module names",
        example: Some(
            "```chrn\n@def\n\timport \"module.chrn\" as mod\n\t\tlet x = mod.MAGIC_NUM - 2\nvar->\n\tfield: mod.EXTERN_TYPE\n@end\n```",
        ),
    },
    Document {
        key: "var->",
        description: "Defines serializable fields section",
        example: Some(
            "```chrn\nvar->\n\tname: str\n\tage: u8 #warn\n\tscore: f64 [Range(0.0, 100.0)]\n```",
        ),
    },
    Document {
        key: "nest->",
        description: "Defines structs and enums section",
        example: Some(
            "```chrn\nnest->\n\tstruct Address {\n\t\tcity: str\n\t\tzip: u32\n\t}\n\tenum Color {Red Blue Green}\n\n```",
        ),
    },
    Document {
        key: "complex->",
        description: "Unimplemented",
        example: Some("```chrn\n// Not yet implemented\n```"),
    },
    Document {
        key: "override->",
        description: "Unimplemented",
        example: Some("```chrn\n// Not yet implemented\n```"),
    },
];

// ── Builtin types ─────────────────────────────────────────────────────────────
//
// Indexed by `BuiltinTypeKind as usize`.  Variants must appear in the same order
// as the `BuiltinTypeKind` enum definition in `lang::types::builtins`.
/// Hover documentation for each Chern builtin type.
///
/// Indexed by [`BuiltinTypeKind`] discriminant via [`Document::builtin_type_docs`].
pub static BUILTIN_TYPE_DOCS: [Document; 27] = [
    Document {
        key: "i8",
        description: "8-bit signed integer",
        example: None,
    },
    Document {
        key: "u8",
        description: "8-bit unsigned integer",
        example: None,
    },
    Document {
        key: "i16",
        description: "16-bit signed integer",
        example: None,
    },
    Document {
        key: "u16",
        description: "16-bit unsigned integer",
        example: None,
    },
    Document {
        key: "f16",
        description: "16-bit floating point",
        example: None,
    },
    Document {
        key: "i32",
        description: "32-bit signed integer",
        example: None,
    },
    Document {
        key: "u32",
        description: "32-bit unsigned integer",
        example: None,
    },
    Document {
        key: "f32",
        description: "32-bit floating point",
        example: None,
    },
    Document {
        key: "i64",
        description: "64-bit signed integer",
        example: None,
    },
    Document {
        key: "u64",
        description: "64-bit unsigned integer",
        example: None,
    },
    Document {
        key: "f64",
        description: "64-bit floating point",
        example: None,
    },
    Document {
        key: "i128",
        description: "128-bit signed integer",
        example: None,
    },
    Document {
        key: "u128",
        description: "128-bit unsigned integer",
        example: None,
    },
    Document {
        key: "f128",
        description: "128-bit floating point",
        example: None,
    },
    Document {
        key: "sized",
        description: "Platform-sized signed integer",
        example: None,
    },
    Document {
        key: "unsized",
        description: "Platform-sized unsigned integer",
        example: None,
    },
    Document {
        key: "str",
        description: "String type",
        example: None,
    },
    Document {
        key: "char",
        description: "Unicode character",
        example: None,
    },
    Document {
        key: "nil",
        description: "Representation of a null/nil within a given language if possible",
        example: None,
    },
    Document {
        key: "bool",
        description: "Boolean type",
        example: None,
    },
    Document {
        key: "BigInt",
        description: "Arbitrary precision integer",
        example: None,
    },
    Document {
        key: "BigFloat",
        description: "Arbitrary precision float",
        example: None,
    },
    Document {
        key: "List",
        description: "Generic list type",
        example: None,
    },
    Document {
        key: "Set",
        description: "Generic set type",
        example: None,
    },
    Document {
        key: "Map",
        description: "Generic map type",
        example: None,
    },
    Document {
        key: "Tuple",
        description: "Generic tuple type",
        example: None,
    },
    Document {
        key: "Runtime",
        description: "Runtime detected type",
        example: None,
    },
];

// ── Intrinsic functions ───────────────────────────────────────────────────────
//
// Indexed by `FuncKind as usize`.  Variants must appear in the same order as the
// `FuncKind` enum definition in `compilation::semantic::hir`.
/// Hover documentation for each Chern intrinsic (built-in) function.
///
/// Indexed by [`FuncKind`] discriminant via [`Document::func_docs`].
pub static FUNC_DOCS: [Document; 7] = [
    Document {
        key: "IsEmpty",
        description: "Checks if a value is empty",
        example: None,
    },
    Document {
        key: "IsWhitespace",
        description: "Checks if a string contains only whitespace characters",
        example: None,
    },
    Document {
        key: "Contains",
        description: "Checks if a value contains a specified pattern",
        example: None,
    },
    Document {
        key: "Range",
        description: "Checks if a value falls within a specified range",
        example: None,
    },
    Document {
        key: "StartsW",
        description: "Checks if a string starts with a specified prefix",
        example: None,
    },
    Document {
        key: "EndsW",
        description: "Checks if a string ends with a specified suffix",
        example: None,
    },
    Document {
        key: "Equals",
        description: "Checks if a value equals another value",
        example: None,
    },
];

// ── Directives ────────────────────────────────────────────────────────────────
//
// Ordered by directive index: warn, ignore, scient, hex, bin, octal.
// Looked up by key name via [`Document::directive_docs`].
/// Hover documentation for each Chern directive.
///
/// Indexed by key name via [`Document::directive_docs`].
pub static DIRECTIVE_DOCS: [Document; 6] = [
    Document {
        key: "warn",
        description: "Warns instead of terminating on constraint violations",
        example: Some("```chrn\nvar->\n\tscore: f64 [Range(0.0, 100.0)] #warn\n```"),
    },
    Document {
        key: "ignore",
        description: "Ignores all serialization errors for the applied type",
        example: Some("```chrn\nvar->\n\tptr: Runtime #ignore\n\tlen: Runtime #ignore\n```"),
    },
    Document {
        key: "scient",
        description: "Outputs numeric values in scientific notation",
        example: Some("```chrn\nvar->\n\tpi: f64 #scient\n```"),
    },
    Document {
        key: "hex",
        description: "Outputs numeric values in hexadecimal notation",
        example: Some(
            "```chrn\nnest->\n\tenum Color { Red: Tuple<u8> Green: Tuple<u8> Blue: Tuple<u8> } #hex\n```",
        ),
    },
    Document {
        key: "bin",
        description: "Outputs numeric values in binary notation",
        example: Some("```chrn\nvar->\n\tflags: u8 #bin\n```"),
    },
    Document {
        key: "octal",
        description: "Outputs numeric values in octal notation",
        example: Some("```chrn\nvar->\n\tperm: u32 #octal\n```"),
    },
];
