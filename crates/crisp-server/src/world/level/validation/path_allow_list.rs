use std::path::Path;
use std::sync::OnceLock;

use regex_lite::Regex;

const LINE_SEPARATOR: &str = if cfg!(windows) { "\r\n" } else { "\n" };
const NAME_SEPARATOR: char = if cfg!(windows) { '\\' } else { '/' };

#[derive(Debug)]
enum EntryType {
    FileSystem,
    Prefix,
}

#[derive(Debug)]
pub struct ConfigEntry {
    entry_type: EntryType,
    pattern: String,
}

impl ConfigEntry {
    fn parse(definition: &str) -> Result<Option<ConfigEntry>, String> {
        if definition.chars().all(char::is_whitespace) || definition.starts_with('#') {
            return Ok(None);
        }
        if !definition.starts_with('[') {
            return Ok(Some(ConfigEntry { entry_type: EntryType::Prefix, pattern: definition.to_string() }));
        }
        let Some(split) = definition[1..].find(']').map(|i| i + 1) else {
            return Err(format!("java.lang.IllegalArgumentException: Unterminated type in line '{definition}'"));
        };
        let contents = &definition[split + 1..];
        match &definition[1..split] {
            entry_type @ ("glob" | "regex") => Ok(Some(ConfigEntry { entry_type: EntryType::FileSystem, pattern: format!("{entry_type}:{contents}") })),
            "prefix" => Ok(Some(ConfigEntry { entry_type: EntryType::Prefix, pattern: contents.to_string() })),
            _ => Err(format!("java.lang.IllegalArgumentException: Unsupported definition type in line '{definition}'")),
        }
    }

    fn compile(&self) -> Result<Matcher, String> {
        match self.entry_type {
            EntryType::Prefix => Ok(Matcher::Prefix(self.pattern.clone())),
            EntryType::FileSystem => {
                let (syntax, input) = self.pattern.split_once(':').expect("a syntax");
                let expression = if syntax == "glob" { to_regex_pattern(input)? } else { format!("^(?:{input})$") };
                let expression = if cfg!(windows) { format!("(?i){expression}") } else { expression };
                Regex::new(&expression).map(Matcher::Regex).map_err(|e| format!("java.util.regex.PatternSyntaxException: {e}"))
            }
        }
    }
}

#[derive(Debug)]
enum Matcher {
    Prefix(String),
    Regex(Regex),
}

impl Matcher {
    fn matches(&self, path: &str) -> bool {
        match self {
            Matcher::Prefix(prefix) => path.starts_with(prefix.as_str()),
            Matcher::Regex(regex) => regex.is_match(path),
        }
    }
}

#[derive(Debug)]
pub struct PathAllowList {
    entries: Vec<ConfigEntry>,
    compiled: OnceLock<Vec<Matcher>>,
}

impl PathAllowList {
    pub fn read_plain(text: &str) -> Result<PathAllowList, String> {
        let mut entries = Vec::new();
        for line in text.split("\r\n").flat_map(|l| l.split(['\r', '\n'])) {
            entries.extend(ConfigEntry::parse(line)?);
        }
        Ok(PathAllowList { entries, compiled: OnceLock::new() })
    }

    pub fn matches(&self, path: &Path) -> bool {
        let compiled = self.compiled.get_or_init(|| match self.entries.iter().map(ConfigEntry::compile).collect::<Result<Vec<_>, _>>() {
            Ok(matchers) => matchers,
            Err(e) => {
                error!("Failed to compile file pattern list\n{e}");
                Vec::new()
            }
        });
        let path = path.to_string_lossy();
        compiled.iter().any(|m| m.matches(&path))
    }
}

fn syntax_error(description: &str, pattern: &str, index: usize) -> String {
    let mut message = format!("java.util.regex.PatternSyntaxException: {description} near index {index}{LINE_SEPARATOR}{pattern}");
    if index < pattern.chars().count() {
        message.push_str(LINE_SEPARATOR);
        message.push_str(&" ".repeat(index));
        message.push('^');
    }
    message
}

fn literal(c: char) -> String {
    regex_lite::escape(c.encode_utf8(&mut [0; 4]))
}

fn case_variants(c: char) -> impl Iterator<Item = char> {
    let variants: [String; 2] = if cfg!(windows) { [c.to_lowercase().collect(), c.to_uppercase().collect()] } else { Default::default() };
    variants.into_iter().filter_map(move |variant| {
        let mut chars = variant.chars();
        match (chars.next(), chars.next()) {
            (Some(v), None) if v != c => Some(v),
            _ => None,
        }
    })
}

fn folded_literal(c: char) -> String {
    if case_variants(c).next().is_some() { class(&[(c, c)], false) } else { literal(c) }
}

fn class(ranges: &[(char, char)], negated: bool) -> String {
    let separator = NAME_SEPARATOR as u32;
    let mut folded: Vec<(u32, u32)> = ranges.iter().map(|&(a, b)| (a as u32, b as u32)).collect();
    for &(a, b) in ranges {
        for c in a..=b {
            folded.extend(case_variants(c).map(|v| (v as u32, v as u32)));
        }
    }
    let mut members: Vec<(u32, u32)> = Vec::new();
    if negated {
        members.push((separator, separator));
        members.extend(folded);
    } else {
        for (a, b) in folded {
            if a <= separator && separator <= b {
                if a < separator {
                    members.push((a, separator - 1));
                }
                if separator < b {
                    members.push((separator + 1, b));
                }
            } else {
                members.push((a, b));
            }
        }
        if members.is_empty() {
            return "[^\\s\\S]".into();
        }
    }
    let mut result = String::from(if negated { "[^" } else { "[" });
    for (a, b) in members {
        result.push_str(&literal(char::from_u32(a).expect("a character")));
        if b != a {
            result.push('-');
            result.push_str(&literal(char::from_u32(b).expect("a character")));
        }
    }
    result.push(']');
    result
}

fn to_regex_pattern(glob: &str) -> Result<String, String> {
    let chars: Vec<char> = glob.chars().collect();
    let next = |i: usize| chars.get(i).copied().unwrap_or('\0');
    let java_separator = if cfg!(windows) { "\\\\" } else { "/" };
    let not_separator = class(&[], true);
    let mut in_group = false;
    let mut empty_class = false;
    let mut regex = String::from("^");
    let mut java = String::from("^");
    let mut i = 0;
    while i < chars.len() {
        let mut c = chars[i];
        i += 1;
        match c {
            '\\' => {
                if i == chars.len() {
                    return Err(syntax_error("No character to escape", glob, i - 1));
                }
                if is_glob_meta(chars[i]) || is_regex_meta(chars[i]) {
                    java.push('\\');
                }
                java.push(chars[i]);
                regex.push_str(&folded_literal(chars[i]));
                i += 1;
            }
            '/' => {
                java.push_str(java_separator);
                regex.push_str(&literal(NAME_SEPARATOR));
            }
            '[' => {
                java.push_str(&format!("[[^{java_separator}]&&["));
                let mut negated = false;
                let mut ranges: Vec<(char, char)> = Vec::new();
                if next(i) == '^' {
                    java.push_str("\\^");
                    ranges.push(('^', '^'));
                    i += 1;
                } else {
                    if next(i) == '!' {
                        java.push('^');
                        negated = true;
                        i += 1;
                    }
                    if next(i) == '-' {
                        java.push('-');
                        ranges.push(('-', '-'));
                        i += 1;
                    }
                }
                let mut has_range_start = false;
                let mut last = '\0';
                while i < chars.len() {
                    c = chars[i];
                    i += 1;
                    if c == ']' {
                        break;
                    }
                    if c == '/' || (cfg!(windows) && c == '\\') {
                        return Err(syntax_error("Explicit 'name separator' in class", glob, i - 1));
                    }
                    if c == '\\' || c == '[' || (c == '&' && next(i) == '&') {
                        java.push('\\');
                    }
                    java.push(c);
                    if c == '-' {
                        if !has_range_start {
                            return Err(syntax_error("Invalid range", glob, i - 1));
                        }
                        c = next(i);
                        i += 1;
                        if c == '\0' || c == ']' {
                            ranges.push(('-', '-'));
                            break;
                        }
                        if c < last {
                            return Err(syntax_error("Invalid range", glob, i - 3));
                        }
                        java.push(c);
                        ranges.pop();
                        ranges.push((last, c));
                        has_range_start = false;
                    } else {
                        ranges.push((c, c));
                        has_range_start = true;
                        last = c;
                    }
                }
                if c != ']' {
                    return Err(syntax_error("Missing ']", glob, i - 1));
                }
                java.push_str("]]");
                empty_class |= ranges.is_empty();
                regex.push_str(&class(&ranges, negated));
            }
            '{' => {
                if in_group {
                    return Err(syntax_error("Cannot nest groups", glob, i - 1));
                }
                java.push_str("(?:(?:");
                regex.push_str("(?:(?:");
                in_group = true;
            }
            '}' if in_group => {
                java.push_str("))");
                regex.push_str("))");
                in_group = false;
            }
            ',' if in_group => {
                java.push_str(")|(?:");
                regex.push_str(")|(?:");
            }
            '*' => {
                if next(i) == '*' {
                    java.push_str(".*");
                    regex.push_str(".*");
                    i += 1;
                } else {
                    java.push_str(&format!("[^{java_separator}]*"));
                    regex.push_str(&not_separator);
                    regex.push('*');
                }
            }
            '?' => {
                java.push_str(&format!("[^{java_separator}]"));
                regex.push_str(&not_separator);
            }
            c => {
                if is_regex_meta(c) {
                    java.push('\\');
                }
                java.push(c);
                regex.push_str(&folded_literal(c));
            }
        }
    }
    if in_group {
        return Err(syntax_error("Missing '}", glob, i - 1));
    }
    java.push('$');
    regex.push('$');
    if empty_class {
        return Err(syntax_error("Unclosed character class", &java, java.chars().count() - 1));
    }
    Ok(regex)
}

fn is_regex_meta(c: char) -> bool {
    ".^$+{[]|()".contains(c)
}

fn is_glob_meta(c: char) -> bool {
    "\\*?[{".contains(c)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn allows(list: &str, path: &str) -> bool {
        PathAllowList::read_plain(list).unwrap().matches(Path::new(path))
    }

    #[test]
    fn entries() {
        assert!(allows("C:\\links", "C:\\links\\a"));
        assert!(!allows("# C:\\links\n\n", "C:\\links\\a"));
        assert!(allows("[prefix]/srv", "/srv/x"));
        assert!(allows("[regex].*\\.zip", "a.zip"));
        assert!(PathAllowList::read_plain("[glob").is_err());
        assert!(PathAllowList::read_plain("[foo]x").is_err());
    }

    #[cfg(windows)]
    #[test]
    fn windows_globs() {
        assert!(allows("[glob]C:/Packs/*", "c:\\packs\\a"));
        assert!(!allows("[glob]C:/Packs/*", "C:\\Packs\\a\\b"));
        assert!(allows("[glob]C:/Packs/**", "C:\\Packs\\a\\b"));
        assert!(allows("[glob]*.{zip,jar}", "pack.JAR"));
        assert!(allows("[glob]pack[0-9]", "pack7"));
        assert!(!allows("[glob]pack[!0-9]", "pack7"));
        assert!(!allows("[glob]pack[!0-9]", "pack\\"));
        assert!(allows("[glob][a-]", "-"));
        assert!(allows("[glob][^a]", "^"));
        assert!(allows("[glob]C:/x/[a-z]*", "C:\\x\\Q"));
        assert!(allows("[glob]a}b,c", "a}b,c"));
        assert!(allows("[glob]C:/x/?", "C:\\x\\a"));
        let error = |glob: &str| to_regex_pattern(glob).unwrap_err().replace("java.util.regex.PatternSyntaxException: ", "").replace("\r\n", "|");
        assert_eq!(error("a[b"), "Missing '] near index 2|a[b|  ^");
        assert_eq!(error("{a,{b}}"), "Cannot nest groups near index 3|{a,{b}}|   ^");
        assert_eq!(error("a\\"), "No character to escape near index 1|a\\| ^");
        assert_eq!(error("[a/b]"), "Explicit 'name separator' in class near index 2|[a/b]|  ^");
        assert_eq!(error("[z-a]"), "Invalid range near index 1|[z-a]| ^");
        assert_eq!(error("{a"), "Missing '} near index 1|{a| ^");
        assert_eq!(error("[--]"), "Invalid range near index 2|[--]|  ^");
        assert_eq!(error("[\\]"), "Explicit 'name separator' in class near index 1|[\\]| ^");
        assert_eq!(error("[a-c-e]"), "Invalid range near index 4|[a-c-e]|    ^");
        assert_eq!(error("[!]"), "Unclosed character class near index 13|^[[^\\\\]&&[^]]$|             ^");
        assert_eq!(error("[]"), "Unclosed character class near index 12|^[[^\\\\]&&[]]$|            ^");
        assert!(allows("[glob]É*", "école"));
    }
}
