use chrn_utils::keywords::Keyword;

/// 60 Dashes
pub static HOVER_DASHES: &str = "------------------------------------------------------------";

/// A structured documentation entry for a language construct.
/// Separates the name and description from the rendered presentation,
/// enabling both direct lookup by key and formatted hover output.
pub struct Document {
    /// The name of the construct (e.g. "struct", "Range", "i32")
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

    /// Looks up documentation by key across directives, predicates, and types.
    pub fn lookup(key: &str) -> Option<&'static Document> {
        DIRECTIVE_DOCS
            .iter()
            .chain(PREDICATE_DOCS.iter())
            .chain(TYPE_DOCS.iter())
            .find(|doc| doc.key == key)
    }
}

//  Keywords

pub static KEYWORD_DOCS: [Document; 15] = [
    //FIX: Key should be the interned id, or maybe a mixture of both depending on DECISIONS
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
    Document {
        key: "IsEmpty",
        description: "Predicate to check emptiness",
        example: Some("```chrn\nvar->\n\tfield: List<i32> [!IsEmpty]\n```"),
    },
    Document {
        key: "IsWhitespace",
        description: "Predicate to check whitespace",
        example: Some("```chrn\nvar->\n\tfield: str [!IsWhitespace]\n```"),
    },
];

//  Directives

pub static DIRECTIVE_DOCS: [Document; 4] = [
    Document {
        key: "@def",
        description: "Starts embedded script block",
        example: Some("```chrn\n@def\n\tlet x = 1\n\tvar->\n\t\tname: str\n@end\n```"),
    },
    Document {
        key: "@end",
        description: "Ends embedded script block",
        example: Some(
            "```chrn\n@def\n\tlet x = 1\n@end\n// Everything after this is serialized data\n```",
        ),
    },
    Document {
        key: "@",
        description: "Directive marker (e.g. @def/@end)",
        example: None,
    },
    Document {
        key: "#",
        description: "Argument prefix (#warn/#ignore)",
        example: None,
    },
];

//  Predicates

pub static PREDICATE_DOCS: [Document; 8] = [
    Document {
        key: "Range",
        description: "Range predicate",
        example: Some("```chrn\nvar->\n\tscore: f64 [Range(0.0, 100.0)]\n```"),
    },
    Document {
        key: "StartsW",
        description: "Starts with predicate",
        example: Some("```chrn\nvar->\n\tname: str [StartsW(\"A\")]\n```"),
    },
    Document {
        key: "EndsW",
        description: "Ends with predicate",
        example: Some("```chrn\nvar->\n\textension: str [EndsW(\".chrn\")]\n```"),
    },
    Document {
        key: "Contains",
        description: "Contains predicate",
        example: Some("```chrn\nvar->\n\tdescription: str [Contains(\"important\")]\n```"),
    },
    Document {
        key: "Equals",
        description: "Equality predicate",
        example: Some("```chrn\nvar->\n\tstatus: str [Equals(\"active\")]\n```"),
    },
    Document {
        key: "?",
        description: "Type inference placeholder",
        example: Some("```chrn\nvar->\n\tvalue: ?\n```"),
    },
    Document {
        key: "#warn",
        description: "Treat as warning instead of error",
        example: Some("```chrn\nvar->\n\tfield: str #warn\n```"),
    },
    Document {
        key: "#ignore",
        description: "Ignore type errors",
        example: Some("```chrn\nvar->\n\tfield: ? #ignore\n```"),
    },
];

//  Types

pub static TYPE_DOCS: [Document; 27] = [
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
        key: "Any",
        description: "Generic type",
        example: None,
    },
    Document {
        key: "str",
        description: "String type",
        example: None,
    },
    Document {
        key: "bool",
        description: "Boolean type",
        example: None,
    },
    Document {
        key: "char",
        description: "Unicode character",
        example: None,
    },
    Document {
        key: "nil",
        description: "Nil type (no value)",
        example: None,
    },
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
        key: "f16",
        description: "16-bit floating point",
        example: None,
    },
    Document {
        key: "f32",
        description: "32-bit floating point",
        example: None,
    },
    Document {
        key: "f64",
        description: "64-bit floating point",
        example: None,
    },
    Document {
        key: "f128",
        description: "128-bit floating point",
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
];
