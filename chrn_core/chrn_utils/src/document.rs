use crate::keywords::Keyword;

pub struct Document {
    pub header: &'static str,
    pub example: Option<&'static str>,
}

impl Document {
    pub fn compose(&self) -> String {
        if let Some(example) = self.example {
            format!("{}\n\n---\n\n**Example:**\n{}", self.header, example)
        } else {
            self.header.to_string()
        }
    }
}

pub static KEYWORD_DOCS: [Document; 15] = [
    Document {
        header: "**struct** — Defines a data structure",
        example: Some(
            "```chrn\nnest->\n    struct Person {\n        name: str\n        age: u8\n    }\n```",
        ),
    },
    Document {
        header: "**enum** — Defines an enum type",
        example: Some(
            "```chrn\nnest->\n    enum Status {\n        Pending\n        Active: Tuple<i32>\n        Completed\n    }\n```",
        ),
    },
    Document {
        header: "**import** — Imports other .chrn files",
        example: Some("```chrn\nimport \"definitions.chrn\"\nimport \"utils.chrn\" as u\n```"),
    },
    Document {
        header: "**export** — Exports types for cross-module use",
        example: Some(
            "```chrn\n\texport let CONST = 42\n\n\texport struct Thing {\n\t\tthings: List<Thing>\n\t}\n\n\texport enum State {\n\t\tReady\t\n}\n\n```",
        ),
    },
    Document {
        header: "**bind** — References external serialized file",
        example: Some("```chrn\nbind \"data.chrn\"\n```"),
    },
    Document {
        header: "**alias** — Creates reusable predicate functions",
        example: Some(
            "```chrn\nalias Positive() = [Range(0.0, 100.0)]\nalias ValidName() = [!IsEmpty, StartsW(\"A\")]\n```",
        ),
    },
    Document {
        header: "**let** — Declares reusable values",
        example: Some(
            "```chrn\n@def\n    let count = 10\n    let name = \"test\"\n    let result = VALUE * 2\n@end\n```",
        ),
    },
    Document {
        header: "**change** — Unimplemented",
        example: Some("```chrn\n// Not yet implemented\n```"),
    },
    Document {
        header: "**as** — Aliases imported module names",
        example: Some("```chrn\n@def\n    import \"module.chrn\" as m\n@end\n```"),
    },
    Document {
        header: "**var->** — Defines serializable fields section",
        example: Some(
            "```chrn\nvar->\n    name: str\n    age: u8 #warn\n    score: f64 [Range(0.0, 100.0)]\n```",
        ),
    },
    Document {
        header: "**nest->** — Defines structs and enums section",
        example: Some(
            "```chrn\nnest->\n    struct Address {\n        city: str\n        zip: u32\n    }\n    enum Color {Red Blue Green}\n```",
        ),
    },
    Document {
        header: "**complex->** — Unimplemented",
        example: Some("```chrn\n// Not yet implemented\n```"),
    },
    Document {
        header: "**override->** — Unimplemented",
        example: Some("```chrn\n// Not yet implemented\n```"),
    },
    Document {
        header: "**IsEmpty** — Predicate to check emptiness",
        example: Some("```chrn\nvar->\n    field: List<T> [!IsEmpty]\n```"),
    },
    Document {
        header: "**IsWhitespace** — Predicate to check whitespace",
        example: Some("```chrn\nvar->\n    field: str [!IsWhitespace]\n```"),
    },
];

pub static DIRECTIVE_DOCS: [Document; 4] = [
    Document {
        header: "**@def** — Starts embedded script block",
        example: Some("```chrn\n@def\n\tlet x = 1\n\tvar->\n\t\tname: str\n@end\n```"),
    },
    Document {
        header: "**@end** — Ends embedded script block",
        example: Some(
            "```chrn\n@def\n    let x = 1\n@end\n// Everything after this is serialized data\n```",
        ),
    },
    Document {
        header: "**@** — Directive marker (e.g. @def/@end)",
        example: None,
    },
    Document {
        header: "**#** — Argument prefix (#warn/#ignore)",
        example: None,
    },
];

pub static PREDICATE_DOCS: [Document; 13] = [
    Document {
        header: "**Range(min, max)** — Range predicate",
        example: Some("```chrn\nvar->\n    score: f64 [Range(0.0, 100.0)]\n```"),
    },
    Document {
        header: "**StartsW(prefix)** — Starts with predicate",
        example: Some("```chrn\nvar->\n    name: str [StartsW(\"A\")]\n```"),
    },
    Document {
        header: "**EndsW(suffix)** — Ends with predicate",
        example: Some("```chrn\nvar->\n    extension: str [EndsW(\".chrn\")]\n```"),
    },
    Document {
        header: "**Contains(substr)** — Contains predicate",
        example: Some("```chrn\nvar->\n    description: str [Contains(\"important\")]\n```"),
    },
    Document {
        header: "**Equals(value)** — Equality predicate",
        example: Some("```chrn\nvar->\n    status: str [Equals(\"active\")]\n```"),
    },
    Document {
        header: "**?** — Type inference placeholder",
        example: Some("```chrn\nvar->\n    value: ?\n```"),
    },
    Document {
        header: "**#warn** — Treat as warning instead of error",
        example: Some("```chrn\nvar->\n    field: str #warn\n```"),
    },
    Document {
        header: "**#ignore** — Ignore type errors",
        example: Some("```chrn\nvar->\n    field: ? #ignore\n```"),
    },
    Document {
        header: "**Range** — Range predicate",
        example: Some("```chrn\nvar->\n    score: f64 [Range(0.0, 100.0)]\n```"),
    },
    Document {
        header: "**StartsW** — Starts with predicate",
        example: Some("```chrn\nvar->\n    name: str [StartsW(\"A\")]\n```"),
    },
    Document {
        header: "**EndsW** — Ends with predicate",
        example: Some("```chrn\nvar->\n    extension: str [EndsW(\".chrn\")]\n```"),
    },
    Document {
        header: "**Contains** — Contains predicate",
        example: Some("```chrn\nvar->\n    description: str [Contains(\"important\")]\n```"),
    },
    Document {
        header: "**Equals** — Equality predicate",
        example: Some("```chrn\nvar->\n    status: str [Equals(\"active\")]\n```"),
    },
];

pub static TYPE_DOCS: [Document; 32] = [
    Document {
        header: "**List<T>** — Generic list type",
        example: Some("```chrn\nvar->\n    items: List<Item>\n```"),
    },
    Document {
        header: "**Set<T>** — Generic set type",
        example: Some("```chrn\nvar->\n    tags: Set<Tag>\n```"),
    },
    Document {
        header: "**Map<K, V>** — Generic map type",
        example: Some("```chrn\nvar->\n    lookup: Map<str, Value>\n```"),
    },
    Document {
        header: "**Tuple<A, B, ...>** — Generic tuple type",
        example: Some("```chrn\nvar->\n    coord: Tuple<i32, i32>\n```"),
    },
    Document {
        header: "**<T>** — Generic type",
        example: Some("```chrn\nvar->\n    field: Any<str>\n```"),
    },
    Document {
        header: "**List** — Generic list type",
        example: Some("```chrn\nvar->\n    items: List<Item>\n```"),
    },
    Document {
        header: "**Set** — Generic set type",
        example: Some("```chrn\nvar->\n    tags: Set<Tag>\n```"),
    },
    Document {
        header: "**Map** — Generic map type",
        example: Some("```chrn\nvar->\n    lookup: Map<str, Value>\n```"),
    },
    Document {
        header: "**Tuple** — Generic tuple type",
        example: Some("```chrn\nvar->\n    coord: Tuple<i32, i32>\n```"),
    },
    Document {
        header: "**Any** — Generic type",
        example: Some("```chrn\nvar->\n    field: Any<str>\n```"),
    },
    Document {
        header: "**str** — String type",
        example: None,
    },
    Document {
        header: "**bool** — Boolean type",
        example: Some("```chrn\nvar->\n    active: bool\n```"),
    },
    Document {
        header: "**char** — Unicode character",
        example: None,
    },
    Document {
        header: "**nil** — Nil type (no value)",
        example: Some("```chrn\nvar->\n    nothing: nil\n```"),
    },
    Document {
        header: "**i8** — 8-bit signed integer",
        example: None,
    },
    Document {
        header: "**u8** — 8-bit unsigned integer",
        example: None,
    },
    Document {
        header: "**i16** — 16-bit signed integer",
        example: None,
    },
    Document {
        header: "**u16** — 16-bit unsigned integer",
        example: None,
    },
    Document {
        header: "**i32** — 32-bit signed integer",
        example: None,
    },
    Document {
        header: "**u32** — 32-bit unsigned integer",
        example: None,
    },
    Document {
        header: "**i64** — 64-bit signed integer",
        example: None,
    },
    Document {
        header: "**u64** — 64-bit unsigned integer",
        example: None,
    },
    Document {
        header: "**i128** — 128-bit signed integer",
        example: None,
    },
    Document {
        header: "**u128** — 128-bit unsigned integer",
        example: None,
    },
    Document {
        header: "**sized** — Platform-sized signed integer",
        example: None,
    },
    Document {
        header: "**unsized** — Platform-sized unsigned integer",
        example: None,
    },
    Document {
        header: "**f16** — 16-bit floating point",
        example: None,
    },
    Document {
        header: "**f32** — 32-bit floating point",
        example: None,
    },
    Document {
        header: "**f64** — 64-bit floating point",
        example: None,
    },
    Document {
        header: "**f128** — 128-bit floating point",
        example: None,
    },
    Document {
        header: "**BigInt** — Arbitrary precision integer",
        example: Some("```chrn\nvar->\n    big_num: BigInt\n```"),
    },
    Document {
        header: "**BigFloat** — Arbitrary precision float",
        example: Some("```chrn\nvar->\n    big_price: BigFloat\n```"),
    },
];

impl Document {
    pub fn keyword_docs(kw: Keyword) -> &'static Document {
        &KEYWORD_DOCS[kw as usize]
    }

    pub fn lookup(key: &str) -> Option<&'static Document> {
        for doc in DIRECTIVE_DOCS.iter() {
            if doc.header.starts_with(&format!("**{}**", key)) {
                return Some(doc);
            }
        }
        for doc in PREDICATE_DOCS.iter() {
            if doc.header.starts_with(&format!("**{}**", key)) {
                return Some(doc);
            }
        }
        for doc in TYPE_DOCS.iter() {
            if doc.header.starts_with(&format!("**{}**", key)) {
                return Some(doc);
            }
        }
        None
    }
}
