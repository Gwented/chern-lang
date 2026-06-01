use std::path::Path;

use chrn_utils::source_map::source_diagnostic::{AnnotationKind, DiagnosticLevel};
use common::color;

pub(super) fn get_diag_level_text(level: DiagnosticLevel) -> &'static str {
    match level {
        DiagnosticLevel::Error => "error",
        DiagnosticLevel::Warn => "warn",
        DiagnosticLevel::Help => "help",
        DiagnosticLevel::Note => "note",
    }
}

pub(super) fn get_diag_level_color(level: DiagnosticLevel) -> &'static str {
    match level {
        DiagnosticLevel::Error => {
            let (red, _) = color::get_red(true);
            red
        }
        DiagnosticLevel::Warn => {
            let (orange, _) = color::get_orange(true);
            orange
        }
        DiagnosticLevel::Help => {
            let (orange, _) = color::get_orange(true);
            orange
        }
        DiagnosticLevel::Note => {
            let (cyan, _) = color::get_cyan(true);
            cyan
        }
    }
}

pub(super) fn get_annotation_kind_text(kind: AnnotationKind) -> &'static str {
    match kind {
        AnnotationKind::Primary | AnnotationKind::Secondary => "",
        AnnotationKind::Note => "note",
        AnnotationKind::Help => "help",
    }
}

pub(super) fn get_annotation_kind_color(kind: AnnotationKind, can_color: bool) -> &'static str {
    match kind {
        AnnotationKind::Primary | AnnotationKind::Secondary => "",
        AnnotationKind::Note => {
            let (cyan, _) = color::get_cyan(can_color);
            cyan
        }
        AnnotationKind::Help => {
            let (orange, _) = color::get_orange(can_color);
            orange
        }
    }
}

// Nice name bud
pub(super) fn get_annotation_kind_ptr_color(kind: AnnotationKind, can_color: bool) -> &'static str {
    match kind {
        AnnotationKind::Primary => {
            let (red, _) = color::get_red(can_color);
            red
        }
        AnnotationKind::Secondary | AnnotationKind::Note => {
            // This is kind of hard to see without bold
            let (cyan, _) = color::get_bold_cyan(can_color);
            cyan
        }
        AnnotationKind::Help => {
            let (orange, _) = color::get_orange(can_color);
            orange
        }
    }
}

pub(super) fn get_annotation_kind_ptr(kind: AnnotationKind) -> &'static str {
    match kind {
        AnnotationKind::Primary => "^",
        AnnotationKind::Secondary | AnnotationKind::Note | AnnotationKind::Help => "-",
    }
}

pub(super) fn standardize_help(msg: &str, can_color: bool) -> String {
    let (orange, nc) = color::get_orange(can_color);

    if can_color {
        format!("{orange}help{nc}: {msg}\n")
    } else {
        format!("help: {msg}\n")
    }
}

pub(super) fn standardize_note(msg: &str, can_color: bool) -> String {
    let (cyan, nc) = color::get_cyan(can_color);

    if can_color {
        format!("{cyan}help{nc}: {msg}\n")
    } else {
        format!("note: {msg}\n")
    }
}

pub(super) fn create_diag_header(level: DiagnosticLevel, path: &Path, can_color: bool) -> String {
    let header_text = get_diag_level_text(level);

    let nc = color::NC;
    let header_color = if can_color {
        get_diag_level_color(level)
    } else {
        "".into()
    };

    let level_header = format!("{header_color}{header_text}{nc}");

    let (bold, nc) = color::get_bold(can_color);

    format!("{bold}PATH{nc} => \"{}\"\n{level_header}:", path.display())
}

// Probably needs to standardize, given a layout instead.
// Might not use this beyond ensuring!@#!#!
pub(super) fn standardize_err(path: &Path, can_color: bool) -> String {
    todo!()
    // let (red, nc) = color::get_red(can_color);
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
