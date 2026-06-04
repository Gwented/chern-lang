use std::path::Path;

use chrn_utils::source_map::source_diagnostic::{AnnotationKind, DiagnosticLevel};
use common::color::{self, TerminalColorType};

/// Returns the text that corresponds with a given diagnostic level
pub(super) fn get_diag_level_text(level: DiagnosticLevel) -> &'static str {
    match level {
        DiagnosticLevel::Error => "error",
        DiagnosticLevel::Warn => "warn",
        DiagnosticLevel::Help => "help",
        DiagnosticLevel::Note => "note",
    }
}

/// Returns the color code that corresponds to the given diagnostic level
pub(super) fn get_diag_level_color(
    level: DiagnosticLevel,
    can_color: bool,
    terminal: TerminalColorType,
) -> String {
    match level {
        DiagnosticLevel::Error => {
            let (red, _) = color::get_red(can_color, terminal);
            red
        }
        DiagnosticLevel::Warn => {
            let (orange, _) = color::get_orange(can_color, terminal);
            orange
        }
        DiagnosticLevel::Help => {
            let (orange, _) = color::get_orange(can_color, terminal);
            orange
        }
        DiagnosticLevel::Note => {
            let (cyan, _) = color::get_cyan(can_color, terminal);
            cyan
        }
    }
}

// Might not use this
/// Returns the text associated with an annotation label, if present
pub(super) fn get_annotation_kind_text(kind: AnnotationKind) -> &'static str {
    match kind {
        AnnotationKind::Primary | AnnotationKind::Secondary => "",
        AnnotationKind::Note => "note",
        AnnotationKind::Help => "help",
    }
}

/// Returns the color code to be given to a label, if any
pub(super) fn get_annotation_kind_color(
    kind: AnnotationKind,
    can_color: bool,
    terminal: TerminalColorType,
) -> String {
    match kind {
        AnnotationKind::Primary | AnnotationKind::Secondary => String::new(),
        AnnotationKind::Note => {
            let (cyan, _) = color::get_cyan(can_color, terminal);
            cyan
        }
        AnnotationKind::Help => {
            let (orange, _) = color::get_orange(can_color, terminal);
            orange
        }
    }
}

/// Returns the color code to be given to a set of pointers, given an `AnnotationKind`
pub(super) fn get_annotation_kind_ptr_color(
    kind: AnnotationKind,
    can_color: bool,
    terminal: TerminalColorType,
) -> String {
    match kind {
        AnnotationKind::Primary => {
            let (red, _) = color::get_red(can_color, terminal);
            red
        }
        AnnotationKind::Secondary | AnnotationKind::Note => {
            // This is kind of hard to see without bold
            let (cyan, _) = color::get_bold_cyan(can_color, terminal);
            cyan
        }
        AnnotationKind::Help => {
            let (orange, _) = color::get_orange(can_color, terminal);
            orange
        }
    }
}

/// Returns the pointer type according to an `AnnotationKind`
pub(super) fn get_annotation_kind_ptr(kind: AnnotationKind) -> &'static str {
    match kind {
        AnnotationKind::Primary => "^",
        AnnotationKind::Secondary | AnnotationKind::Note | AnnotationKind::Help => "-",
    }
}

pub(super) fn standardize_help(msg: &str, can_color: bool, terminal: TerminalColorType) -> String {
    let (orange, nc) = color::get_orange(can_color, terminal);

    if can_color {
        format!("{orange}help{nc}: {msg}")
    } else {
        format!("help: {msg}")
    }
}

pub(super) fn standardize_note(msg: &str, can_color: bool, terminal: TerminalColorType) -> String {
    let (cyan, nc) = color::get_cyan(can_color, terminal);

    if can_color {
        format!("{cyan}note{nc}: {msg}")
    } else {
        format!("note: {msg}")
    }
}

/// Creates a template header with the path and diagnostic level given
pub(super) fn create_diag_header(
    level: DiagnosticLevel,
    path: &Path,
    can_color: bool,
    terminal: TerminalColorType,
) -> String {
    let header_text = get_diag_level_text(level);

    let nc = color::NC;
    let header_color = get_diag_level_color(level, can_color, terminal);

    let level_header = format!("{header_color}{header_text}{nc}");

    let (bold, nc) = color::get_bold(can_color);

    format!("{bold}PATH{nc} => \"{}\"\n{level_header}:", path.display())
}

// Not sure about this anymore but might use it
pub(super) fn standardize_err(path: &Path, can_color: bool, terminal: TerminalColorType) -> String {
    let _ = terminal;
    todo!()
    // let (red, nc) = color::get_red(can_color, terminal);
    // let header = format!("From path => \"{}\"\n{red}error{nc}:", path.display());
    // let help = help.unwrap_or_default();
    //
    // Probably stays the same other than the help and notes being printed as multiple if possible
    // format!(
    //     "{header} {base_msg}\n[{}:{}]\n{}\n{help}{note}{}",
    //     line_data.ln,
    //     line_data.col,
    //     line_data.diag,
    //     "-".repeat(TOTAL_SEPARATORS)
    // )
}
