use super::helpers::*;

/// Namespace-stage diagnostics for a single-module script.
fn ns_diags(text: &str) -> SourceDiagnosticSummary {
    resolve_single_module(text, Stage::Namespace).ns
}

#[test]
fn nameresolver_duplicate_simple_test() {
    // -- NEUTRAL --
    let wrong = "
            let DUPLICATE = 3
            let DUPLICATE = \"Hi\"
            ";

    let diags = ns_diags(wrong);

    assert!(
        !diags.diags.is_empty(),
        "Expected errors from NamespaceResolver"
    );

    let correct = "
                let ORIGINAL = 2 + 2
                let NEW = \"Hallo\"
            ";

    let diags = ns_diags(correct);

    assert!(
        diags.diags.is_empty(),
        "NamespaceResolver should have no errors: {:?}",
        diags
    );

    // -- VAR --
    let wrong = "
            var->
                duplicate: i32
                duplicate: i8
            ";

    // Doing this first since if modules were identified during the parsing stage any
    // syntax error within another module would not be reportable since the parser failed.
    let diags = ns_diags(wrong);

    assert!(
        !diags.diags.is_empty(),
        "Expected errors from NamespaceResolver"
    );

    let correct = "
            var->
                original: u32
                new: i8
            ";

    let diags = ns_diags(correct);

    assert!(
        diags.diags.is_empty(),
        "NamespaceResolver should have no errors: {:?}",
        diags
    );

    // -- NEST --

    let wrong = "
            nest->
                struct Duplicate {}
                struct Duplicate {}
            ";

    let diags = ns_diags(wrong);

    assert!(
        !diags.diags.is_empty(),
        "Expected errors from NamespaceResolver"
    );

    let correct = "
            nest->
                struct Original {}
                struct New {}
            ";

    let diags = ns_diags(correct);

    assert!(
        diags.diags.is_empty(),
        "NamespaceResolver should have no errors: {:?}",
        diags
    );
    //TEST: -- COMPLEX --

    //TEST: -- OVERRIDE --
}
