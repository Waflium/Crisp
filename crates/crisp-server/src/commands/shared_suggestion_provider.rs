use super::suggestion::SuggestionsBuilder;
use crate::network::chat::Component;

pub struct TextCoordinates {
    pub x: &'static str,
    pub y: &'static str,
    pub z: &'static str,
}

pub const DEFAULT_LOCAL: TextCoordinates = TextCoordinates { x: "^", y: "^", z: "^" };
pub const DEFAULT_GLOBAL: TextCoordinates = TextCoordinates { x: "~", y: "~", z: "~" };

pub fn matches_sub_str(pattern: &str, input: &str) -> bool {
    let mut index = 0;
    loop {
        if input[index..].starts_with(pattern) {
            return true;
        }
        match input[index..].find(['.', '_', '/']) {
            Some(splitter) => index += splitter + 1,
            None => return false,
        }
    }
}

pub fn suggest<S: AsRef<str>>(values: impl IntoIterator<Item = S>, builder: &mut SuggestionsBuilder) {
    let lower_prefix = builder.remaining().to_lowercase();
    for name in values {
        let name = name.as_ref();
        if matches_sub_str(&lower_prefix, &name.to_lowercase()) {
            builder.suggest(name);
        }
    }
}

fn split_identifier(id: &str) -> (&str, &str) {
    id.split_once(':').unwrap_or(("minecraft", id))
}

fn identifier_matches(contents: &str, id: &str) -> bool {
    if contents.contains(':') {
        matches_sub_str(contents, id)
    } else {
        let (namespace, path) = split_identifier(id);
        matches_sub_str(contents, namespace) || matches_sub_str(contents, path)
    }
}

fn common_prefix_len(a: &str, b: &str) -> usize {
    a.char_indices().zip(b.chars()).take_while(|((_, x), y)| x == y).map(|((i, x), _)| i + x.len_utf8()).last().unwrap_or(0)
}

pub fn suggest_resource<S: AsRef<str>>(values: impl IntoIterator<Item = S>, builder: &mut SuggestionsBuilder) {
    let contents = builder.remaining().to_lowercase();
    for id in values {
        let id = id.as_ref();
        if identifier_matches(&contents, id) {
            builder.suggest(id);
        }
    }
}

pub fn suggest_resource_with_prefix<S: AsRef<str>>(values: impl IntoIterator<Item = S>, builder: &mut SuggestionsBuilder, prefix: &str) {
    let contents = builder.remaining().to_lowercase();
    let stripped = if contents.is_empty() {
        None
    } else {
        let common = common_prefix_len(&contents, prefix);
        if common == 0 {
            return;
        }
        Some(contents[common..].to_string())
    };
    for id in values {
        let id = id.as_ref();
        if stripped.as_deref().is_none_or(|stripped| identifier_matches(stripped, id)) {
            builder.suggest(format!("{prefix}{id}"));
        }
    }
}

pub fn suggest_resource_with_tooltips<'a>(values: impl IntoIterator<Item = (&'a str, Component)>, builder: &mut SuggestionsBuilder) {
    let contents = builder.remaining().to_lowercase();
    for (id, tooltip) in values {
        if identifier_matches(&contents, id) {
            builder.suggest_with_tooltip(id, Some(tooltip));
        }
    }
}

pub fn suggest_coordinates(current_input: &str, all: &[TextCoordinates], builder: &mut SuggestionsBuilder, validator: impl Fn(&str) -> bool) {
    let mut result = Vec::new();
    if current_input.is_empty() {
        for coordinate in all {
            let full = format!("{} {} {}", coordinate.x, coordinate.y, coordinate.z);
            if validator(&full) {
                result.push(coordinate.x.to_string());
                result.push(format!("{} {}", coordinate.x, coordinate.y));
                result.push(full);
            }
        }
    } else {
        let fields = java_split_space(current_input);
        if fields.len() == 1 {
            for coordinate in all {
                let full = format!("{} {} {}", fields[0], coordinate.y, coordinate.z);
                if validator(&full) {
                    result.push(format!("{} {}", fields[0], coordinate.y));
                    result.push(full);
                }
            }
        } else if fields.len() == 2 {
            for coordinate in all {
                let full = format!("{} {} {}", fields[0], fields[1], coordinate.z);
                if validator(&full) {
                    result.push(full);
                }
            }
        }
    }
    suggest(result, builder);
}

pub fn suggest_2d_coordinates(current_input: &str, all: &[TextCoordinates], builder: &mut SuggestionsBuilder, validator: impl Fn(&str) -> bool) {
    let mut result = Vec::new();
    if current_input.is_empty() {
        for coordinate in all {
            let full = format!("{} {}", coordinate.x, coordinate.z);
            if validator(&full) {
                result.push(coordinate.x.to_string());
                result.push(full);
            }
        }
    } else {
        let fields = java_split_space(current_input);
        if fields.len() == 1 {
            for coordinate in all {
                let full = format!("{} {}", fields[0], coordinate.z);
                if validator(&full) {
                    result.push(full);
                }
            }
        }
    }
    suggest(result, builder);
}

fn java_split_space(input: &str) -> Vec<&str> {
    let mut fields: Vec<&str> = input.split(' ').collect();
    while fields.last().is_some_and(|f| f.is_empty()) {
        fields.pop();
    }
    fields
}
