use std::path::Path;

use chrn_utils::source_map::source_diagnostic::{AnnotationKind, DiagnosticLevel};
use common::color::{self, TerminalColorType};

use crate::renderer::render_settings::RenderSettings;

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
pub(super) fn get_diag_level_color(level: DiagnosticLevel, settings: &RenderSettings) -> String {
    match level {
        DiagnosticLevel::Error => {
            let (red, _) = color::get_red(settings.can_color, settings.terminal_type);
            red
        }
        DiagnosticLevel::Warn | DiagnosticLevel::Help => {
            let (orange, _) = color::get_orange(settings.can_color, settings.terminal_type);
            orange
        }

        DiagnosticLevel::Note => {
            let (cyan, _) = color::get_cyan(settings.can_color, settings.terminal_type);
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

/// Formats a help message with heuristic styling
pub(super) fn standardize_help(msg: &str, can_color: bool, terminal: TerminalColorType) -> String {
    let (orange, nc) = color::get_orange(can_color, terminal);

    if can_color {
        format!("{orange}help{nc}: {msg}")
    } else {
        format!("help: {msg}")
    }
}

/// Formats a note message with heuristic styling
pub(super) fn standardize_note(msg: &str, can_color: bool, terminal: TerminalColorType) -> String {
    let (cyan, nc) = color::get_cyan(can_color, terminal);

    if can_color {
        format!("{cyan}note{nc}: {msg}")
    } else {
        format!("note: {msg}")
    }
}

/// Creates a template header with with given path
pub(super) fn create_path_header(path: &Path, settings: &RenderSettings) -> String {
    let (bold, nc) = color::get_bold(settings.can_color);
    format!("{bold}PATH{nc} => \"{}\"", path.display())
}

/// Creates a template header with the diagnostic level and msg given
pub(super) fn create_level_header(
    level: DiagnosticLevel,
    msg: &str,
    settings: &RenderSettings,
) -> String {
    let header_text = get_diag_level_text(level);

    let nc = color::get_nc(settings.can_color);
    let header_color = get_diag_level_color(level, settings);

    let level_header = format!("{header_color}{header_text}{nc}");

    format!("{level_header}: {msg}")
}

// Not sure about this anymore but might use it
// /// Creates a full basic error message
// pub(super) fn standardize_err(
//     level: DiagnosticLevel,
//     msg: &str,
//     path: &Path,
//     settings: &RenderSettings,
// ) -> String {
//     let header_text = get_diag_level_text(level);
//
//     let nc = color::NC;
//     let header_color = get_diag_level_color(level, settings);
//
//     let level_header = format!("{header_color}{header_text}{nc}");
//     let path_header = create_path_header(path, settings);
//
//     let (bold, nc) = color::get_bold(settings.can_color);
//
//     format!("{path_header}\n{level_header}: {msg}")
// }
