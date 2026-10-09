use std::fmt;

const L_DIGIT: u64 = 0x3FF000000000000;
const H_DIGIT: u64 = 0;
const L_UPALPHA: u64 = 0;
const H_UPALPHA: u64 = 0x7FFFFFE;
const L_LOWALPHA: u64 = 0;
const H_LOWALPHA: u64 = 0x7FFFFFE00000000;
const L_ALPHA: u64 = L_LOWALPHA | L_UPALPHA;
const H_ALPHA: u64 = H_LOWALPHA | H_UPALPHA;
const L_ALPHANUM: u64 = L_DIGIT | L_ALPHA;
const H_ALPHANUM: u64 = H_DIGIT | H_ALPHA;
const L_HEX: u64 = L_DIGIT;
const H_HEX: u64 = 0x7E0000007E;
const L_MARK: u64 = 0x678200000000;
const H_MARK: u64 = 0x4000000080000000;
const L_UNRESERVED: u64 = L_ALPHANUM | L_MARK;
const H_UNRESERVED: u64 = H_ALPHANUM | H_MARK;
const L_RESERVED: u64 = 0xAC00985000000000;
const H_RESERVED: u64 = 0x28000001;
const L_ESCAPED: u64 = 1;
const H_ESCAPED: u64 = 0;
const L_URIC: u64 = L_RESERVED | L_UNRESERVED | L_ESCAPED;
const H_URIC: u64 = H_RESERVED | H_UNRESERVED | H_ESCAPED;
const L_PCHAR: u64 = L_UNRESERVED | L_ESCAPED | 0x2400185000000000;
const H_PCHAR: u64 = H_UNRESERVED | H_ESCAPED | 0x1;
const L_PATH: u64 = L_PCHAR | 0x800800000000000;
const H_PATH: u64 = H_PCHAR;
const L_DASH: u64 = 0x200000000000;
const H_DASH: u64 = 0;
const L_DOT: u64 = 0x400000000000;
const H_DOT: u64 = 0;
const L_USERINFO: u64 = L_UNRESERVED | L_ESCAPED | 0x2C00185000000000;
const H_USERINFO: u64 = H_UNRESERVED | H_ESCAPED;
const L_REG_NAME: u64 = L_UNRESERVED | L_ESCAPED | 0x2C00185000000000;
const H_REG_NAME: u64 = H_UNRESERVED | H_ESCAPED | 0x1;
const L_SERVER: u64 = L_USERINFO | L_ALPHANUM | L_DASH | 0x400400000000000;
const H_SERVER: u64 = H_USERINFO | H_ALPHANUM | H_DASH | 0x28000001;
const L_SERVER_PERCENT: u64 = L_SERVER | 0x2000000000;
const H_SERVER_PERCENT: u64 = H_SERVER;
const L_SCHEME: u64 = L_ALPHA | L_DIGIT | 0x680000000000;
const H_SCHEME: u64 = H_ALPHA | H_DIGIT;
const L_SCOPE_ID: u64 = L_ALPHANUM | 0x400000000000;
const H_SCOPE_ID: u64 = H_ALPHANUM | 0x80000000;

#[derive(Clone, Debug, PartialEq)]
pub struct URISyntaxException {
    input: String,
    reason: String,
    index: i32,
}

impl fmt::Display for URISyntaxException {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.reason)?;
        if self.index > -1 {
            write!(f, " at index {}", self.index)?;
        }
        write!(f, ": {}", self.input)
    }
}

impl URISyntaxException {
    pub fn new(input: &str, reason: String, index: i32) -> URISyntaxException {
        URISyntaxException { input: input.to_string(), reason, index }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct URI {
    string: String,
    scheme: Option<String>,
}

impl URI {
    pub fn new(s: &str) -> Result<URI, URISyntaxException> {
        let mut parser = Parser { input: s, chars: s.encode_utf16().collect(), scheme: None, ipv6_byte_count: 0 };
        parser.parse()?;
        Ok(URI { string: s.to_string(), scheme: parser.scheme })
    }

    pub fn scheme(&self) -> Option<&str> {
        self.scheme.as_deref()
    }
}

impl fmt::Display for URI {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.string)
    }
}

fn matches(c: u16, low_mask: u64, high_mask: u64) -> bool {
    match c {
        0 => false,
        1..64 => (1u64 << c) & low_mask != 0,
        64..128 => (1u64 << (c - 64)) & high_mask != 0,
        _ => false,
    }
}

fn is_space_char(c: u16) -> bool {
    matches!(c, 0x20 | 0xA0 | 0x1680 | 0x2000..=0x200A | 0x2028 | 0x2029 | 0x202F | 0x205F | 0x3000)
}

fn is_iso_control(c: u16) -> bool {
    c <= 0x1F || (0x7F..=0x9F).contains(&c)
}

struct Parser<'a> {
    input: &'a str,
    chars: Vec<u16>,
    scheme: Option<String>,
    ipv6_byte_count: i32,
}

type ParseResult<T> = std::result::Result<T, URISyntaxException>;

impl Parser<'_> {
    fn fail<T>(&self, reason: impl Into<String>, p: usize) -> ParseResult<T> {
        Err(URISyntaxException::new(self.input, reason.into(), p as i32))
    }

    fn fail_without_index<T>(&self, reason: &str) -> ParseResult<T> {
        Err(URISyntaxException::new(self.input, reason.to_string(), -1))
    }

    fn fail_expecting<T>(&self, expected: &str, p: usize) -> ParseResult<T> {
        self.fail(format!("Expected {expected}"), p)
    }

    fn char_at(&self, i: usize) -> u16 {
        self.chars[i]
    }

    fn at(&self, start: usize, end: usize, c: u8) -> bool {
        start < end && self.char_at(start) == c as u16
    }

    fn at_str(&self, start: usize, end: usize, s: &str) -> bool {
        let s = s.as_bytes();
        if s.len() > end.saturating_sub(start) {
            return false;
        }
        s.iter().enumerate().all(|(i, &c)| self.char_at(start + i) == c as u16)
    }

    fn scan_char(&self, start: usize, end: usize, c: u8) -> usize {
        if start < end && self.char_at(start) == c as u16 { start + 1 } else { start }
    }

    fn scan_err_stop(&self, start: usize, end: usize, err: &str, stop: &str) -> Option<usize> {
        let mut p = start;
        while p < end {
            let c = self.char_at(p);
            if err.encode_utf16().any(|e| e == c) {
                return None;
            }
            if stop.encode_utf16().any(|s| s == c) {
                break;
            }
            p += 1;
        }
        Some(p)
    }

    fn scan_stop(&self, start: usize, end: usize, stop: &str) -> usize {
        let mut p = start;
        while p < end && !stop.encode_utf16().any(|s| s == self.char_at(p)) {
            p += 1;
        }
        p
    }

    fn scan_escape(&self, start: usize, n: usize, first: u16) -> ParseResult<usize> {
        let p = start;
        if first == b'%' as u16 {
            if p + 3 <= n && matches(self.char_at(p + 1), L_HEX, H_HEX) && matches(self.char_at(p + 2), L_HEX, H_HEX) {
                return Ok(p + 3);
            }
            return self.fail("Malformed escape pair", p);
        } else if first > 128 && !is_space_char(first) && !is_iso_control(first) {
            return Ok(p + 1);
        }
        Ok(p)
    }

    fn scan_mask(&self, start: usize, n: usize, low_mask: u64, high_mask: u64) -> ParseResult<usize> {
        let mut p = start;
        while p < n {
            let c = self.char_at(p);
            if matches(c, low_mask, high_mask) {
                p += 1;
                continue;
            }
            if low_mask & L_ESCAPED != 0 {
                let q = self.scan_escape(p, n, c)?;
                if q > p {
                    p = q;
                    continue;
                }
            }
            break;
        }
        Ok(p)
    }

    fn check_chars(&self, start: usize, end: usize, low_mask: u64, high_mask: u64, what: &str) -> ParseResult<()> {
        let p = self.scan_mask(start, end, low_mask, high_mask)?;
        if p < end {
            return self.fail(format!("Illegal character in {what}"), p);
        }
        Ok(())
    }

    fn check_char(&self, p: usize, low_mask: u64, high_mask: u64, what: &str) -> ParseResult<()> {
        self.check_chars(p, p + 1, low_mask, high_mask, what)
    }

    fn parse(&mut self) -> ParseResult<()> {
        let n = self.chars.len();
        let scanned = self.scan_err_stop(0, n, "/?#", ":");
        let mut p;
        if let Some(colon) = scanned
            && self.at(colon, n, b':')
        {
            if colon == 0 {
                return self.fail_expecting("scheme name", 0);
            }
            self.check_char(0, L_ALPHA, H_ALPHA, "scheme name")?;
            self.check_chars(1, colon, L_SCHEME, H_SCHEME, "scheme name")?;
            self.scheme = Some(String::from_utf16_lossy(&self.chars[..colon]));
            p = colon + 1;
            if self.at(p, n, b'/') {
                p = self.parse_hierarchical(p, n)?;
            } else {
                let q = self.scan_stop(p, n, "#");
                if q <= p {
                    return self.fail_expecting("scheme-specific part", p);
                }
                self.check_chars(p, q, L_URIC, H_URIC, "opaque part")?;
                p = q;
            }
        } else {
            p = self.parse_hierarchical(0, n)?;
        }
        if self.at(p, n, b'#') {
            self.check_chars(p + 1, n, L_URIC, H_URIC, "fragment")?;
            p = n;
        }
        if p < n {
            return self.fail("end of URI", p);
        }
        Ok(())
    }

    fn parse_hierarchical(&mut self, start: usize, n: usize) -> ParseResult<usize> {
        let mut p = start;
        if self.at(p, n, b'/') && self.at(p + 1, n, b'/') {
            p += 2;
            let q = self.scan_stop(p, n, "/?#");
            if q > p {
                p = self.parse_authority(p, q)?;
            } else if q >= n {
                return self.fail_expecting("authority", p);
            }
        }
        let q = self.scan_stop(p, n, "?#");
        self.check_chars(p, q, L_PATH, H_PATH, "path")?;
        p = q;
        if self.at(p, n, b'?') {
            p += 1;
            let q = self.scan_stop(p, n, "#");
            self.check_chars(p, q, L_URIC, H_URIC, "query")?;
            p = q;
        }
        Ok(p)
    }

    fn parse_authority(&mut self, start: usize, n: usize) -> ParseResult<usize> {
        let p = start;
        let mut q = p;
        let mut ex = None;
        let server_chars = if self.scan_stop(p, n, "]") > p {
            self.scan_mask(p, n, L_SERVER_PERCENT, H_SERVER_PERCENT)? == n
        } else {
            self.scan_mask(p, n, L_SERVER, H_SERVER)? == n
        };
        let qreg = self.scan_mask(p, n, L_REG_NAME, H_REG_NAME)?;
        let reg_chars = qreg == n;
        if reg_chars && !server_chars {
            return Ok(n);
        }
        let skip_parse_exception = reg_chars;
        if server_chars {
            match self.parse_server(p, n, skip_parse_exception) {
                Ok(end) => {
                    q = end;
                    if q < n && skip_parse_exception {
                        q = p;
                    } else if q < n {
                        return self.fail_expecting("end of authority", q);
                    }
                }
                Err(x) => {
                    ex = Some(x);
                    q = p;
                }
            }
        }
        if q < n {
            if reg_chars {
            } else if let Some(ex) = ex {
                return Err(ex);
            } else {
                return self.fail("Illegal character in authority", if server_chars { q } else { qreg });
            }
        }
        Ok(n)
    }

    fn parse_server(&mut self, start: usize, n: usize, skip_parse_exception: bool) -> ParseResult<usize> {
        let mut p = start;
        if let Some(q) = self.scan_err_stop(p, n, "/?#", "@")
            && q >= p
            && self.at(q, n, b'@')
        {
            self.check_chars(p, q, L_USERINFO, H_USERINFO, "user info")?;
            p = q + 1;
        }
        if self.at(p, n, b'[') {
            p += 1;
            let q = self.scan_err_stop(p, n, "/?#", "]");
            match q {
                Some(q) if q > p && self.at(q, n, b']') => {
                    let r = self.scan_stop(p, q, "%");
                    if r > p {
                        self.parse_ipv6_reference(p, r)?;
                        if r + 1 == q {
                            return self.fail_without_index("scope id expected");
                        }
                        self.check_chars(r + 1, q, L_SCOPE_ID, H_SCOPE_ID, "scope id")?;
                    } else {
                        self.parse_ipv6_reference(p, q)?;
                    }
                    p = q + 1;
                }
                q => {
                    let at = q.map_or(-1, |q| q as i32);
                    return Err(URISyntaxException::new(self.input, "Expected closing bracket for IPv6 address".into(), at));
                }
            }
        } else {
            let q = self.parse_ipv4_address(p, n);
            p = if q <= p as isize { self.parse_hostname(p, n, skip_parse_exception)? } else { q as usize };
        }
        if self.at(p, n, b':') {
            p += 1;
            let q = self.scan_stop(p, n, "/");
            if q > p {
                self.check_chars(p, q, L_DIGIT, H_DIGIT, "port number")?;
                if String::from_utf16_lossy(&self.chars[p..q]).parse::<i32>().is_err() {
                    return self.fail("Malformed port number", p);
                }
                p = q;
            }
        } else if p < n && skip_parse_exception {
            return Ok(p);
        }
        if p < n {
            return self.fail_expecting("port number", p);
        }
        Ok(p)
    }

    fn scan_byte(&self, start: usize, n: usize) -> ParseResult<usize> {
        let p = start;
        let q = self.scan_mask(p, n, L_DIGIT, H_DIGIT)?;
        if q <= p {
            return Ok(q);
        }
        let mut i = p;
        loop {
            let j = self.scan_char(i, q, b'0');
            if j <= i {
                break;
            }
            i = j;
        }
        let significant = q - i;
        if significant < 3 {
            return Ok(q);
        }
        if significant > 3 {
            return Ok(p);
        }
        let value: i32 = String::from_utf16_lossy(&self.chars[p..q]).parse().unwrap_or(i32::MAX);
        Ok(if value > 255 { p } else { q })
    }

    fn scan_ipv4_address(&self, start: usize, n: usize, strict: bool) -> ParseResult<isize> {
        let mut p = start;
        let m = self.scan_mask(p, n, L_DIGIT | L_DOT, H_DIGIT | H_DOT)?;
        if m <= p || (strict && m != n) {
            return Ok(-1);
        }
        let mut q;
        loop {
            q = self.scan_byte(p, m)?;
            if q <= p {
                break;
            }
            p = q;
            q = self.scan_char(p, m, b'.');
            if q <= p {
                break;
            }
            p = q;
            q = self.scan_byte(p, m)?;
            if q <= p {
                break;
            }
            p = q;
            q = self.scan_char(p, m, b'.');
            if q <= p {
                break;
            }
            p = q;
            q = self.scan_byte(p, m)?;
            if q <= p {
                break;
            }
            p = q;
            q = self.scan_char(p, m, b'.');
            if q <= p {
                break;
            }
            p = q;
            q = self.scan_byte(p, m)?;
            if q <= p {
                break;
            }
            if q < m {
                break;
            }
            return Ok(q as isize);
        }
        if strict {
            return self.fail("Malformed IPv4 address", q);
        }
        Ok(-1)
    }

    fn take_ipv4_address(&self, start: usize, n: usize, expected: &str) -> ParseResult<usize> {
        let p = self.scan_ipv4_address(start, n, true)?;
        if p <= start as isize {
            return self.fail_expecting(expected, start);
        }
        Ok(p as usize)
    }

    fn parse_ipv4_address(&self, start: usize, n: usize) -> isize {
        let Ok(p) = self.scan_ipv4_address(start, n, false) else { return -1 };
        if p == -1 {
            return p;
        }
        if p > start as isize && (p as usize) < n && self.char_at(p as usize) != b':' as u16 {
            return -1;
        }
        p
    }

    fn parse_hostname(&self, start: usize, n: usize, skip_parse_exception: bool) -> ParseResult<usize> {
        let mut p = start;
        let mut l = None;
        loop {
            let q = self.scan_mask(p, n, L_ALPHANUM, H_ALPHANUM)?;
            if q <= p {
                break;
            }
            l = Some(p);
            p = q;
            let q = self.scan_mask(p, n, L_ALPHANUM | L_DASH, H_ALPHANUM | H_DASH)?;
            if q > p {
                if self.char_at(q - 1) == b'-' as u16 {
                    return self.fail("Illegal character in hostname", q - 1);
                }
                p = q;
            }
            let q = self.scan_char(p, n, b'.');
            if q <= p {
                break;
            }
            p = q;
            if p >= n {
                break;
            }
        }
        if p < n && !self.at(p, n, b':') {
            if skip_parse_exception {
                return Ok(p);
            }
            return self.fail("Illegal character in hostname", p);
        }
        let Some(l) = l else { return self.fail_expecting("hostname", start) };
        if l > start && !matches(self.char_at(l), L_ALPHA, H_ALPHA) {
            return self.fail("Illegal character in hostname", l);
        }
        Ok(p)
    }

    fn parse_ipv6_reference(&mut self, start: usize, n: usize) -> ParseResult<usize> {
        let mut p = start;
        let mut compressed_zeros = false;
        let q = self.scan_hex_seq(p, n)?;
        if q > p as isize {
            p = q as usize;
            if self.at_str(p, n, "::") {
                compressed_zeros = true;
                p = self.scan_hex_post(p + 2, n)?;
            } else if self.at(p, n, b':') {
                p = self.take_ipv4_address(p + 1, n, "IPv4 address")?;
                self.ipv6_byte_count += 4;
            }
        } else if self.at_str(p, n, "::") {
            compressed_zeros = true;
            p = self.scan_hex_post(p + 2, n)?;
        }
        if p < n {
            return self.fail("Malformed IPv6 address", start);
        }
        if self.ipv6_byte_count > 16 {
            return self.fail("IPv6 address too long", start);
        }
        if !compressed_zeros && self.ipv6_byte_count < 16 {
            return self.fail("IPv6 address too short", start);
        }
        if compressed_zeros && self.ipv6_byte_count == 16 {
            return self.fail("Malformed IPv6 address", start);
        }
        Ok(p)
    }

    fn scan_hex_post(&mut self, start: usize, n: usize) -> ParseResult<usize> {
        let mut p = start;
        if p == n {
            return Ok(p);
        }
        let q = self.scan_hex_seq(p, n)?;
        if q > p as isize {
            p = q as usize;
            if self.at(p, n, b':') {
                p += 1;
                p = self.take_ipv4_address(p, n, "hex digits or IPv4 address")?;
                self.ipv6_byte_count += 4;
            }
        } else {
            p = self.take_ipv4_address(p, n, "hex digits or IPv4 address")?;
            self.ipv6_byte_count += 4;
        }
        Ok(p)
    }

    fn scan_hex_seq(&mut self, start: usize, n: usize) -> ParseResult<isize> {
        let mut p = start;
        let q = self.scan_mask(p, n, L_HEX, H_HEX)?;
        if q <= p {
            return Ok(-1);
        }
        if self.at(q, n, b'.') {
            return Ok(-1);
        }
        if q > p + 4 {
            return self.fail("IPv6 hexadecimal digit sequence too long", p);
        }
        self.ipv6_byte_count += 2;
        p = q;
        while p < n {
            if !self.at(p, n, b':') {
                break;
            }
            if self.at(p + 1, n, b':') {
                break;
            }
            p += 1;
            let q = self.scan_mask(p, n, L_HEX, H_HEX)?;
            if q <= p {
                return self.fail_expecting("digits for an IPv6 address", p);
            }
            if self.at(q, n, b'.') {
                p -= 1;
                break;
            }
            if q > p + 4 {
                return self.fail("IPv6 hexadecimal digit sequence too long", p);
            }
            self.ipv6_byte_count += 2;
            p = q;
        }
        Ok(p as isize)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn matches_java_net_uri() {
        let cases = [
            ("https://example.com/bugs", "OK https://example.com/bugs"),
            ("notaurl", "ERR Missing protocol in URI: notaurl: notaurl"),
            ("ftp://x.y", "ERR Unsupported protocol in URI: ftp://x.y: ftp://x.y"),
            ("http://", "ERR Expected authority at index 7: http://"),
            ("https://a b", "ERR Illegal character in authority at index 9: https://a b"),
            ("http://[::1]/", "OK http://[::1]/"),
            ("http://[::1", "ERR Expected closing bracket for IPv6 address at index 11: http://[::1"),
            ("http://[1:2:3:4:5:6:7:8:9]/", "ERR IPv6 address too long at index 8: http://[1:2:3:4:5:6:7:8:9]/"),
            ("http://host:99999999999/", "OK http://host:99999999999/"),
            ("http://host:12a/", "OK http://host:12a/"),
            ("http://-bad.com/", "OK http://-bad.com/"),
            ("http://bad-.com/", "OK http://bad-.com/"),
            ("http://1.2.3.4:80/x?q#f", "OK http://1.2.3.4:80/x?q#f"),
            ("http://300.1.1.1/", "OK http://300.1.1.1/"),
            ("https://ex.com/%zz", "ERR Malformed escape pair at index 15: https://ex.com/%zz"),
            ("https://ex.com/\u{e4}", "OK https://ex.com/\u{e4}"),
            ("https://ex.com/ \u{a0}", "ERR Illegal character in path at index 15: https://ex.com/ \u{a0}"),
            (":nope", "ERR Expected scheme name at index 0: :nope"),
            ("1http://x", "ERR Illegal character in scheme name at index 0: 1http://x"),
            ("ht~tp://x", "ERR Illegal character in scheme name at index 2: ht~tp://x"),
            ("mailto:a@b.c", "ERR Unsupported protocol in URI: mailto:a@b.c: mailto:a@b.c"),
            ("http:opaque", "OK http:opaque"),
            ("http:#frag", "ERR Expected scheme-specific part at index 5: http:#frag"),
            ("https://user@host.com:8080/p", "OK https://user@host.com:8080/p"),
            ("https://user@:80/", "OK https://user@:80/"),
            ("http://[fe80::1%eth0]/", "OK http://[fe80::1%eth0]/"),
            ("http://[fe80::1%]/", "ERR scope id expected: http://[fe80::1%]/"),
            ("http://[::ffff:1.2.3.4]/", "OK http://[::ffff:1.2.3.4]/"),
            ("http://[::ffff:1.2.3]/", "ERR Malformed IPv4 address at index 20: http://[::ffff:1.2.3]/"),
            ("https://ex.com/a|b", "ERR Illegal character in path at index 16: https://ex.com/a|b"),
            ("https://ex.com/?a=b c", "ERR Illegal character in query at index 19: https://ex.com/?a=b c"),
            ("", "ERR Missing protocol in URI: : "),
            ("https://ex.com#a#b", "ERR Illegal character in fragment at index 16: https://ex.com#a#b"),
            ("http://a.b.c.d.e/", "OK http://a.b.c.d.e/"),
            ("http://1.2.3/", "OK http://1.2.3/"),
            ("http://a_b.com/", "OK http://a_b.com/"),
            ("https://ex.com:/", "OK https://ex.com:/"),
            ("HTTPS://EX.COM", "OK HTTPS://EX.COM"),
            ("http://[12345::]/", "ERR IPv6 hexadecimal digit sequence too long at index 8: http://[12345::]/"),
            ("http://[1:2:3:4:5:6:7]/", "ERR IPv6 address too short at index 8: http://[1:2:3:4:5:6:7]/"),
            ("http://[1::2::3]/", "ERR Expected hex digits or IPv4 address at index 13: http://[1::2::3]/"),
        ];
        for (input, expected) in cases {
            let got = match crate::util::parse_and_validate_untrusted_uri(input) {
                Ok(uri) => format!("OK {uri}"),
                Err(e) => format!("ERR {e}"),
            };
            assert_eq!(got, expected, "{input:?}");
        }
    }
}
