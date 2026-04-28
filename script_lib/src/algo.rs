use std::cmp;

use chrn_utils::{builtins, keywords};

pub(crate) enum FuzzyMatch {
    KW,
    Type,
    Sect,
    Stmt,
    Arg,
}

pub fn fuzzy_match(given: &[u8], target: FuzzyMatch) -> Option<&str> {
    match target {
        FuzzyMatch::KW => fuzzy_match_inner(given, &keywords::KEYWORDS_ARRAY),
        FuzzyMatch::Type => fuzzy_match_inner(given, &builtins::BUILTIN_TYPE_ARRAY),
        FuzzyMatch::Stmt => {
            fuzzy_match_inner(given, &keywords::KEYWORDS_ARRAY[keywords::stmt_range()])
        }
        FuzzyMatch::Sect => {
            fuzzy_match_inner(given, &keywords::KEYWORDS_ARRAY[keywords::sect_range()])
        }
        FuzzyMatch::Arg => fuzzy_match_inner(given, &chrn_utils::inner_args::ARGS_ARRAY),
    }
}

/// `given` represents the given bytes that are to be compared to the elements of `arr`.
// Returns option string instead of index because not all arrays are loaded at startup
fn fuzzy_match_inner<'a, 'b>(given: &'a [u8], arr: &'b [&str]) -> Option<&'b str> {
    // Calculating this in-line instead of constants due to it being prone to bugs
    let mut max_len = 0;

    for s in arr {
        if s.len() > max_len {
            max_len = s.len();
        }
    }

    if given.len() > max_len || given.len() == 1 {
        return None;
    }

    for (i, var) in arr.iter().enumerate() {
        let mut chances = 2;
        let mut matched = 0;

        let var_bytes = var.as_bytes();

        let size_diff =
            cmp::max(given.len(), var_bytes.len()) - cmp::min(given.len(), var_bytes.len());

        if size_diff > 2 {
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

        if matched > 3 || (matched >= 2 && matched + 1 >= var_bytes.len()) {
            // Since keywords exist now this may not be needed
            // if given == var_bytes {
            //     return None;
            // }

            // Similar
            return Some(arr[i]);
        }
    }

    None
}
