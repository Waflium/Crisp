use serde_json::{Map, Number, Value};

const BUFFER_SIZE: usize = 1024;
const NESTING_LIMIT: usize = 255;
const MIN_INCOMPLETE_INTEGER: i64 = -922337203685477580;

const EMPTY_ARRAY: u8 = 1;
const NONEMPTY_ARRAY: u8 = 2;
const EMPTY_OBJECT: u8 = 3;
const DANGLING_NAME: u8 = 4;
const NONEMPTY_OBJECT: u8 = 5;
const EMPTY_DOCUMENT: u8 = 6;
const NONEMPTY_DOCUMENT: u8 = 7;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Peeked {
    None,
    BeginObject,
    EndObject,
    BeginArray,
    EndArray,
    True,
    False,
    Null,
    SingleQuoted,
    DoubleQuoted,
    Unquoted,
    SingleQuotedName,
    DoubleQuotedName,
    UnquotedName,
    Long,
    Number,
    Eof,
}

enum Failure {
    Malformed(String),
    Eof(String),
}

type R<T> = Result<T, Failure>;

struct JsonReader {
    buffer: Vec<u16>,
    pos: usize,
    line_number: usize,
    line_start: usize,
    peeked: Peeked,
    peeked_long: i64,
    peeked_number_length: usize,
    stack: Vec<u8>,
    path_names: Vec<Option<String>>,
    path_indices: Vec<i32>,
}

fn is_literal(c: u16) -> bool {
    !matches!(
        c,
        0x23 | 0x2F | 0x3B | 0x3D | 0x5C | 0x09 | 0x0A | 0x0C | 0x0D | 0x20 | 0x2C | 0x3A | 0x5B | 0x5D | 0x7B | 0x7D
    )
}

impl JsonReader {
    fn new(text: &str) -> JsonReader {
        let buffer: Vec<u16> = text.encode_utf16().collect();
        let skip = usize::from(buffer.first() == Some(&0xFEFF));
        JsonReader {
            buffer,
            pos: skip,
            line_number: 0,
            line_start: skip,
            peeked: Peeked::None,
            peeked_long: 0,
            peeked_number_length: 0,
            stack: vec![EMPTY_DOCUMENT],
            path_names: vec![None],
            path_indices: vec![0],
        }
    }

    fn limit(&self) -> usize {
        self.buffer.len()
    }

    fn available(&self, from: usize, minimum: usize) -> bool {
        self.limit().saturating_sub(from) >= minimum
    }

    fn path(&self) -> String {
        let mut result = String::from("$");
        for (i, &scope) in self.stack.iter().enumerate() {
            match scope {
                EMPTY_ARRAY | NONEMPTY_ARRAY => result.push_str(&format!("[{}]", self.path_indices[i])),
                EMPTY_OBJECT | DANGLING_NAME | NONEMPTY_OBJECT => {
                    result.push('.');
                    if let Some(name) = &self.path_names[i] {
                        result.push_str(name);
                    }
                }
                _ => {}
            }
        }
        result
    }

    fn location_string(&self) -> String {
        format!(" at line {} column {} path {}", self.line_number + 1, self.pos - self.line_start + 1, self.path())
    }

    fn syntax_error<T>(&self, message: &str) -> R<T> {
        Err(Failure::Malformed(format!(
            "{message}{}\nSee https://github.com/google/gson/blob/main/Troubleshooting.md#malformed-json",
            self.location_string()
        )))
    }

    fn push(&mut self, top: u8) -> R<()> {
        if self.stack.len() - 1 >= NESTING_LIMIT {
            return Err(Failure::Malformed(format!("Nesting limit {NESTING_LIMIT} reached{}", self.location_string())));
        }
        self.stack.push(top);
        self.path_names.push(None);
        self.path_indices.push(0);
        Ok(())
    }

    fn pop(&mut self) {
        self.stack.pop();
        self.path_names.pop();
        self.path_indices.pop();
    }

    fn top(&mut self) -> &mut u8 {
        self.stack.last_mut().unwrap()
    }

    fn next_non_whitespace(&mut self, throw_on_eof: bool) -> R<i32> {
        let mut p = self.pos;
        loop {
            if p == self.limit() {
                self.pos = p;
                if throw_on_eof {
                    return Err(Failure::Eof(format!("End of input{}", self.location_string())));
                }
                return Ok(-1);
            }
            let c = self.buffer[p];
            p += 1;
            if c == 0x0A {
                self.line_number += 1;
                self.line_start = p;
            } else if c != 0x20 && c != 0x0D && c != 0x09 {
                if c == 0x2F {
                    self.pos = p;
                    if p == self.limit() {
                        return Ok(c as i32);
                    }
                    match self.buffer[self.pos] {
                        0x2A => {
                            self.pos += 1;
                            if !self.skip_to(&[0x2A, 0x2F]) {
                                return self.syntax_error("Unterminated comment");
                            }
                            p = self.pos + 2;
                        }
                        0x2F => {
                            self.pos += 1;
                            self.skip_to_end_of_line();
                            p = self.pos;
                        }
                        _ => return Ok(c as i32),
                    }
                } else if c == 0x23 {
                    self.pos = p;
                    self.skip_to_end_of_line();
                    p = self.pos;
                } else {
                    self.pos = p;
                    return Ok(c as i32);
                }
            }
        }
    }

    fn skip_to_end_of_line(&mut self) {
        while self.pos < self.limit() {
            let c = self.buffer[self.pos];
            self.pos += 1;
            if c == 0x0A {
                self.line_number += 1;
                self.line_start = self.pos;
                break;
            } else if c == 0x0D {
                break;
            }
        }
    }

    fn skip_to(&mut self, to_find: &[u16]) -> bool {
        while self.available(self.pos, to_find.len()) {
            if self.buffer[self.pos] == 0x0A {
                self.line_number += 1;
                self.line_start = self.pos + 1;
            } else if self.buffer[self.pos..self.pos + to_find.len()] == *to_find {
                return true;
            }
            self.pos += 1;
        }
        false
    }

    fn consume_non_execute_prefix(&mut self) -> R<()> {
        self.next_non_whitespace(true)?;
        self.pos -= 1;
        if self.available(self.pos, 5) && self.buffer[self.pos..self.pos + 5] == [0x29, 0x5D, 0x7D, 0x27, 0x0A] {
            self.pos += 5;
        }
        Ok(())
    }

    fn do_peek(&mut self) -> R<Peeked> {
        let peek_stack = *self.stack.last().unwrap();
        if peek_stack == EMPTY_ARRAY {
            *self.top() = NONEMPTY_ARRAY;
        } else if peek_stack == NONEMPTY_ARRAY {
            match self.next_non_whitespace(true)? {
                0x2C | 0x3B => {}
                0x5D => return Ok(self.set_peeked(Peeked::EndArray)),
                _ => return self.syntax_error("Unterminated array"),
            }
        } else if peek_stack == EMPTY_OBJECT || peek_stack == NONEMPTY_OBJECT {
            *self.top() = DANGLING_NAME;
            if peek_stack == NONEMPTY_OBJECT {
                match self.next_non_whitespace(true)? {
                    0x2C | 0x3B => {}
                    0x7D => return Ok(self.set_peeked(Peeked::EndObject)),
                    _ => return self.syntax_error("Unterminated object"),
                }
            }
            let c = self.next_non_whitespace(true)?;
            return match c {
                0x22 => Ok(self.set_peeked(Peeked::DoubleQuotedName)),
                0x27 => Ok(self.set_peeked(Peeked::SingleQuotedName)),
                0x7D if peek_stack != NONEMPTY_OBJECT => Ok(self.set_peeked(Peeked::EndObject)),
                0x7D => self.syntax_error("Expected name"),
                _ => {
                    self.pos -= 1;
                    if is_literal(c as u16) { Ok(self.set_peeked(Peeked::UnquotedName)) } else { self.syntax_error("Expected name") }
                }
            };
        } else if peek_stack == DANGLING_NAME {
            *self.top() = NONEMPTY_OBJECT;
            match self.next_non_whitespace(true)? {
                0x3A => {}
                0x3D => {
                    if self.pos < self.limit() && self.buffer[self.pos] == 0x3E {
                        self.pos += 1;
                    }
                }
                _ => return self.syntax_error("Expected ':'"),
            }
        } else if peek_stack == EMPTY_DOCUMENT {
            self.consume_non_execute_prefix()?;
            *self.top() = NONEMPTY_DOCUMENT;
        } else if peek_stack == NONEMPTY_DOCUMENT {
            if self.next_non_whitespace(false)? == -1 {
                return Ok(self.set_peeked(Peeked::Eof));
            }
            self.pos -= 1;
        }
        let c = self.next_non_whitespace(true)?;
        match c {
            0x22 => Ok(self.set_peeked(Peeked::DoubleQuoted)),
            0x27 => Ok(self.set_peeked(Peeked::SingleQuoted)),
            0x5B => Ok(self.set_peeked(Peeked::BeginArray)),
            0x5D if peek_stack == EMPTY_ARRAY => Ok(self.set_peeked(Peeked::EndArray)),
            0x5D | 0x2C | 0x3B => {
                if peek_stack != EMPTY_ARRAY && peek_stack != NONEMPTY_ARRAY {
                    return self.syntax_error("Unexpected value");
                }
                self.pos -= 1;
                Ok(self.set_peeked(Peeked::Null))
            }
            0x7B => Ok(self.set_peeked(Peeked::BeginObject)),
            _ => {
                self.pos -= 1;
                if let Some(result) = self.peek_keyword() {
                    return Ok(result);
                }
                if let Some(result) = self.peek_number() {
                    return Ok(result);
                }
                if !is_literal(self.buffer[self.pos]) {
                    return self.syntax_error("Expected value");
                }
                Ok(self.set_peeked(Peeked::Unquoted))
            }
        }
    }

    fn set_peeked(&mut self, peeked: Peeked) -> Peeked {
        self.peeked = peeked;
        peeked
    }

    fn peek_keyword(&mut self) -> Option<Peeked> {
        let (keyword, peeking): (&[u8], Peeked) = match self.buffer[self.pos] {
            0x74 | 0x54 => (b"true", Peeked::True),
            0x66 | 0x46 => (b"false", Peeked::False),
            0x6E | 0x4E => (b"null", Peeked::Null),
            _ => return None,
        };
        for (i, &expected) in keyword.iter().enumerate() {
            if self.pos + i >= self.limit() {
                return None;
            }
            let c = self.buffer[self.pos + i];
            if c != expected as u16 && c != expected.to_ascii_uppercase() as u16 {
                return None;
            }
        }
        let length = keyword.len();
        if self.pos + length < self.limit() && is_literal(self.buffer[self.pos + length]) {
            return None;
        }
        self.pos += length;
        Some(self.set_peeked(peeking))
    }

    fn peek_number(&mut self) -> Option<Peeked> {
        let p = self.pos;
        let mut value: i64 = 0;
        let mut negative = false;
        let mut fits_in_long = true;
        let mut last = 0;
        let mut i = 0;
        loop {
            if i == BUFFER_SIZE {
                return None;
            }
            if p + i == self.limit() {
                break;
            }
            let c = self.buffer[p + i];
            match c {
                0x2B => {
                    if last != 5 {
                        return None;
                    }
                    last = 6;
                }
                0x2D => {
                    if last == 0 {
                        negative = true;
                        last = 1;
                    } else {
                        if last != 5 {
                            return None;
                        }
                        last = 6;
                    }
                }
                0x2E => {
                    if last != 2 {
                        return None;
                    }
                    last = 3;
                }
                0x45 | 0x65 => {
                    if last != 2 && last != 4 {
                        return None;
                    }
                    last = 5;
                }
                _ => {
                    if !(0x30..=0x39).contains(&c) {
                        if is_literal(c) {
                            return None;
                        }
                        break;
                    }
                    let digit = (c - 0x30) as i64;
                    if last == 1 || last == 0 {
                        value = -digit;
                        last = 2;
                    } else if last == 2 {
                        if value == 0 {
                            return None;
                        }
                        let new_value = value.wrapping_mul(10).wrapping_sub(digit);
                        fits_in_long &= value > MIN_INCOMPLETE_INTEGER || (value == MIN_INCOMPLETE_INTEGER && new_value < value);
                        value = new_value;
                    } else if last == 3 {
                        last = 4;
                    } else if last == 5 || last == 6 {
                        last = 7;
                    }
                }
            }
            i += 1;
        }
        if last != 2 || !fits_in_long || (value == i64::MIN && !negative) || (value == 0 && negative) {
            if last != 2 && last != 4 && last != 7 {
                return None;
            }
            self.peeked_number_length = i;
            Some(self.set_peeked(Peeked::Number))
        } else {
            self.peeked_long = if negative { value } else { value.wrapping_neg() };
            self.pos += i;
            Some(self.set_peeked(Peeked::Long))
        }
    }

    fn peek(&mut self) -> R<Peeked> {
        if self.peeked == Peeked::None {
            self.do_peek()?;
        }
        Ok(self.peeked)
    }

    fn string_of(&self, start: usize, end: usize) -> String {
        String::from_utf16_lossy(&self.buffer[start..end])
    }

    fn next_quoted_value(&mut self, quote: u16) -> R<String> {
        let mut builder: Vec<u16> = Vec::new();
        let mut p = self.pos;
        let mut start = p;
        while p < self.limit() {
            let c = self.buffer[p];
            p += 1;
            if c == quote {
                self.pos = p;
                builder.extend_from_slice(&self.buffer[start..p - 1]);
                return Ok(String::from_utf16_lossy(&builder));
            }
            if c == 0x5C {
                self.pos = p;
                builder.extend_from_slice(&self.buffer[start..p - 1]);
                let escaped = self.read_escape_character()?;
                builder.push(escaped);
                p = self.pos;
                start = p;
            } else if c == 0x0A {
                self.line_number += 1;
                self.line_start = p;
            }
        }
        self.pos = p;
        self.syntax_error("Unterminated string")
    }

    fn read_escape_character(&mut self) -> R<u16> {
        if self.pos == self.limit() {
            return self.syntax_error("Unterminated escape sequence");
        }
        let escaped = self.buffer[self.pos];
        self.pos += 1;
        match escaped {
            0x0A => {
                self.line_number += 1;
                self.line_start = self.pos;
                Ok(escaped)
            }
            0x27 | 0x22 | 0x2F | 0x5C => Ok(escaped),
            0x62 => Ok(0x08),
            0x66 => Ok(0x0C),
            0x6E => Ok(0x0A),
            0x72 => Ok(0x0D),
            0x74 => Ok(0x09),
            0x75 => {
                if !self.available(self.pos, 4) {
                    return self.syntax_error("Unterminated escape sequence");
                }
                let mut result: u16 = 0;
                for i in self.pos..self.pos + 4 {
                    let c = self.buffer[i];
                    result <<= 4;
                    result += match c {
                        0x30..=0x39 => c - 0x30,
                        0x61..=0x66 => c - 0x61 + 10,
                        0x41..=0x46 => c - 0x41 + 10,
                        _ => return self.syntax_error(&format!("Malformed Unicode escape \\u{}", self.string_of(self.pos, self.pos + 4))),
                    };
                }
                self.pos += 4;
                Ok(result)
            }
            _ => self.syntax_error("Invalid escape sequence"),
        }
    }

    fn next_unquoted_value(&mut self) -> String {
        let mut i = 0;
        while self.pos + i < self.limit() && is_literal(self.buffer[self.pos + i]) {
            i += 1;
        }
        let result = self.string_of(self.pos, self.pos + i);
        self.pos += i;
        result
    }

    fn next_name(&mut self) -> R<String> {
        let result = match self.peek()? {
            Peeked::UnquotedName => self.next_unquoted_value(),
            Peeked::SingleQuotedName => self.next_quoted_value(0x27)?,
            Peeked::DoubleQuotedName => self.next_quoted_value(0x22)?,
            _ => unreachable!("only called for names"),
        };
        self.peeked = Peeked::None;
        *self.path_names.last_mut().unwrap() = Some(result.clone());
        Ok(result)
    }

    fn advance_index(&mut self) {
        *self.path_indices.last_mut().unwrap() += 1;
    }

    fn read_terminal(&mut self, peeked: Peeked) -> R<Value> {
        let value = match peeked {
            Peeked::Unquoted => Value::String(self.next_unquoted_value()),
            Peeked::SingleQuoted => Value::String(self.next_quoted_value(0x27)?),
            Peeked::DoubleQuoted => Value::String(self.next_quoted_value(0x22)?),
            Peeked::Long => Value::Number(Number::from(self.peeked_long)),
            Peeked::Number => {
                let text = self.string_of(self.pos, self.pos + self.peeked_number_length);
                self.pos += self.peeked_number_length;
                number(&text)
            }
            Peeked::True => Value::Bool(true),
            Peeked::False => Value::Bool(false),
            Peeked::Null => Value::Null,
            _ => unreachable!("not a terminal"),
        };
        self.peeked = Peeked::None;
        self.advance_index();
        Ok(value)
    }

    fn try_begin_nesting(&mut self, peeked: Peeked) -> R<Option<Value>> {
        match peeked {
            Peeked::BeginArray => {
                self.push(EMPTY_ARRAY)?;
                self.peeked = Peeked::None;
                Ok(Some(Value::Array(Vec::new())))
            }
            Peeked::BeginObject => {
                self.push(EMPTY_OBJECT)?;
                self.peeked = Peeked::None;
                Ok(Some(Value::Object(Map::new())))
            }
            _ => Ok(None),
        }
    }

    fn has_next(&mut self) -> R<bool> {
        Ok(!matches!(self.peek()?, Peeked::EndObject | Peeked::EndArray | Peeked::Eof))
    }

    fn end(&mut self) {
        self.pop();
        self.advance_index();
        self.peeked = Peeked::None;
    }

    fn read(&mut self) -> R<Value> {
        let peeked = self.peek()?;
        let Some(root) = self.try_begin_nesting(peeked)? else { return self.read_terminal(peeked) };
        let mut stack: Vec<(Option<String>, Value)> = Vec::new();
        let mut current = root;
        let mut current_name: Option<String> = None;
        loop {
            while !self.has_next()? {
                self.end();
                match stack.pop() {
                    None => return Ok(current),
                    Some((name, mut parent)) => {
                        insert(&mut parent, current_name.take(), current);
                        current = parent;
                        current_name = name;
                    }
                }
            }
            let name = if current.is_object() { Some(self.next_name()?) } else { None };
            let peeked = self.peek()?;
            match self.try_begin_nesting(peeked)? {
                Some(value) => {
                    stack.push((current_name.take(), std::mem::replace(&mut current, value)));
                    current_name = name;
                }
                None => {
                    let value = self.read_terminal(peeked)?;
                    insert(&mut current, name, value);
                }
            }
        }
    }
}

fn insert(container: &mut Value, name: Option<String>, value: Value) {
    match container {
        Value::Array(items) => items.push(value),
        Value::Object(fields) => {
            fields.insert(name.unwrap(), value);
        }
        _ => unreachable!(),
    }
}

fn number(text: &str) -> Value {
    if let Ok(v) = text.parse::<i64>() {
        return Value::Number(Number::from(v));
    }
    if let Ok(v) = text.parse::<u64>() {
        return Value::Number(Number::from(v));
    }
    text.parse::<f64>().ok().and_then(Number::from_f64).map_or(Value::Null, Value::Number)
}

pub fn parse_string(json: &str) -> Result<Value, String> {
    let mut reader = JsonReader::new(json);
    let element = match reader.peek() {
        Err(Failure::Eof(_)) => return Ok(Value::Null),
        Err(Failure::Malformed(m)) => return Err(syntax_exception(&m)),
        Ok(_) => match reader.read() {
            Ok(element) => element,
            Err(Failure::Eof(m)) => return Err(format!("com.google.gson.JsonSyntaxException: java.io.EOFException: {m}")),
            Err(Failure::Malformed(m)) => return Err(syntax_exception(&m)),
        },
    };
    if !element.is_null() {
        match reader.peek() {
            Ok(Peeked::Eof) => {}
            Ok(_) => return Err("com.google.gson.JsonSyntaxException: Did not consume the entire document.".into()),
            Err(Failure::Malformed(m)) => return Err(syntax_exception(&m)),
            Err(Failure::Eof(m)) => return Err(format!("com.google.gson.JsonIOException: java.io.EOFException: {m}")),
        }
    }
    Ok(element)
}

fn syntax_exception(message: &str) -> String {
    format!("com.google.gson.JsonSyntaxException: com.google.gson.stream.MalformedJsonException: {message}")
}

pub fn type_name(element: Option<&Value>) -> String {
    let Some(element) = element else { return "null (missing)".into() };
    let text = element.to_string();
    let units: Vec<u16> = text.encode_utf16().collect();
    let value = if units.len() <= 10 { text.clone() } else { format!("{}...{}", String::from_utf16_lossy(&units[..4]), String::from_utf16_lossy(&units[units.len() - 3..])) };
    match element {
        Value::Null => "null (json)".into(),
        Value::Array(_) => format!("an array ({value})"),
        Value::Object(_) => format!("an object ({value})"),
        Value::Number(_) => format!("a number ({value})"),
        Value::Bool(_) => format!("a boolean ({value})"),
        Value::String(_) => value,
    }
}

pub fn as_string(element: &Value) -> Option<String> {
    match element {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}
