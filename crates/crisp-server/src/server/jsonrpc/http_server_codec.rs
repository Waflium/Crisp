use std::collections::VecDeque;

pub const MAX_CONTENT_LENGTH: usize = 65536;
const MAX_INITIAL_LINE_LENGTH: usize = 4096;
const MAX_HEADER_SIZE: usize = 8192;
const MAX_CHUNK_SIZE: usize = 8192;
const TOKEN_CHARS_LOW: u64 = 288068722172624896;
const TOKEN_CHARS_HIGH: u64 = 6341068274398134270;

const CONTINUE: &[u8] = b"HTTP/1.1 100 Continue\r\n\r\n";
const EXPECTATION_FAILED: &[u8] = b"HTTP/1.1 417 Expectation Failed\r\ncontent-length: 0\r\n\r\n";
const TOO_LARGE: &[u8] = b"HTTP/1.1 413 Request Entity Too Large\r\ncontent-length: 0\r\n\r\n";
const TOO_LARGE_CLOSE: &[u8] = b"HTTP/1.1 413 Request Entity Too Large\r\ncontent-length: 0\r\nconnection: close\r\n\r\n";

#[derive(Clone, Copy)]
pub struct HttpVersion {
    major: u32,
    minor: u32,
    http_1_1: bool,
    keep_alive_default: bool,
}

impl HttpVersion {
    const HTTP_1_0: HttpVersion = HttpVersion { major: 1, minor: 0, http_1_1: false, keep_alive_default: false };
    const HTTP_1_1: HttpVersion = HttpVersion { major: 1, minor: 1, http_1_1: true, keep_alive_default: true };

    fn value_of(text: &str) -> Option<HttpVersion> {
        match text {
            "HTTP/1.1" => Some(HttpVersion::HTTP_1_1),
            "HTTP/1.0" => Some(HttpVersion::HTTP_1_0),
            _ => {
                let text = text.to_ascii_uppercase();
                let bytes = text.as_bytes();
                if bytes.len() != 8 || !text.starts_with("HTTP/") || bytes[6] != b'.' || !bytes[5].is_ascii_digit() || !bytes[7].is_ascii_digit() {
                    return None;
                }
                Some(HttpVersion { major: (bytes[5] - b'0') as u32, minor: (bytes[7] - b'0') as u32, http_1_1: false, keep_alive_default: true })
            }
        }
    }

    fn at_least_1_1(self) -> bool {
        (self.major, self.minor) >= (1, 1)
    }
}

pub struct HttpRequest {
    pub method: String,
    pub uri: String,
    pub version: HttpVersion,
    pub headers: Vec<(String, String)>,
    pub content: Vec<u8>,
}

impl HttpRequest {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.iter().find(|(n, _)| n.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str())
    }

    pub fn has_header(&self, name: &str) -> bool {
        self.header(name).is_some()
    }

    fn all(&self, name: &str) -> impl Iterator<Item = &str> {
        self.headers.iter().filter(move |(n, _)| n.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str())
    }

    pub fn contains_token(&self, name: &str, token: &str) -> bool {
        self.all(name).any(|v| v.split(',').any(|t| java_trim(t).eq_ignore_ascii_case(token)))
    }

    pub fn contains_value(&self, name: &str, value: &str) -> bool {
        self.all(name).any(|v| v.eq_ignore_ascii_case(value))
    }

    pub fn names(&self) -> String {
        let mut names: Vec<&str> = Vec::new();
        for (name, _) in &self.headers {
            if !names.contains(&name.as_str()) {
                names.push(name);
            }
        }
        format!("[{}]", names.join(", "))
    }

    fn remove_header(&mut self, name: &str) {
        self.headers.retain(|(n, _)| !n.eq_ignore_ascii_case(name));
    }

    fn set_header(&mut self, name: &str, values: impl IntoIterator<Item = String>) {
        self.remove_header(name);
        self.headers.extend(values.into_iter().map(|v| (name.to_string(), v)));
    }

    fn is_keep_alive(&self) -> bool {
        !self.contains_token("connection", "close") && (self.version.keep_alive_default || self.contains_token("connection", "keep-alive"))
    }

    fn is_100_continue_expected(&self) -> bool {
        self.version.at_least_1_1() && self.contains_value("expect", "100-continue")
    }

    fn is_unsupported_expectation(&self) -> bool {
        self.version.at_least_1_1() && self.header("expect").is_some_and(|e| !e.eq_ignore_ascii_case("100-continue"))
    }

    fn content_length(&self) -> Option<i64> {
        match self.header("content-length") {
            Some(value) => parse_long(value),
            None => Some(self.web_socket_content_length()),
        }
    }

    fn web_socket_content_length(&self) -> i64 {
        if self.method == "GET" && self.has_header("sec-websocket-key1") && self.has_header("sec-websocket-key2") { 8 } else { -1 }
    }

    fn set_transfer_encoding_unchunked(&mut self) {
        let encodings: Vec<String> = self.all("transfer-encoding").map(str::to_string).collect();
        if encodings.is_empty() {
            return;
        }
        let values: Vec<String> = encodings.into_iter().filter(|v| !v.eq_ignore_ascii_case("chunked")).collect();
        if values.is_empty() {
            self.remove_header("transfer-encoding");
        } else {
            self.set_header("transfer-encoding", values);
        }
    }
}

pub enum HttpObject {
    Message { request: HttpRequest, failed: bool },
    FullMessage(HttpRequest),
    Content { data: Vec<u8>, last: bool, failed: bool },
}

#[derive(Clone, Copy, PartialEq)]
enum State {
    SkipInitialLineChars,
    ReadInitial,
    ReadHeader,
    ReadVariableLengthContent,
    ReadFixedLengthContent,
    ReadChunkSize,
    ReadChunkedContent,
    ReadChunkDelimiter,
    ReadChunkFooter,
    BadMessage,
}

struct HeaderParser {
    max_length: usize,
    size: usize,
}

impl HeaderParser {
    fn parse(&mut self, buffer: &[u8], pos: &mut usize) -> Result<Option<(usize, usize)>, ()> {
        let readable = buffer.len() - *pos;
        let max_body_size = self.max_length - self.size;
        let to_process = (max_body_size + 2).min(readable);
        let Some(lf) = buffer[*pos..*pos + to_process].iter().position(|&b| b == b'\n').map(|i| *pos + i) else {
            return if readable > max_body_size { Err(()) } else { Ok(None) };
        };
        if lf == *pos || buffer[lf - 1] != b'\r' {
            return Err(());
        }
        let end = lf - 1;
        let start = *pos;
        let new_size = end - start;
        if new_size > 0 {
            let size = self.size + new_size;
            if size > self.max_length {
                return Err(());
            }
            self.size = size;
        }
        *pos = lf + 1;
        Ok(Some((start, end)))
    }
}

pub struct HttpObjectDecoder {
    state: State,
    line_parser: HeaderParser,
    header_parser: HeaderParser,
    message: Option<HttpRequest>,
    name: Option<String>,
    value: String,
    chunk_size: usize,
    content_length: i64,
    trailer: bool,
    reset_requested: bool,
}

impl HttpObjectDecoder {
    fn new() -> HttpObjectDecoder {
        HttpObjectDecoder {
            state: State::SkipInitialLineChars,
            line_parser: HeaderParser { max_length: MAX_INITIAL_LINE_LENGTH, size: 0 },
            header_parser: HeaderParser { max_length: MAX_HEADER_SIZE, size: 0 },
            message: None,
            name: None,
            value: String::new(),
            chunk_size: 0,
            content_length: i64::MIN,
            trailer: false,
            reset_requested: false,
        }
    }

    fn reset(&mut self) {
        if matches!(self.state, State::ReadChunkSize | State::ReadVariableLengthContent | State::ReadFixedLengthContent) {
            self.reset_requested = true;
        }
    }

    fn reset_now(&mut self) {
        self.message = None;
        self.name = None;
        self.value.clear();
        self.content_length = i64::MIN;
        self.line_parser.size = 0;
        self.header_parser.size = 0;
        self.trailer = false;
        self.reset_requested = false;
        self.state = State::SkipInitialLineChars;
    }

    fn parse_line(&mut self, buffer: &[u8], pos: &mut usize) -> Result<Option<(usize, usize)>, ()> {
        self.line_parser.size = 0;
        let readable = buffer.len() - *pos;
        if readable == 0 {
            return Ok(None);
        }
        if self.state == State::SkipInitialLineChars {
            let max_to_skip = MAX_INITIAL_LINE_LENGTH.min(readable);
            match buffer[*pos..*pos + max_to_skip].iter().position(|&b| b != b'\r' && b != b'\n') {
                None => {
                    *pos += max_to_skip;
                    return if readable > MAX_INITIAL_LINE_LENGTH { Err(()) } else { Ok(None) };
                }
                Some(i) => {
                    if is_iso_control(buffer[*pos + i]) {
                        return Err(());
                    }
                    *pos += i;
                    self.state = State::ReadInitial;
                }
            }
        }
        self.line_parser.parse(buffer, pos)
    }

    fn decode(&mut self, buffer: &[u8], pos: &mut usize, out: &mut Vec<HttpObject>) {
        if self.reset_requested {
            self.reset_now();
        }
        loop {
            match self.state {
                State::ReadChunkSize => {
                    let line = match self.parse_line(buffer, pos) {
                        Ok(Some(line)) => line,
                        Ok(None) => return,
                        Err(()) => return out.push(self.invalid_chunk(buffer, pos)),
                    };
                    let line = &buffer[line.0..line.1];
                    let Some(chunk_size) = (if line.contains(&b';') && !is_valid_chunk_line(line) { None } else { chunk_size(line) }) else {
                        return out.push(self.invalid_chunk(buffer, pos));
                    };
                    self.chunk_size = chunk_size;
                    if chunk_size == 0 {
                        self.state = State::ReadChunkFooter;
                        return;
                    }
                    self.state = State::ReadChunkedContent;
                }
                State::ReadChunkedContent => {
                    let to_read = self.chunk_size.min(MAX_CHUNK_SIZE).min(buffer.len() - *pos);
                    if to_read == 0 {
                        return;
                    }
                    out.push(HttpObject::Content { data: buffer[*pos..*pos + to_read].to_vec(), last: false, failed: false });
                    *pos += to_read;
                    self.chunk_size -= to_read;
                    if self.chunk_size != 0 {
                        return;
                    }
                    self.state = State::ReadChunkDelimiter;
                }
                State::ReadChunkDelimiter => {
                    if buffer.len() - *pos >= 2 {
                        if buffer[*pos] == b'\r' && buffer[*pos + 1] == b'\n' {
                            *pos += 2;
                            self.state = State::ReadChunkSize;
                        } else {
                            out.push(self.invalid_chunk(buffer, pos));
                        }
                    }
                    return;
                }
                State::SkipInitialLineChars | State::ReadInitial => {
                    let line = match self.parse_line(buffer, pos) {
                        Ok(Some(line)) => line,
                        Ok(None) => return,
                        Err(()) => return out.push(self.invalid_message(buffer, pos)),
                    };
                    let Some(request) = split_initial_line(&buffer[line.0..line.1]) else {
                        return out.push(self.invalid_message(buffer, pos));
                    };
                    self.message = Some(request);
                    self.state = State::ReadHeader;
                }
                State::ReadHeader => {
                    let next = match self.read_headers(buffer, pos) {
                        Ok(Some(next)) => next,
                        Ok(None) => return,
                        Err(()) => return out.push(self.invalid_message(buffer, pos)),
                    };
                    self.state = next;
                    let request = self.message.take().expect("a message");
                    out.push(HttpObject::Message { request, failed: false });
                    if next == State::ReadFixedLengthContent && self.content_length != 0 {
                        self.chunk_size = self.content_length as usize;
                    } else if next != State::ReadChunkSize {
                        out.push(HttpObject::Content { data: Vec::new(), last: true, failed: false });
                        self.reset_now();
                    }
                    return;
                }
                State::ReadFixedLengthContent => {
                    let readable = buffer.len() - *pos;
                    if readable == 0 {
                        return;
                    }
                    let to_read = readable.min(MAX_CHUNK_SIZE).min(self.chunk_size);
                    let data = buffer[*pos..*pos + to_read].to_vec();
                    *pos += to_read;
                    self.chunk_size -= to_read;
                    if self.chunk_size == 0 {
                        out.push(HttpObject::Content { data, last: true, failed: false });
                        self.reset_now();
                    } else {
                        out.push(HttpObject::Content { data, last: false, failed: false });
                    }
                    return;
                }
                State::ReadChunkFooter => {
                    match self.read_trailing_headers(buffer, pos) {
                        Ok(true) => {
                            out.push(HttpObject::Content { data: Vec::new(), last: true, failed: false });
                            self.reset_now();
                        }
                        Ok(false) => {}
                        Err(()) => out.push(self.invalid_chunk(buffer, pos)),
                    }
                    return;
                }
                State::BadMessage | State::ReadVariableLengthContent => {
                    *pos = buffer.len();
                    return;
                }
            }
        }
    }

    fn decode_last(&mut self, buffer: &[u8], pos: &mut usize, out: &mut Vec<HttpObject>) {
        if *pos < buffer.len() {
            self.decode(buffer, pos, out);
        }
        if self.reset_requested {
            self.reset_now();
        }
        match self.state {
            State::SkipInitialLineChars | State::ReadInitial | State::BadMessage | State::ReadVariableLengthContent => {}
            State::ReadChunkSize | State::ReadFixedLengthContent | State::ReadChunkedContent | State::ReadChunkDelimiter | State::ReadChunkFooter => {
                self.reset_now()
            }
            State::ReadHeader => {
                let message = self.invalid_message(buffer, pos);
                out.push(message);
                self.reset_now();
            }
        }
    }

    fn invalid_message(&mut self, buffer: &[u8], pos: &mut usize) -> HttpObject {
        self.state = State::BadMessage;
        self.trailer = false;
        *pos = buffer.len();
        match self.message.take() {
            Some(request) => HttpObject::Message { request, failed: true },
            None => HttpObject::FullMessage(HttpRequest {
                method: "GET".into(),
                uri: "/bad-request".into(),
                version: HttpVersion::HTTP_1_0,
                headers: Vec::new(),
                content: Vec::new(),
            }),
        }
    }

    fn invalid_chunk(&mut self, buffer: &[u8], pos: &mut usize) -> HttpObject {
        self.state = State::BadMessage;
        self.message = None;
        self.trailer = false;
        *pos = buffer.len();
        HttpObject::Content { data: Vec::new(), last: true, failed: true }
    }

    fn read_headers(&mut self, buffer: &[u8], pos: &mut usize) -> Result<Option<State>, ()> {
        let Some(mut line) = self.header_parser.parse(buffer, pos)? else { return Ok(None) };
        while line.1 > line.0 {
            let first = buffer[line.0];
            if self.name.is_none() || (first != b' ' && first != b'\t') {
                if let Some(name) = self.name.take() {
                    let value = std::mem::take(&mut self.value);
                    add_header(&mut self.message.as_mut().expect("a message").headers, name, value)?;
                }
                self.split_header(&buffer[line.0..line.1])?;
            } else {
                self.value.push(' ');
                self.value.push_str(java_trim(&latin1(&buffer[line.0..line.1])));
            }
            let Some(next) = self.header_parser.parse(buffer, pos)? else { return Ok(None) };
            line = next;
        }
        let message = self.message.as_mut().expect("a message");
        if let Some(name) = self.name.take() {
            add_header(&mut message.headers, name, std::mem::take(&mut self.value))?;
        }
        let content_length_fields: Vec<String> = message.all("content-length").map(str::to_string).collect();
        if content_length_fields.is_empty() {
            self.content_length = message.web_socket_content_length();
        } else {
            let http_1_0_or_earlier = message.version.major < 1 || (message.version.major == 1 && message.version.minor == 0);
            self.content_length = normalize_and_get_content_length(&content_length_fields, http_1_0_or_earlier)?;
            let length_value = java_trim(&content_length_fields[0]);
            if content_length_fields.len() > 1 || parse_long(length_value) != Some(self.content_length) {
                message.set_header("content-length", [self.content_length.to_string()]);
            }
        }
        if message.has_header("transfer-encoding") && !message.version.http_1_1 {
            return Err(());
        }
        if message.contains_token("transfer-encoding", "chunked") {
            let last = message.all("transfer-encoding").last().unwrap_or_default();
            let chars: Vec<char> = last.chars().collect();
            if chars.len() > 7 && !chars[chars.len() - 7..].iter().collect::<String>().eq_ignore_ascii_case("chunked") {
                return Err(());
            }
            if !content_length_fields.is_empty() {
                self.content_length = i64::MIN;
                return Err(());
            }
            return Ok(Some(State::ReadChunkSize));
        }
        Ok(Some(if self.content_length >= 0 { State::ReadFixedLengthContent } else { State::ReadVariableLengthContent }))
    }

    fn read_trailing_headers(&mut self, buffer: &[u8], pos: &mut usize) -> Result<bool, ()> {
        let Some(mut line) = self.header_parser.parse(buffer, pos)? else { return Ok(false) };
        if line.1 == line.0 && !self.trailer {
            return Ok(true);
        }
        self.trailer = true;
        let mut trailers = Vec::new();
        while line.1 > line.0 {
            let first = buffer[line.0];
            if self.name.is_none() || (first != b' ' && first != b'\t') {
                if let Some(name) = self.name.take() {
                    let value = std::mem::take(&mut self.value);
                    if is_permitted_trailing_header(&name) {
                        add_header(&mut trailers, name, value)?;
                    }
                }
                self.split_header(&buffer[line.0..line.1])?;
            } else {
                self.value.push(' ');
                self.value.push_str(java_trim(&latin1(&buffer[line.0..line.1])));
            }
            let Some(next) = self.header_parser.parse(buffer, pos)? else { return Ok(false) };
            line = next;
        }
        if let Some(name) = self.name.take() {
            let value = std::mem::take(&mut self.value);
            if is_permitted_trailing_header(&name) {
                add_header(&mut trailers, name, value)?;
            }
        }
        self.trailer = false;
        Ok(true)
    }

    fn split_header(&mut self, line: &[u8]) -> Result<(), ()> {
        let name_end = line.iter().position(|&b| b == b':').ok_or(())?;
        let colon_end = name_end + 1;
        self.name = Some(latin1(&line[..name_end]));
        let mut value_start = line.len();
        for (i, &c) in line.iter().enumerate().skip(colon_end) {
            if !is_whitespace(c) {
                value_start = i;
                break;
            }
            if !is_ows(c) {
                return Err(());
            }
        }
        self.value = if value_start == line.len() {
            String::new()
        } else {
            let value_end = (1..line.len()).rev().find(|&i| !is_ows(line[i])).map_or(0, |i| i + 1);
            latin1(&line[value_start..value_end])
        };
        Ok(())
    }
}

fn add_header(headers: &mut Vec<(String, String)>, name: String, value: String) -> Result<(), ()> {
    if name.is_empty() || !name.chars().all(|c| (c as u32) < 128 && is_valid_token_char(c as u8)) {
        return Err(());
    }
    let mut chars = value.chars().map(|c| c as u32);
    if let Some(first) = chars.next()
        && (first < 33 || first == 127 || chars.any(|c| (c < 32 && c != 9) || c == 127))
    {
        return Err(());
    }
    headers.push((name, value));
    Ok(())
}

fn is_permitted_trailing_header(name: &str) -> bool {
    !name.eq_ignore_ascii_case("content-length") && !name.eq_ignore_ascii_case("transfer-encoding") && !name.eq_ignore_ascii_case("trailer")
}

fn split_initial_line(line: &[u8]) -> Option<HttpRequest> {
    let end = line.len();
    if is_control_or_whitespace(*line.last()?) {
        return None;
    }
    let a_start = find_non_sp_lenient(line, 0)?;
    let a_end = find_sp_lenient(line, a_start);
    let b_start = find_non_sp_lenient(line, a_end)?;
    let b_end = find_sp_lenient(line, b_start);
    let c_start = find_non_sp_lenient(line, b_end)?;
    let c_end = (c_start.saturating_sub(1) + 1..end).rev().find(|&i| !is_ows(line[i])).map_or(0, |i| i + 1);
    let version = if c_start < c_end { latin1(&line[c_start..c_end]) } else { String::new() };
    if version.is_empty() {
        return None;
    }
    let version = HttpVersion::value_of(&version)?;
    let method = latin1(&line[a_start..a_end]);
    if method.is_empty() || !method.chars().all(|c| (c as u32) < 128 && is_valid_token_char(c as u8)) {
        return None;
    }
    Some(HttpRequest { method, uri: latin1(&line[b_start..b_end]), version, headers: Vec::new(), content: Vec::new() })
}

fn find_non_sp_lenient(line: &[u8], offset: usize) -> Option<usize> {
    for (i, &c) in line.iter().enumerate().skip(offset) {
        if !is_sp_lenient(c) {
            return if is_whitespace(c) { None } else { Some(i) };
        }
    }
    Some(line.len())
}

fn find_sp_lenient(line: &[u8], offset: usize) -> usize {
    line.iter().skip(offset).position(|&c| is_sp_lenient(c)).map_or(line.len(), |i| offset + i)
}

fn chunk_size(line: &[u8]) -> Option<usize> {
    let skipped = line.iter().position(|&b| !is_whitespace(b))?;
    let mut result: u64 = 0;
    for (i, &b) in line[skipped..].iter().enumerate() {
        let Some(digit) = (b as char).to_digit(16) else {
            if b != b';' && !is_control_or_whitespace(b) {
                return None;
            }
            return if i == 0 { None } else { Some(result as usize) };
        };
        result = result * 16 + digit as u64;
        if result > i32::MAX as u64 {
            return None;
        }
    }
    Some(result as usize)
}

fn is_valid_chunk_line(line: &[u8]) -> bool {
    #[derive(Clone, Copy, PartialEq)]
    enum S {
        Size,
        ExtName,
        ExtValStart,
        ExtValQuoted,
        ExtValQuotedEscape,
        ExtValQuotedEnd,
        ExtValToken,
    }
    let separator = |b: u8| b"(),/:<=>?@[\\]{}".contains(&b);
    let mut state = S::Size;
    for &b in line {
        let visible = (33..=126).contains(&b);
        state = match state {
            S::Size if b.is_ascii_hexdigit() || b == b' ' || b == b'\t' => S::Size,
            S::Size if b == b';' => S::ExtName,
            S::ExtName if (visible || b == b' ' || b == b'\t') && !separator(b) => S::ExtName,
            S::ExtName if b == b'=' => S::ExtValStart,
            S::ExtValStart if b == b' ' || b == b'\t' => S::ExtValStart,
            S::ExtValStart if b == b'"' => S::ExtValQuoted,
            S::ExtValStart if visible && !separator(b) && b != b'"' => S::ExtValToken,
            S::ExtValQuoted if b == b'\\' => S::ExtValQuotedEscape,
            S::ExtValQuoted if b == b'"' => S::ExtValQuotedEnd,
            S::ExtValQuoted if b == b'\t' || b == b' ' || b == b'!' || (35..=91).contains(&b) || (93..=126).contains(&b) => S::ExtValQuoted,
            S::ExtValQuotedEscape if b == b'\t' || b == b' ' || visible => S::ExtValQuoted,
            S::ExtValQuotedEnd if b == b'\t' || b == b' ' => S::ExtValQuotedEnd,
            S::ExtValQuotedEnd if b == b';' => S::ExtName,
            S::ExtValToken if visible && !separator(b) && b != b';' => S::ExtValToken,
            S::ExtValToken if b == b';' => S::ExtName,
            _ => return false,
        };
    }
    !matches!(state, S::ExtValQuoted | S::ExtValQuotedEscape | S::ExtValStart)
}

fn normalize_and_get_content_length(fields: &[String], http_1_0_or_earlier: bool) -> Result<i64, ()> {
    let first = &fields[0];
    if (fields.len() > 1 || first.contains(',')) && !http_1_0_or_earlier {
        return Err(());
    }
    if !first.starts_with(|c: char| c.is_ascii_digit()) {
        return Err(());
    }
    parse_long(first).ok_or(())
}

fn parse_long(text: &str) -> Option<i64> {
    let digits = text.strip_prefix(['+', '-']).unwrap_or(text);
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    text.parse().ok()
}

fn latin1(bytes: &[u8]) -> String {
    bytes.iter().map(|&b| b as char).collect()
}

fn java_trim(text: &str) -> &str {
    text.trim_matches(|c: char| c <= ' ')
}

fn is_valid_token_char(b: u8) -> bool {
    if b < 64 { TOKEN_CHARS_LOW & (1 << b) != 0 } else { b < 128 && TOKEN_CHARS_HIGH & (1 << (b - 64)) != 0 }
}

fn is_control_or_whitespace(b: u8) -> bool {
    b <= 0x20
}

fn is_whitespace(b: u8) -> bool {
    matches!(b, 0x09..=0x0D | 0x1C..=0x20)
}

fn is_sp_lenient(b: u8) -> bool {
    matches!(b, b' ' | 0x09 | 0x0B | 0x0C | 0x0D)
}

fn is_ows(b: u8) -> bool {
    b == b' ' || b == b'\t'
}

fn is_iso_control(b: u8) -> bool {
    b <= 0x1F || (0x7F..=0x9F).contains(&b)
}

pub enum Inbound {
    Request(HttpRequest),
    Write(&'static [u8]),
    WriteAndClose(&'static [u8]),
}

struct HttpObjectAggregator {
    current: Option<HttpRequest>,
    aggregating: bool,
    handle_incomplete_aggregate_during_close: bool,
}

impl HttpObjectAggregator {
    fn channel_read(&mut self, object: HttpObject, decoder: &mut HttpObjectDecoder, out: &mut VecDeque<Inbound>) {
        match object {
            HttpObject::FullMessage(request) => out.push_back(Inbound::Request(request)),
            HttpObject::Message { mut request, failed } => {
                self.aggregating = true;
                let content_length_invalid = request.content_length().is_some_and(|l| l > MAX_CONTENT_LENGTH as i64);
                let continue_response = if request.is_unsupported_expectation() {
                    decoder.reset();
                    Some(EXPECTATION_FAILED)
                } else if request.is_100_continue_expected() {
                    if content_length_invalid {
                        decoder.reset();
                        Some(TOO_LARGE)
                    } else {
                        Some(CONTINUE)
                    }
                } else {
                    None
                };
                if let Some(response) = continue_response {
                    request.remove_header("expect");
                    out.push_back(Inbound::Write(response));
                    if response != CONTINUE {
                        return;
                    }
                } else if content_length_invalid {
                    self.current = None;
                    self.handle_incomplete_aggregate_during_close = false;
                    out.push_back(if request.is_100_continue_expected() || request.is_keep_alive() {
                        Inbound::Write(TOO_LARGE)
                    } else {
                        Inbound::WriteAndClose(TOO_LARGE_CLOSE)
                    });
                    return;
                }
                request.set_transfer_encoding_unchunked();
                if failed {
                    self.finish_aggregation(request, out);
                } else {
                    self.current = Some(request);
                }
            }
            HttpObject::Content { data, last, failed } => {
                if !self.aggregating {
                    return;
                }
                let Some(current) = &mut self.current else { return };
                if current.content.len() > MAX_CONTENT_LENGTH - data.len() {
                    self.current = None;
                    self.handle_incomplete_aggregate_during_close = false;
                    out.push_back(Inbound::WriteAndClose(TOO_LARGE_CLOSE));
                    return;
                }
                current.content.extend_from_slice(&data);
                if last || failed {
                    let request = self.current.take().expect("a message");
                    self.finish_aggregation(request, out);
                }
            }
        }
    }

    fn finish_aggregation(&mut self, mut request: HttpRequest, out: &mut VecDeque<Inbound>) {
        self.aggregating = false;
        if !request.has_header("content-length") {
            let length = request.content.len().to_string();
            request.headers.push(("content-length".into(), length));
        }
        out.push_back(Inbound::Request(request));
    }
}

pub struct HttpServerCodec {
    buffer: Vec<u8>,
    pos: usize,
    decoder: HttpObjectDecoder,
    aggregator: HttpObjectAggregator,
    decoded: Vec<HttpObject>,
    pending: VecDeque<Inbound>,
}

impl HttpServerCodec {
    pub fn new() -> HttpServerCodec {
        HttpServerCodec {
            buffer: Vec::new(),
            pos: 0,
            decoder: HttpObjectDecoder::new(),
            aggregator: HttpObjectAggregator { current: None, aggregating: false, handle_incomplete_aggregate_during_close: true },
            decoded: Vec::new(),
            pending: VecDeque::new(),
        }
    }

    pub fn feed(&mut self, data: &[u8]) {
        if self.pos == self.buffer.len() {
            self.buffer.clear();
            self.pos = 0;
        }
        self.buffer.extend_from_slice(data);
    }

    pub fn poll(&mut self) -> Option<Inbound> {
        loop {
            if let Some(inbound) = self.pending.pop_front() {
                return Some(inbound);
            }
            if self.pos == self.buffer.len() {
                return None;
            }
            let before = self.pos;
            self.decoder.decode(&self.buffer, &mut self.pos, &mut self.decoded);
            if self.decoded.is_empty() && before == self.pos {
                return None;
            }
            self.deliver();
        }
    }

    fn deliver(&mut self) {
        for object in std::mem::take(&mut self.decoded) {
            self.aggregator.channel_read(object, &mut self.decoder, &mut self.pending);
        }
    }

    pub fn close(&mut self) -> bool {
        self.decoder.decode_last(&self.buffer, &mut self.pos, &mut self.decoded);
        self.deliver();
        self.pending.clear();
        self.aggregator.aggregating && self.aggregator.handle_incomplete_aggregate_during_close
    }

    pub fn into_remaining(self) -> Vec<u8> {
        self.buffer[self.pos..].to_vec()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn requests(input: &str) -> Vec<String> {
        let mut codec = HttpServerCodec::new();
        codec.feed(input.replace('\n', "\r\n").as_bytes());
        let mut result = Vec::new();
        while let Some(inbound) = codec.poll() {
            result.push(match inbound {
                Inbound::Request(request) => format!("{} {} {} {}", request.method, request.uri, request.names(), request.content.len()),
                Inbound::Write(response) => format!("write {}", String::from_utf8_lossy(response).lines().next().unwrap_or_default()),
                Inbound::WriteAndClose(response) => format!("close {}", String::from_utf8_lossy(response).lines().next().unwrap_or_default()),
            });
        }
        result
    }

    #[test]
    fn header_names_match_vanilla() {
        assert_eq!(requests("GET / HTTP/1.1\nHost: a\nContent-Length: 2\nX-A: 1\n\nab"), ["GET / [Host, Content-Length, X-A] 2"]);
        assert_eq!(requests("GET / HTTP/1.1\nHost: a\nTransfer-Encoding: chunked\nX-A: 1\n\n3;ext=1\nabc\n0\nX-Trail: t\n\n"), ["GET / [Host, X-A, content-length] 3"]);
        assert_eq!(requests("GET / HTTP/1.1\nTransfer-Encoding: gzip, chunked\n\n1\na\n0\n\n"), ["GET / [transfer-encoding, content-length] 1"]);
        assert_eq!(requests("GET / HTTP/1.1\nTransfer-Encoding: chunked\n\nzz\nabc\n0\n\n"), ["GET / [content-length] 0"]);
        assert_eq!(requests("GET / HTTP/1.1\nContent-Length: 5\nTransfer-Encoding: chunked\n\n1\na\n0\n\n"), ["GET / [Content-Length] 0"]);
        assert_eq!(requests("GET / HTTP/1.1\nContent-Length: abc\n\n"), ["GET / [Content-Length] 0"]);
        assert_eq!(requests("GET / HTTP/1.1\nContent-Length: 1\nContent-Length: 1\n\na"), ["GET / [Content-Length] 0"]);
        assert_eq!(requests("hello there\n\n"), ["GET /bad-request [] 0"]);
    }

    #[test]
    fn expectations_and_oversized_requests() {
        assert_eq!(requests("POST / HTTP/1.1\nExpect: 100-continue\nContent-Length: 2\n\nab"), ["write HTTP/1.1 100 Continue", "POST / [Content-Length] 2"]);
        assert_eq!(requests("POST / HTTP/1.1\nExpect: foo\nContent-Length: 2\n\nGET /x HTTP/1.1\n\n"), ["write HTTP/1.1 417 Expectation Failed", "GET /x [content-length] 0"]);
        assert_eq!(requests("POST / HTTP/1.1\nContent-Length: 70000\n\n"), ["write HTTP/1.1 413 Request Entity Too Large"]);
        assert_eq!(requests("POST / HTTP/1.0\nContent-Length: 70000\n\n"), ["close HTTP/1.1 413 Request Entity Too Large"]);
    }

    #[test]
    fn premature_closure() {
        let mut codec = HttpServerCodec::new();
        codec.feed(b"POST / HTTP/1.1\r\nContent-Length: 3\r\n\r\nab");
        assert!(codec.poll().is_none());
        assert!(codec.close());
        let mut codec = HttpServerCodec::new();
        codec.feed(b"GET / HTTP/1.1\r\nHost: a\r\n");
        assert!(codec.poll().is_none());
        assert!(!codec.close());
    }
}
