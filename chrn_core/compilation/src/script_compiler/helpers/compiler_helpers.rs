// Not really a deep distinction from core since core is compiler generated, but core is a library
// intrinsically needed by the compiler. Directives are not a part of a library of any kind, but
// what if it was?
// Beep

use chrn_utils::{id_types::InternedId, intern};
use lang::directives::{Directive, TypeDirective};

use crate::script_compiler::compiler_constants;

pub static DIRECTIVES_DATASET: [(InternedId, Directive);
    compiler_constants::DIRECTIVE_UNICODE_IDX + 1] = [
    (InternedId::new(intern::INTERNED_WARN), Directive::Warn),
    (InternedId::new(intern::INTERNED_IGNORE), Directive::Ignore),
    (
        InternedId::new(intern::INTERNED_SCIENT),
        Directive::Type(TypeDirective::Scient),
    ),
    (
        InternedId::new(intern::INTERNED_HEX),
        Directive::Type(TypeDirective::Hex),
    ),
    (
        InternedId::new(intern::INTERNED_BIN),
        Directive::Type(TypeDirective::Bin),
    ),
    (
        InternedId::new(intern::INTERNED_OCTAL),
        Directive::Type(TypeDirective::Octal),
    ),
    (
        InternedId::new(intern::INTERNED_UNICODE),
        Directive::Type(TypeDirective::Unicode),
    ),
];
