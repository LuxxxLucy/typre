use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

pub(crate) fn cell_width(s: &str) -> usize {
    UnicodeWidthStr::width(s)
}

pub(crate) fn break_units(s: &str) -> Vec<(&str, usize)> {
    let mut out = Vec::new();
    let mut start = 0;
    if s.is_ascii() {
        for (offset, byte) in s.bytes().enumerate() {
            if byte == b' ' {
                if start < offset {
                    out.push((&s[start..offset], cell_width(&s[start..offset])));
                }
                out.push((&s[offset..offset + 1], 1));
                start = offset + 1;
            }
        }
        if start < s.len() {
            out.push((&s[start..], cell_width(&s[start..])));
        }
        return out;
    }
    for (offset, grapheme) in s.grapheme_indices(true) {
        let width = cell_width(grapheme);
        if grapheme == " " || width >= 2 {
            if start < offset {
                let word = &s[start..offset];
                out.push((word, cell_width(word)));
            }
            out.push((grapheme, width));
            start = offset + grapheme.len();
        }
    }
    if start < s.len() {
        out.push((&s[start..], cell_width(&s[start..])));
    }
    out
}

pub(crate) fn split_word(s: &str, width: usize) -> Vec<&str> {
    let width = width.max(1);
    if cell_width(s) <= width {
        return vec![s];
    }
    if s.bytes().all(|byte| (b' '..=b'~').contains(&byte)) {
        return (0..s.len())
            .step_by(width)
            .map(|start| &s[start..(start + width).min(s.len())])
            .collect();
    }
    let mut out = Vec::new();
    let mut start = 0;
    for (offset, grapheme) in s.grapheme_indices(true) {
        let end = offset + grapheme.len();
        if cell_width(&s[start..end]) > width {
            if offset > start {
                out.push(&s[start..offset]);
                start = offset;
            }
            if cell_width(grapheme) > width {
                out.push("…");
                start = end;
            }
        }
    }
    if start < s.len() {
        out.push(&s[start..]);
    }
    out
}

pub(crate) fn wrap_text(s: &str, width: usize) -> Vec<String> {
    let width = width.max(1);
    let mut lines = Vec::new();
    let mut line = String::new();
    let mut columns = 0;
    for (unit, unit_width) in break_units(s) {
        if columns > 0 && columns + unit_width > width {
            lines.push(std::mem::take(&mut line));
            columns = 0;
            if unit == " " {
                continue;
            }
        }
        for part in split_word(unit, width) {
            let part_width = cell_width(part);
            if columns > 0 && columns + part_width > width {
                lines.push(std::mem::take(&mut line));
                columns = 0;
            }
            line.push_str(part);
            columns += part_width;
        }
    }
    if !line.is_empty() || lines.is_empty() {
        lines.push(line);
    }
    lines
}

pub(crate) fn truncate(s: &str, max: usize) -> String {
    if cell_width(s) <= max {
        return s.to_string();
    }
    if max == 0 {
        return String::new();
    }
    let mut end = 0;
    for (offset, grapheme) in s.grapheme_indices(true) {
        let next = offset + grapheme.len();
        if cell_width(&s[..next]) >= max {
            break;
        }
        end = next;
    }
    format!("{}…", &s[..end])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrapping_preserves_combining_marks_and_emoji() {
        assert_eq!(
            wrap_text("e\u{301}e\u{301}e\u{301}", 2),
            ["e\u{301}e\u{301}", "e\u{301}"]
        );
        assert_eq!(wrap_text("👩‍💻👩‍💻", 2), ["👩‍💻", "👩‍💻"]);
        assert_eq!(wrap_text("abcdefgh", 3), ["abc", "def", "gh"]);
    }

    #[test]
    fn truncation_uses_display_columns() {
        assert_eq!(truncate("中文中文", 5), "中文…");
        assert_eq!(truncate("e\u{301}xy", 2), "e\u{301}…");
        assert_eq!(truncate("x", 0), "");
    }

    #[test]
    fn wrapping_measures_ligatures_as_complete_prefixes() {
        assert_eq!(cell_width("لا"), 1);
        assert_eq!(split_word("لا", 1), ["لا"]);
        assert_eq!(split_word("لاx", 1), ["لا", "x"]);
        assert_eq!(wrap_text("لاxy", 2), ["لاx", "y"]);
        assert_eq!(truncate("لاxy", 2), "لا…");
    }
}
