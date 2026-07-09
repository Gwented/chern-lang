pub mod arena;
pub mod budget;
pub mod chrn_config;
pub mod core_error;
pub mod files;
pub mod help_model;
pub mod id_types;
pub mod intern;
pub mod macros;
pub mod pair;
pub mod source_map;

// -- Heuristic max amounts for `chrn` to abide by for safety purposes --

// IGNORE THIS
/// Max loops before what would be considered a broken mutation loop.
/// Arbitrarily high number to help examine recursive bugs better
pub const MAX_LOOPS: usize = 10000004;

/// Max modules that can be in memory at once
pub const MAX_MODULES: u8 = 1; // 500

/// Max recursive descent that can be done
pub const MAX_RECURSIVE_DEPTH: u16 = 1; // 512

/// Max expression nodes that can be consumed in a singule expression tree
pub const MAX_EXPR_NODES: usize = 1; // 5,000,000

#[cfg(test)]
pub mod tests {
    use crate::{
        budget::mem_budget::{BudgetResult, MemoryBudget},
        id_types::PathId,
        intern::{self, Intern},
        source_map::source_diagnostic::{DiagnosticLevel, Reporter, SourceDiagnostic},
    };

    #[test]
    fn keyword_intern_alignment() {
        let interner = Intern::init();

        assert_eq!(
            "self",
            interner.search_idx(intern::INTERNED_SELF as usize),
            "INTERNED_SELF (0) should be 'self'"
        );
        assert_eq!(
            "struct",
            interner.search_idx(intern::INTERNED_STRUCT as usize),
            "INTERNED_STRUCT (1) should be 'struct'"
        );
        assert_eq!(
            "import",
            interner.search_idx(intern::INTERNED_IMPORT as usize),
            "INTERNED_IMPORT (2) should be 'import'"
        );
        assert_eq!(
            "export",
            interner.search_idx(intern::INTERNED_EXPORT as usize),
            "INTERNED_EXPORT (3) should be 'export'"
        );
        assert_eq!(
            "bind",
            interner.search_idx(intern::INTERNED_BIND as usize),
            "INTERNED_BIND (4) should be 'bind'"
        );
        assert_eq!(
            "alias",
            interner.search_idx(intern::INTERNED_ALIAS as usize),
            "INTERNED_ALIAS (5) should be 'alias'"
        );
        assert_eq!(
            "let",
            interner.search_idx(intern::INTERNED_LET as usize),
            "INTERNED_LET (6) should be 'let'"
        );
        assert_eq!(
            "change",
            interner.search_idx(intern::INTERNED_CHANGE as usize),
            "INTERNED_CHANGE (7) should be 'change'"
        );
        assert_eq!(
            "as",
            interner.search_idx(intern::INTERNED_AS as usize),
            "INTERNED_AS (8) should be 'as'"
        );
        assert_eq!(
            "var",
            interner.search_idx(intern::INTERNED_VAR as usize),
            "INTERNED_VAR (9) should be 'var'"
        );
        assert_eq!(
            "nest",
            interner.search_idx(intern::INTERNED_NEST as usize),
            "INTERNED_NEST (10) should be 'nest'"
        );
        assert_eq!(
            "complex",
            interner.search_idx(intern::INTERNED_COMPLEX as usize),
            "INTERNED_COMPLEX (11) should be 'complex'"
        );
        assert_eq!(
            "override",
            interner.search_idx(intern::INTERNED_OVERRIDE as usize),
            "INTERNED_OVERRIDE (12) should be 'override'"
        );
        assert_eq!(
            "true",
            interner.search_idx(intern::INTERNED_TRUE as usize),
            "INTERNED_TRUE (13) should be 'true'"
        );
        assert_eq!(
            "false",
            interner.search_idx(intern::INTERNED_FALSE as usize),
            "INTERNED_FALSE (14) should be 'false'"
        );
        assert_eq!(
            "IsEmpty",
            interner.search_idx(intern::INTERNED_IS_EMPTY as usize),
            "INTERNED_IS_EMPTY (15) should be 'IsEmpty'"
        );
        assert_eq!(
            "IsWhitespace",
            interner.search_idx(intern::INTERNED_IS_WHITESPACE as usize),
            "INTERNED_IS_WHITESPACE (16) should be 'IsWhitespace'"
        );
        assert_eq!(
            "Range",
            interner.search_idx(intern::INTERNED_RANGE as usize),
            "INTERNED_RANGE (17) should be 'Range'"
        );
        assert_eq!(
            "StartsW",
            interner.search_idx(intern::INTERNED_STARTSW as usize),
            "INTERNED_STARTSW (18) should be 'StartsW'"
        );
        assert_eq!(
            "EndsW",
            interner.search_idx(intern::INTERNED_ENDSW as usize),
            "INTERNED_ENDSW (19) should be 'EndsW'"
        );
        assert_eq!(
            "Contains",
            interner.search_idx(intern::INTERNED_CONTAINS as usize),
            "INTERNED_CONTAINS (20) should be 'Contains'"
        );
        assert_eq!(
            "Equals",
            interner.search_idx(intern::INTERNED_EQUALS as usize),
            "INTERNED_EQUALS (21) should be 'Equals'"
        );
    }

    /// Regression test for the out-of-bounds read in `Intern::append`.
    ///
    /// Before the fix, the loop was `INTERNER_PRELOAD_SIZE..=other.stored_strs.len()`,
    /// which iterated past the end of the vector whenever `other` had more than
    /// `INTERNER_PRELOAD_SIZE` (= 64) interned strings. This manifested as the
    /// LSP observing "xThingState2026", "vairableo", and other corrupt identifiers
    /// during rapid editing (see `chrn_tools/lsp/stop_removing_this_file`).
    ///
    /// With the fix, `append` only iterates `INTERNER_PRELOAD_SIZE..len` (exclusive)
    /// so we can build a fully-populated second interner, append it into a third,
    /// and assert every string round-trips correctly.
    #[test]
    fn append_does_not_read_out_of_bounds() {
        let mut donor = Intern::init();

        // Add 200 user-defined strings to `donor`. This is well past the
        // 64-slot preloaded range, which is what triggers the original bug.
        for i in 0..200 {
            donor.intern(&format!("user_string_{i}"));
        }

        let mut receiver = Intern::init();
        receiver.append(&donor);

        // Every user-defined string must round-trip.
        for i in 0..200 {
            let original = format!("user_string_{i}");
            let id = receiver
                .try_search_str(&original)
                .expect("missing {original} after append");
            assert_eq!(receiver.search(id), original);
        }
    }

    /// Specifically targets the case where the donor has *exactly* the
    /// preloaded size, so the buggy `..=` would have iterated i = N, the
    /// single illegal index.
    #[test]
    fn append_donor_at_exact_preload_size_boundary() {
        let mut donor = Intern::init();
        // Intern exactly one user string so the donor has INTERNER_PRELOAD_SIZE + 1 entries.
        donor.intern("boundary_string");

        let mut receiver = Intern::init();
        receiver.append(&donor);

        assert_eq!(
            receiver.try_search_str("boundary_string"),
            Some(receiver.try_search_str("boundary_string").unwrap()),
        );
    }

    #[test]
    fn builtin_type_intern_alignment() {
        let interner = Intern::init();

        assert_eq!(
            "i8",
            interner.search_idx(intern::INTERNED_I8 as usize),
            "INTERNED_I8 (22) should be 'i8'"
        );
        assert_eq!(
            "u8",
            interner.search_idx(intern::INTERNED_U8 as usize),
            "INTERNED_U8 (23) should be 'u8'"
        );
        assert_eq!(
            "i16",
            interner.search_idx(intern::INTERNED_I16 as usize),
            "INTERNED_I16 (24) should be 'i16'"
        );
        assert_eq!(
            "u16",
            interner.search_idx(intern::INTERNED_U16 as usize),
            "INTERNED_U16 (25) should be 'u16'"
        );
        assert_eq!(
            "i32",
            interner.search_idx(intern::INTERNED_I32 as usize),
            "INTERNED_I32 (26) should be 'i32'"
        );
        assert_eq!(
            "u32",
            interner.search_idx(intern::INTERNED_U32 as usize),
            "INTERNED_U32 (27) should be 'u32'"
        );
        assert_eq!(
            "f32",
            interner.search_idx(intern::INTERNED_F32 as usize),
            "INTERNED_F32 (28) should be 'f32'"
        );
        assert_eq!(
            "i64",
            interner.search_idx(intern::INTERNED_I64 as usize),
            "INTERNED_I64 (29) should be 'i64'"
        );
        assert_eq!(
            "u64",
            interner.search_idx(intern::INTERNED_U64 as usize),
            "INTERNED_U64 (30) should be 'u64'"
        );
        assert_eq!(
            "f64",
            interner.search_idx(intern::INTERNED_F64 as usize),
            "INTERNED_F64 (31) should be 'f64'"
        );
        assert_eq!(
            "i128",
            interner.search_idx(intern::INTERNED_I128 as usize),
            "INTERNED_I128 (32) should be 'i128'"
        );
        assert_eq!(
            "u128",
            interner.search_idx(intern::INTERNED_U128 as usize),
            "INTERNED_U128 (33) should be 'u128'"
        );
        assert_eq!(
            "f128",
            interner.search_idx(intern::INTERNED_F128 as usize),
            "INTERNED_F128 (34) should be 'f128'"
        );
        assert_eq!(
            "sized",
            interner.search_idx(intern::INTERNED_SIZED as usize),
            "INTERNED_SIZED (35) should be 'sized'"
        );
        assert_eq!(
            "unsized",
            interner.search_idx(intern::INTERNED_UNSIZED as usize),
            "INTERNED_UNSIZED (36) should be 'unsized'"
        );
        assert_eq!(
            "bool",
            interner.search_idx(intern::INTERNED_BOOL as usize),
            "INTERNED_BOOL (37) should be 'bool'"
        );
        assert_eq!(
            "nil",
            interner.search_idx(intern::INTERNED_NIL as usize),
            "INTERNED_NIL (38) should be 'nil'"
        );
        assert_eq!(
            "char",
            interner.search_idx(intern::INTERNED_CHAR as usize),
            "INTERNED_CHAR (39) should be 'char'"
        );
        assert_eq!(
            "str",
            interner.search_idx(intern::INTERNED_STR as usize),
            "INTERNED_STR (40) should be 'str'"
        );
        assert_eq!(
            "BigInt",
            interner.search_idx(intern::INTERNED_BIGINT as usize),
            "INTERNED_BIGINT (41) should be 'BigInt'"
        );
        assert_eq!(
            "BigFloat",
            interner.search_idx(intern::INTERNED_BIGFLOAT as usize),
            "INTERNED_BIGFLOAT (42) should be 'BigFloat'"
        );
        assert_eq!(
            "List",
            interner.search_idx(intern::INTERNED_LIST as usize),
            "INTERNED_LIST (43) should be 'List'"
        );
        assert_eq!(
            "Set",
            interner.search_idx(intern::INTERNED_SET as usize),
            "INTERNED_SET (44) should be 'Set'"
        );
        assert_eq!(
            "Map",
            interner.search_idx(intern::INTERNED_MAP as usize),
            "INTERNED_MAP (45) should be 'Map'"
        );
        assert_eq!(
            "Tuple",
            interner.search_idx(intern::INTERNED_TUPLE as usize),
            "INTERNED_TUPLE (46) should be 'Tuple'"
        );
    }

    #[test]
    fn builtin_type_kind_alignment() {
        let interner = Intern::init();

        assert_eq!(interner.search_idx(intern::INTERNED_I8 as usize), "i8");
        assert_eq!(interner.search_idx(intern::INTERNED_U8 as usize), "u8");
        assert_eq!(interner.search_idx(intern::INTERNED_I16 as usize), "i16");
        assert_eq!(interner.search_idx(intern::INTERNED_U16 as usize), "u16");
        assert_eq!(interner.search_idx(intern::INTERNED_I32 as usize), "i32");
        assert_eq!(interner.search_idx(intern::INTERNED_U32 as usize), "u32");
        assert_eq!(interner.search_idx(intern::INTERNED_F32 as usize), "f32");
        assert_eq!(interner.search_idx(intern::INTERNED_I64 as usize), "i64");
        assert_eq!(interner.search_idx(intern::INTERNED_U64 as usize), "u64");
        assert_eq!(interner.search_idx(intern::INTERNED_F64 as usize), "f64");
        assert_eq!(interner.search_idx(intern::INTERNED_I128 as usize), "i128");
        assert_eq!(interner.search_idx(intern::INTERNED_U128 as usize), "u128");
        assert_eq!(interner.search_idx(intern::INTERNED_F128 as usize), "f128");
        assert_eq!(
            interner.search_idx(intern::INTERNED_SIZED as usize),
            "sized"
        );
        assert_eq!(
            interner.search_idx(intern::INTERNED_UNSIZED as usize),
            "unsized"
        );
        assert_eq!(interner.search_idx(intern::INTERNED_BOOL as usize), "bool");
        assert_eq!(interner.search_idx(intern::INTERNED_NIL as usize), "nil");
        assert_eq!(interner.search_idx(intern::INTERNED_CHAR as usize), "char");
        assert_eq!(interner.search_idx(intern::INTERNED_STR as usize), "str");
        assert_eq!(
            interner.search_idx(intern::INTERNED_BIGINT as usize),
            "BigInt"
        );
        assert_eq!(
            interner.search_idx(intern::INTERNED_BIGFLOAT as usize),
            "BigFloat"
        );
        assert_eq!(interner.search_idx(intern::INTERNED_LIST as usize), "List");
        assert_eq!(interner.search_idx(intern::INTERNED_SET as usize), "Set");
        assert_eq!(interner.search_idx(intern::INTERNED_MAP as usize), "Map");
    }

    #[test]
    fn interned_preload_size_matches() {
        let interner = Intern::init();
        assert_eq!(
            intern::INTERNED_CORE + 1,
            51,
            "INTERNER_PRELOAD_SIZE should match number of preloaded interned strings"
        );
    }
    // -- BUDGET --
    /// Makes diagnostics with as many `Default` values as possible of level `Error`
    fn make_diagnostics(amt: usize) -> Vec<SourceDiagnostic> {
        let mut diags = Vec::new();
        for i in 0..amt {
            diags.push(SourceDiagnostic::new(
                DiagnosticLevel::Error,
                Default::default(),
                PathId::new(i as u32),
                Default::default(),
                Default::default(),
                Default::default(),
            ));
        }
        diags
    }

    #[test]
    fn checked_consume_budget_tests() {
        // Overflow check
        let mut budget = MemoryBudget::default();
        budget.consume(1);
        assert!(matches!(
            budget.checked_consume(usize::MAX),
            BudgetResult::Overflow
        ));

        // Overage check
        let mut budget = MemoryBudget::new(10);
        assert!(matches!(
            budget.checked_consume(15),
            BudgetResult::Overage(5)
        ));

        // Should not have consumed anything since it was an overage
        assert_eq!(budget.remaining(), 10);

        // Limit Reached
        let mut budget = MemoryBudget::new(10);
        assert!(matches!(
            budget.checked_consume(10),
            BudgetResult::LimitReached,
        ));

        // Stable
        let mut budget = MemoryBudget::new(10);
        assert!(matches!(budget.checked_consume(9), BudgetResult::Stable,));
    }

    #[test]
    fn reporter_budget_test() {
        let mut reporter = Reporter::new(MemoryBudget::new(5));
        let res = reporter.append_safe(&mut make_diagnostics(5));
        assert_eq!(
            res, true,
            "Should only be LimitReached which should not return `false`"
        );
    }
}
