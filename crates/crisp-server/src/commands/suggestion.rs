use std::cmp::Ordering;

use super::brigadier::StringRange;
use crate::network::chat::Component;

#[derive(Clone, Debug, PartialEq)]
pub struct Suggestion {
    pub range: StringRange,
    pub text: String,
    pub tooltip: Option<Component>,
}

impl Suggestion {
    fn expand(&self, command: &[u16], range: StringRange) -> Suggestion {
        if range == self.range {
            return self.clone();
        }
        let mut text = String::new();
        if range.start < self.range.start {
            text.push_str(&String::from_utf16_lossy(&command[range.start..self.range.start]));
        }
        text.push_str(&self.text);
        if range.end > self.range.end {
            text.push_str(&String::from_utf16_lossy(&command[self.range.end..range.end]));
        }
        Suggestion { range, text, tooltip: self.tooltip.clone() }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Suggestions {
    pub range: StringRange,
    pub list: Vec<Suggestion>,
}

impl Suggestions {
    pub fn empty() -> Suggestions {
        Suggestions { range: StringRange::at(0), list: Vec::new() }
    }

    pub fn merge(command: &[u16], input: Vec<Suggestions>) -> Suggestions {
        match input.len() {
            0 => Suggestions::empty(),
            1 => input.into_iter().next().unwrap(),
            _ => {
                let mut texts: Vec<Suggestion> = Vec::new();
                for suggestion in input.into_iter().flat_map(|s| s.list) {
                    if !texts.contains(&suggestion) {
                        texts.push(suggestion);
                    }
                }
                Suggestions::create(command, texts)
            }
        }
    }

    pub fn create(command: &[u16], suggestions: Vec<Suggestion>) -> Suggestions {
        if suggestions.is_empty() {
            return Suggestions::empty();
        }
        let start = suggestions.iter().map(|s| s.range.start).min().unwrap();
        let end = suggestions.iter().map(|s| s.range.end).max().unwrap();
        let range = StringRange { start, end };
        let mut texts: Vec<Suggestion> = Vec::with_capacity(suggestions.len());
        for suggestion in &suggestions {
            let expanded = suggestion.expand(command, range);
            if !texts.contains(&expanded) {
                texts.push(expanded);
            }
        }
        texts.sort_by(|a, b| compare_to_ignore_case(&a.text, &b.text));
        Suggestions { range, list: texts }
    }
}

pub struct SuggestionsBuilder {
    input: Vec<u16>,
    input_lower_case: Vec<u16>,
    start: usize,
    result: Vec<Suggestion>,
}

impl SuggestionsBuilder {
    pub fn new(input: Vec<u16>, input_lower_case: Vec<u16>, start: usize) -> SuggestionsBuilder {
        SuggestionsBuilder { input, input_lower_case, start, result: Vec::new() }
    }

    pub fn input(&self) -> &[u16] {
        &self.input
    }

    pub fn start(&self) -> usize {
        self.start
    }

    pub fn remaining(&self) -> String {
        String::from_utf16_lossy(&self.input[self.start..])
    }

    pub fn remaining_lower_case(&self) -> String {
        String::from_utf16_lossy(&self.input_lower_case[self.start..])
    }

    pub fn build(self) -> Suggestions {
        Suggestions::create(&self.input, self.result)
    }

    pub fn suggest(&mut self, text: impl Into<String>) {
        self.suggest_with_tooltip(text, None);
    }

    pub fn suggest_with_tooltip(&mut self, text: impl Into<String>, tooltip: Option<Component>) {
        let text = text.into();
        if text.encode_utf16().eq(self.input[self.start..].iter().copied()) {
            return;
        }
        self.result.push(Suggestion { range: StringRange { start: self.start, end: self.input.len() }, text, tooltip });
    }

    pub fn suggest_int(&mut self, value: i32) {
        self.result.push(Suggestion { range: StringRange { start: self.start, end: self.input.len() }, text: value.to_string(), tooltip: None });
    }

    pub fn add(&mut self, other: SuggestionsBuilder) {
        self.result.extend(other.result);
    }

    pub fn create_offset(&self, start: usize) -> SuggestionsBuilder {
        SuggestionsBuilder::new(self.input.clone(), self.input_lower_case.clone(), start)
    }

    pub fn restart(&self) -> SuggestionsBuilder {
        self.create_offset(self.start)
    }
}

pub fn java_lower_case(units: &[u16]) -> Vec<u16> {
    String::from_utf16_lossy(units).to_lowercase().encode_utf16().collect()
}

fn compare_to_ignore_case(a: &str, b: &str) -> Ordering {
    let fold = |unit: u16| -> u32 {
        match char::from_u32(unit as u32) {
            Some(c) => {
                let upper = c.to_uppercase().next().unwrap_or(c);
                upper.to_lowercase().next().unwrap_or(upper) as u32
            }
            None => unit as u32,
        }
    };
    let (a, b): (Vec<u16>, Vec<u16>) = (a.encode_utf16().collect(), b.encode_utf16().collect());
    for (&x, &y) in a.iter().zip(&b) {
        if x != y {
            let (x, y) = (fold(x), fold(y));
            if x != y {
                return x.cmp(&y);
            }
        }
    }
    a.len().cmp(&b.len())
}
