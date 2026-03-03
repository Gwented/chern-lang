use crate::symbols::Span;

//FIX: ANSI
const RED: &str = "\x1b[31m";
const ORANGE: &str = "\x1b[33m";
const NC: &str = "\x1b[0m";

//TODO: Handle multi-line errors
pub fn form_diagnostic(text: &[u8], span: &Span, can_color: bool) -> (usize, usize, String) {
    let mut ln = 1;
    let mut col = 1;

    let mut b: u8;

    let mut seg_start = 0;

    // TODO:
    // Read as char
    for i in 0..span.end {
        b = if text[i].is_ascii() {
            text[i]
        } else {
            todo!("UTF-8 only supported inside of literal");
        };
        // b = self.original_text[i];

        //TODO: See if this works on windows
        //I still haven't checked.
        if b == b'\r' && text.get(i + 1).copied() == Some(b'\n') {
            ln += 1;
            // Offset to skip new line since cannot alter for loop counter directly
            // Should likely just manually loop to avoid odditiy
            seg_start = i + 2;
            col = 1;
        } else if b == b'\n' {
            ln += 1;
            seg_start = i + 1;
            col = 1;
        } else {
            col += 1;
        }
    }

    // Needs offset or will print span.end when span.start is more informational
    col -= span.end - span.start;

    let seg_end = get_line_end(text, seg_start);

    let segment = &text[seg_start..seg_end];

    dbg!(seg_start, seg_end);

    dbg!(str::from_utf8(segment).unwrap());

    //FIX: Should calculate by characters for UTF-1000
    let segment = str::from_utf8(segment)
        .expect("[temp] Invalid UTF-8 although would be impossible after lexer");

    // Span range is inclusive exclusive so final character is missed otherwise
    // Has no other mathematical outside of this
    let span_diff_offset = span.end - span.start + 1;

    let arrows = "^".repeat(span_diff_offset);

    // Spaces need to be proportional to the current line's size therefore it must
    // stay inside the range.
    let space_offset = text[seg_start..span.start].len();

    let spaces = " ".repeat(space_offset);

    let fmt_segment = if can_color {
        format!("\t{segment}\n\t{spaces}{RED}{arrows}{NC}")
    } else {
        format!("\t{segment}\n\t{spaces}{arrows}")
    };

    println!("{}", &fmt_segment);

    (ln, col, fmt_segment)
}

fn get_line_end(original_text: &[u8], start: usize) -> usize {
    for i in start..original_text.len() {
        let b = original_text[i];

        if b == b'\r' && original_text.get(i + 1).copied() == Some(b'\n') {
            return i;
        } else if b == b'\n' {
            return i;
        }
    }

    //WARN: I don't remember why I returned this
    original_text.len()
}

//TODO: Should this exist?
// pub fn format_segment(segment: &str) -> String {
//
// }

pub fn form_help(msg: &str, can_color: bool) -> String {
    if can_color {
        format!("{ORANGE}Help{NC}: {msg}\n")
    } else {
        format!("Help: {msg}\n")
    }
}
