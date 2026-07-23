use crate::config_loader::{ConfigLoader, ConfigLoaderOutput};

use super::helpers::*;
use chrn_utils::source_map::source_diagnostic::annotations::AnnotationKind;

#[test]
fn cfg_at_def_no_separator_before_at_end_test() {
    let res = load_cfg("@def@end").expect_success();
    assert_eq!(res.script_start, 0);
    assert_eq!(res.serial_start, Some(8));

    let res = load_cfg(" @def@end ").expect_success();
    assert_eq!(res.script_start, 1);
    assert_eq!(res.serial_start, Some(9));

    let res = load_cfg("@def \t@end\n\t").expect_success();
    assert_eq!(res.script_start, 0);
    assert_eq!(res.serial_start, Some(10));

    let res = load_cfg(" @def @end").expect_success();
    assert_eq!(res.script_start, 1);
    assert_eq!(res.serial_start, Some(10));

    let res = load_cfg(" @def @end ").expect_success();
    assert_eq!(res.script_start, 1);
    assert_eq!(res.serial_start, Some(10));

    let res = load_cfg("@def\t@\re\rnd");
    match res {
        ConfigLoaderOutput::Broken(region, ConfigLoadError::Diagnostic(diag)) => {
            assert_eq!(region.script_start, 0, "@def at offset 0 in broken case");
            assert!(region.serial_start.is_none(), "Broken => no serial_start");
            assert!(
                !diag.annotations.is_empty(),
                "diagnostic must annotate the @def span"
            );
        }
        other => panic!("Expected Broken with Diagnostic, got {other:?}"),
    }
}

/// -- OLD BEHAVIOR --
/// `@end` (4 bytes) appearing with no preceding `@def` must NOT terminate a script
/// block. The whole file is the script, and `@end` should be reported as plain text.
/// -- NEW BEHAVIOR --
/// `@end` is allowed is allowed to be used without `@def` so it is not treated as plain text.
/// Serial start is some since the conceptual idea of a serial start is just that a script block
/// in a file exists, with no actual guarantee of if there is actually serial data, which is
/// impossible to know from the loader's point of view.
#[test]
fn cfg_at_end_without_at_def_is_plain_text_test() {
    let res = load_cfg("@end").expect_success();
    assert_eq!(res.src_bytes, b"@end");
    // Is some since
    assert!(res.serial_start.is_some());
    assert_eq!(res.script_start, 0);
}

//NOTE: IT DOES NOT CARE ABOUT NUL BYTES (This is intended) May remove these tests

// /// A NUL byte (`\0`) anywhere in the file should terminate the loader's scan
// /// immediately, regardless of whether an `@def` is in progress.
// #[test]
// fn cfg_null_byte_terminates_scan_mid_file_test() {
//     // The bytes after the NUL are never observed, so the unclosed `@def` does NOT
//     // produce a "missing @end" diagnostic - the NUL is treated as the end of the script.
//     let res = load_cfg("@def var-> x: i32\0this would normally break things@end");
//     dbg!(&res);
//     assert!(
//         matches!(res, ConfigLoaderOutput::Broken(_, _)),
//         "NUL after @def should produce a Broken region, not silently swallow the missing-@end error."
//     );
// }
//
// /// A NUL byte at the very start of the file should produce an empty region.
// #[test]
// fn cfg_null_byte_at_start_test() {
//     let res = load_cfg("\0hello world").expect_success();
//     dbg!(&res.src_bytes);
//     assert_eq!(res.src_bytes, []);
//     assert!(res.serial_start.is_none());
// }

/// An `@` sign inside a double-quoted string must be treated as part of the string,
/// NOT as a marker. The string is consumed by `read_quotes` before the `@` arm is reached.
#[test]
fn cfg_at_sign_inside_string_is_not_a_marker_test() {
    // The string contains "@def" as text. The loader should report no error and treat the
    // string as opaque content of the script body (no @def was ever seen at the top level).
    let input = r#""this has @def inside it" remaining"#;
    let res = load_cfg(input).expect_success();
    assert_eq!(res.script_start, 0, "No @def was ever matched");
    assert!(res.serial_start.is_none(), "No @def was ever matched");
    assert_eq!(
        std::str::from_utf8(&res.src_bytes).unwrap(),
        input,
        "src_bytes must preserve the entire input verbatim"
    );
}

/// The substring `/*` inside a string must NOT be treated as a multi-line comment.
/// Confirms `read_quotes` fully consumes the string before any other branch fires.
#[test]
fn cfg_multi_comment_syntax_inside_string_is_not_comment_test() {
    let input = r#""/* still just text " trailing"#;
    let res = load_cfg(input).expect_success();
    assert!(res.serial_start.is_none());
    assert_eq!(
        std::str::from_utf8(&res.src_bytes).unwrap(),
        input,
        "src_bytes must preserve the entire input verbatim"
    );
}

/// `@def` written inside a `//` line comment must be ignored. The comment handler
/// advances until `\n`, so the `@` arm never sees this `@def`.
#[test]
fn cfg_at_def_inside_line_comment_is_ignored_test() {
    let input = "// @def @end\nreal code\n";
    let res = load_cfg(input).expect_success();
    assert!(
        res.serial_start.is_none(),
        "@def inside a // comment must not open a block"
    );
    assert_eq!(
        std::str::from_utf8(&res.src_bytes).unwrap(),
        input,
        "src_bytes must preserve the entire input verbatim"
    );
}

/// `@def` written inside a `/* */` multi-line comment must be ignored. Tests the
/// interaction between comment depth tracking and `@` matching.
#[test]
fn cfg_at_def_inside_multi_comment_is_ignored_test() {
    let input = "/* @def @end */\nreal\n";
    let res = load_cfg(input).expect_success();
    assert!(res.serial_start.is_none());
    assert_eq!(std::str::from_utf8(&res.src_bytes).unwrap(), input);

    let input = "/*@def @end*/\nreal\n";
    let res = load_cfg(input).expect_success();
    assert!(res.serial_start.is_none());
    assert_eq!(std::str::from_utf8(&res.src_bytes).unwrap(), input);

    let input = "/*@def@end*/\r\nreal\n\x25";
    let res = load_cfg(input).expect_success();
    assert!(res.serial_start.is_none());
    assert_eq!(std::str::from_utf8(&res.src_bytes).unwrap(), input);
}

/// A backslash escape inside a string must skip the next byte verbatim, so `"a\b"`
/// closes at the second `"` and the `\b` is part of the string content. This catches
/// off-by-one bugs in `read_quotes` where the escape could consume the closing quote.
#[test]
fn cfg_escape_sequence_in_string_test() {
    // Content: "a\b"  — the \b is an escape; the closing " is at index 4.
    let input = r#""a\b" after"#;
    let res = load_cfg(input).expect_success();
    assert!(res.serial_start.is_none());
    assert_eq!(
        std::str::from_utf8(&res.src_bytes).unwrap(),
        input,
        "src_bytes must preserve the entire input verbatim"
    );
}

/// A string opened with `"` and never closed must produce an unclosed-quotes error.
/// The diagnostic should point to the opening quote location.
#[test]
fn cfg_unclosed_double_quote_errors_test() {
    // "hello "world" → the `"` before `world` opens an unclosed string.
    // The opening quote sits at byte 6 of the input.
    let res = load_cfg("hello \"world");
    match res {
        ConfigLoaderOutput::Broken(_, ConfigLoadError::Diagnostic(diag)) => {
            let primary = diag
                .annotations
                .iter()
                .find(|a| a.kind == AnnotationKind::Primary)
                .expect("diagnostic should have a primary annotation");
            assert_eq!(
                primary.span.start, 6,
                "span should point at the opening double-quote"
            );
            assert_eq!(primary.span.end, 7, "span should be exactly 1 byte");
        }
        other => panic!("Expected unclosed-quote error with Diagnostic, got {other:?}"),
    }
}

/// A string opened with `'` and never closed must produce an unclosed-quotes error.
/// The diagnostic should point to the opening quote location.
#[test]
fn cfg_unclosed_single_quote_errors_test() {
    // "hello 'world" → the `'` before `world` opens an unclosed character/string.
    // The opening quote sits at byte 6 of the input.
    let res = load_cfg("hello 'world");
    match res {
        ConfigLoaderOutput::Broken(_, ConfigLoadError::Diagnostic(diag)) => {
            let primary = diag
                .annotations
                .iter()
                .find(|a| a.kind == AnnotationKind::Primary)
                .expect("diagnostic should have a primary annotation");
            assert_eq!(
                primary.span.start, 6,
                "span should point at the opening single-quote"
            );
            assert_eq!(primary.span.end, 7, "span should be exactly 1 byte");
        }
        other => panic!("Expected unclosed-quote error with Diagnostic, got {other:?}"),
    }
}

/// A backslash at the very end of the file, inside a string, must cause the string
/// to be considered unclosed. The escape handler does `self.skip(2)`, so a trailing `\`
/// runs off the buffer and `read_quotes` returns `Err`.
#[test]
fn cfg_escape_at_eof_in_string_errors_test() {
    // Input: `"abc\` — opening `"` at byte 0, backslash at byte 4 (EOF).
    let res = load_cfg(r#""abc\"#);
    match res {
        ConfigLoaderOutput::Broken(_, ConfigLoadError::Diagnostic(diag)) => {
            let primary = diag
                .annotations
                .iter()
                .find(|a| a.kind == AnnotationKind::Primary)
                .expect("diagnostic should have a primary annotation");
            assert_eq!(
                primary.span.start, 0,
                "span should point at the opening double-quote"
            );
            assert_eq!(primary.span.end, 1, "span should be exactly 1 byte");
        }
        other => panic!("Expected unclosed-quote error with Diagnostic, got {other:?}"),
    }
}

/// An empty input should yield a valid empty region with no serial start and a
/// script_start of 0. This is the canonical "no markers at all" case.
#[test]
fn cfg_empty_file_test() {
    let res = load_cfg("").expect_success();
    assert_eq!(res.src_bytes, []);
    assert_eq!(res.script_start, 0);
    assert!(res.serial_start.is_none());
}

/// `\r\n` (Windows) line endings must behave the same as `\n`. The line-comment
/// handler stops at `\n`, but a stray `\r` should not cause issues. This catches any
/// accidental `\n`-only termination.
#[test]
fn cfg_crlf_line_endings_test() {
    // Comment then real content with CRLF separators.
    let input = "\r//\r\r\r header\r\nlet A = 1\r\nlet B = 2\r\n";
    let res = load_cfg(input).expect_success();
    assert!(res.serial_start.is_none());
    assert_eq!(res.script_start, 0, "no @def seen");
    assert_eq!(
        std::str::from_utf8(&res.src_bytes).unwrap(),
        input,
        "src_bytes must preserve the entire input verbatim"
    );
}

/// A bare `@` at the end of the file with `requires_end == false` triggers the
/// `!can_check` short-circuit branch which skips the remaining bytes and breaks. This
/// must not panic and must report no error (no `@def` was opened).
#[test]
fn cfg_lone_at_sign_at_eof_test() {
    let res = load_cfg("some text @").expect_success();
    assert!(res.serial_start.is_none());
    assert_eq!(res.script_start, 0);
    assert_eq!(std::str::from_utf8(&res.src_bytes).unwrap(), "some text @");
}

/// A long run of `@` characters in normal text must all be consumed as individual
/// `@` tokens, none of which form `@def` or `@end`. Verifies that the `@` arm's
/// `self.advance()` covers the case where neither annotation matches.
#[test]
fn cfg_many_at_signs_in_a_row_test() {
    let input = "@@@@@@@@@@@@ plain@ @ text @@@@@@@@@@@@";
    let res = load_cfg(input).expect_success();
    assert!(res.serial_start.is_none());
    assert_eq!(res.script_start, 0);
    assert_eq!(std::str::from_utf8(&res.src_bytes).unwrap(), input);
}

/// A file containing only a multi-line comment that never closes must report an
/// unclosed multi-line comment error. The handler tracks depth and produces a diagnostic
/// pointing at the start of the comment.
#[test]
fn cfg_unclosed_multi_line_comment_test() {
    let res = load_cfg("/* this comment never ends");
    match res {
        ConfigLoaderOutput::UnrecoverableErr(ConfigLoadError::Diagnostic(diag)) => {
            // The diagnostic should carry *two* annotations: a secondary pointing at the
            // opening `// ` (byte 0) and a primary pointing at EOF (last byte).
            let secondaries: Vec<_> = diag
                .annotations
                .iter()
                .filter(|a| a.kind == AnnotationKind::Secondary)
                .collect();
            let primaries: Vec<_> = diag
                .annotations
                .iter()
                .filter(|a| a.kind == AnnotationKind::Primary)
                .collect();
            assert_eq!(
                secondaries.len(),
                1,
                "one secondary annotation for comment start"
            );
            assert_eq!(
                secondaries[0].span.start, 0,
                "secondary points at opening `/`"
            );
            assert_eq!(primaries.len(), 1, "one primary annotation for EOF");
        }
        other => panic!("UnrecoverableErr with Diagnostic expected, got {other:?}"),
    }
}

/// Tab characters must be treated as ordinary bytes by the loader — they are not
/// treated as whitespace specially (the lexer would normalize later, but the loader
/// must not skip or mis-handle them). This test interleaves tabs with `@def` and `@end`
/// separated by tabs only to confirm the byte scanner does not confuse tab with newline.
#[test]
fn cfg_tab_characters_around_at_def_test() {
    // Use tabs (not spaces, not newlines) between @def and @end.
    // :crab:
    let res = load_cfg("\t@def\tva\nr->\tx:\ti3\r2\t\r\u{32}@end\t");
    match res {
        ConfigLoaderOutput::Success(region) => {
            // The leading tab (byte 0) is serial; @def starts at byte 1.
            assert_eq!(region.script_start, 1);
            // @end starts at byte 23; serial_start = 23 + 4 = 27.
            assert_eq!(region.serial_start, Some(27));
            let s = std::str::from_utf8(&region.src_bytes).unwrap();
            assert!(s.contains("@def"));
            assert!(s.contains("@end"));
        }
        other => {
            // If the loader rejects it, the rejection should be about the actual
            // content (missing @end, etc.) — never a panic from a confused byte position.
            panic!("Loader errored on tab-separated @def/@end: {other:?}");
        }
    }
}

#[test]
fn multi_line_comment_test() {
    // Properly closed multi-line comment
    let correct_input = "
            /* /* */ */
        "
    .as_bytes();

    // Unclosed multi-line comment
    let wrong_input = "
            /* /* */
        "
    .as_bytes();

    let interner = mock_interner(0, 2);
    let region_id = SourceRegionId::new(0);

    let correct = ConfigLoader::new(
        region_id,
        correct_input,
        PathId::default(),
        &ChrnConfig::default(),
        &interner,
    )
    .load_config();

    let wrong = ConfigLoader::new(
        region_id,
        wrong_input,
        PathId::default(),
        &ChrnConfig::default(),
        &interner,
    )
    .load_config();

    let correct_region = correct.expect_success();
    assert_eq!(
        correct_region.src_bytes, correct_input,
        "correct multi-comment: src_bytes must match input verbatim"
    );

    match wrong {
        ConfigLoaderOutput::UnrecoverableErr(ConfigLoadError::Diagnostic(diag)) => {
            let primaries: Vec<_> = diag
                .annotations
                .iter()
                .filter(|a| a.kind == AnnotationKind::Primary)
                .collect();
            assert!(
                !primaries.is_empty(),
                "unclosed multi-comment diagnostic needs a primary annotation"
            );
        }
        other => {
            panic!("unclosed multi-line comment should produce UnrecoverableErr, got {other:?}")
        }
    }
}

#[test]
fn start_and_serial_offset_test() {
    let text = format!("adwh@def var-> int: i32 @endhi");
    let interner = mock_interner(0, 1);
    let region_id = SourceRegionId::new(0);

    let metadata = ConfigLoader::new(
        region_id,
        text.as_bytes(),
        PathId::default(),
        &ChrnConfig::default(),
        &interner,
    )
    .load_config()
    .expect_success();

    assert_eq!(&text[4..], &text[metadata.script_start..]);
    assert_eq!("hi", &text[metadata.serial_start.unwrap()..]);
    assert_eq!(28, metadata.serial_start.unwrap());
}
