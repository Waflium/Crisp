pub fn strip_color(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\u{a7}' && chars.peek().is_some_and(|n| matches!(n.to_ascii_lowercase(), '0'..='9' | 'a'..='f' | 'k'..='o' | 'r')) {
            chars.next();
            continue;
        }
        out.push(c);
    }
    out
}

pub fn is_allowed_chat_character(ch: char) -> bool {
    ch != '\u{a7}' && ch >= ' ' && ch != '\u{7f}'
}

pub fn filter_text(input: &str) -> String {
    input.chars().filter(|&c| is_allowed_chat_character(c)).collect()
}

pub fn is_whitespace(codepoint: char) -> bool {
    codepoint.is_whitespace() && codepoint != '\u{85}' || matches!(codepoint, '\u{1c}'..='\u{1f}')
}

pub fn is_blank(string: Option<&str>) -> bool {
    string.is_none_or(|string| string.chars().all(is_whitespace))
}
