use std::cmp;

use crate::{
    directives,
    fmter::{Formattable, Formatted},
    keywords,
    types::builtins,
};

/// Fuzzy kiwis
#[derive(Clone, Copy)]
pub enum FuzzyMatch {
    KW,
    Type,
    Sect,
    Stmt,
    Directive,
}

impl Formattable for FuzzyMatch {
    fn to_fmt(&self) -> crate::fmter::Formatted {
        match self {
            FuzzyMatch::KW => Formatted::KW,
            FuzzyMatch::Type => Formatted::Type,
            FuzzyMatch::Sect => Formatted::AbstractSection,
            FuzzyMatch::Stmt => Formatted::Stmt,
            FuzzyMatch::Directive => Formatted::Directive,
        }
    }
}

// This is a lie.
/// Executes same search as the function `fuzzy_match`
///
/// Returns an `Option` in comparison to `fuzzy_match` because the target's string `Formatted`
/// version may or may not be relevant to return
pub fn fuzzy_match_with_fmtted(given: &[u8], target: FuzzyMatch) -> Option<(Vec<&str>, Formatted)> {
    Some((fuzzy_match(given, target), target.to_fmt()))
}

/// Fuzzily searches stuff
pub fn fuzzy_match(given: &[u8], target: FuzzyMatch) -> Vec<&str> {
    match target {
        //TODO: Update this!
        FuzzyMatch::KW => fuzzy_match_inner(given, &keywords::KEYWORDS_ARRAY),
        FuzzyMatch::Type => fuzzy_match_inner(given, &builtins::BUILTIN_TYPE_ARRAY),
        FuzzyMatch::Stmt => {
            fuzzy_match_inner(given, &keywords::KEYWORDS_ARRAY[keywords::stmt_range()])
        }
        FuzzyMatch::Sect => {
            fuzzy_match_inner(given, &keywords::KEYWORDS_ARRAY[keywords::sect_range()])
        }
        FuzzyMatch::Directive => fuzzy_match_inner(given, &directives::BUILTIN_DIRECTIVE_STRS),
    }
}

/// `given` represents the given bytes that are to be compared to the elements of `arr`.
// Returns option string instead of index because not all arrays are loaded at startup
fn fuzzy_match_inner<'a>(given: &[u8], arr: &'a [&str]) -> Vec<&'a str> {
    let mut found = Vec::new();

    // Calculating this in-line instead of constants due to it being prone to bugs
    let mut max_len = 0;
    for s in arr {
        if s.len() > max_len {
            max_len = s.len();
        }
    }

    if given.len() > max_len || given.len() == 1 {
        return found;
    }

    for (i, var) in arr.iter().enumerate() {
        let mut chances = 2;
        let mut matched = 0;

        let var_bytes = var.as_bytes();

        let size_diff = given.len().max(var_bytes.len()) - given.len().min(var_bytes.len());

        if size_diff > 3 {
            continue;
        }

        let cap = cmp::min(given.len(), var_bytes.len());

        for j in 0..cap {
            if given[j] == var_bytes[j] {
                matched += 1;
                chances = 1;
            } else if chances == 0 {
                break;
            } else {
                chances -= 1;
            }
        }

        // How about len dependent matching?
        // Edit distance checking?
        if matched > 2 || (matched >= 2 && matched + 1 >= var_bytes.len()) {
            found.push(arr[i]);
        }
    }

    found
}

// IGNORE THIS
// #[macro_export]
// macro_rules! find_similar {
//     ($bytes:expr, $fuzzy_match:expr) => {
//         if true {
//             let found = $crate::algo::fuzzy_match($bytes, $fuzzy_match);
//             if !found.is_empty() {
//                 (found, Some("hi"))
//             } else {
//                 (found, None)
//             }
//         } else {
//             unreachable!()
//         }
//     };
// }
