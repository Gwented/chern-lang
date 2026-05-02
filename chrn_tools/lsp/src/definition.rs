pub fn find_in_source(text: &str, name: &str) -> Option<(usize, usize)> {
    let lines: Vec<&str> = text.lines().collect();
    let name_len = name.len();

    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();

        if trimmed.starts_with("let ") {
            let after_let = &trimmed[4..];
            if let Some(pos) = after_let.find(name) {
                if pos == 0
                    || after_let
                        .chars()
                        .nth(pos.saturating_sub(1))
                        .map(|c| c.is_alphanumeric())
                        .unwrap_or(false)
                {
                    let offset = text.lines().take(i).map(|l| l.len() + 1).sum::<usize>() + 4;
                    return Some((offset, offset + name_len));
                }
            }
        }

        if trimmed.starts_with("struct ") {
            let after_struct = &trimmed[7..];
            if after_struct.starts_with(name) {
                let offset = text.lines().take(i).map(|l| l.len() + 1).sum::<usize>() + 7;
                return Some((offset, offset + name_len));
            }
        }

        if trimmed.starts_with("enum ") {
            let after_enum = &trimmed[5..];
            if after_enum.starts_with(name) {
                let offset = text.lines().take(i).map(|l| l.len() + 1).sum::<usize>() + 5;
                return Some((offset, offset + name_len));
            }
        }

        if trimmed.starts_with("type ") {
            let after_type = &trimmed[5..];
            if let Some(pos) = after_type.find(name) {
                let offset = text.lines().take(i).map(|l| l.len() + 1).sum::<usize>() + 5;
                return Some((offset + pos, offset + pos + name_len));
            }
        }

        if trimmed.starts_with("alias ") {
            let after_alias = &trimmed[6..];
            if let Some(pos) = after_alias.find(name) {
                let offset = text.lines().take(i).map(|l| l.len() + 1).sum::<usize>() + 6;
                return Some((offset + pos, offset + pos + name_len));
            }
        }
    }

    None
}
