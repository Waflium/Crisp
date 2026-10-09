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
