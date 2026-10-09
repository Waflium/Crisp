use crisp_protocol::nbt::Nbt;

use crate::commands::brigadier::{DynamicCommandExceptionType, SimpleCommandExceptionType, tr, utf16};
use crate::util::parsing::packrat::{Delayed, ParseState};

static ERROR_NUMBER_PARSE_FAILURE: DynamicCommandExceptionType = DynamicCommandExceptionType(|m| tr("snbt.parser.number_parse_failure", vec![m]));
static ERROR_EXPECTED_HEX_ESCAPE: DynamicCommandExceptionType = DynamicCommandExceptionType(|l| tr("snbt.parser.expected_hex_escape", vec![l]));
static ERROR_INVALID_CODEPOINT: DynamicCommandExceptionType = DynamicCommandExceptionType(|c| tr("snbt.parser.invalid_codepoint", vec![c]));
static ERROR_NO_SUCH_OPERATION: DynamicCommandExceptionType = DynamicCommandExceptionType(|o| tr("snbt.parser.no_such_operation", vec![o]));
static ERROR_EXPECTED_INTEGER_TYPE: SimpleCommandExceptionType = SimpleCommandExceptionType("snbt.parser.expected_integer_type");
static ERROR_EXPECTED_FLOAT_TYPE: SimpleCommandExceptionType = SimpleCommandExceptionType("snbt.parser.expected_float_type");
static ERROR_EXPECTED_NON_NEGATIVE_NUMBER: SimpleCommandExceptionType = SimpleCommandExceptionType("snbt.parser.expected_non_negative_number");
static ERROR_INVALID_CHARACTER_NAME: SimpleCommandExceptionType = SimpleCommandExceptionType("snbt.parser.invalid_character_name");
static ERROR_INVALID_ARRAY_ELEMENT_TYPE: SimpleCommandExceptionType = SimpleCommandExceptionType("snbt.parser.invalid_array_element_type");
static ERROR_INVALID_UNQUOTED_START: SimpleCommandExceptionType = SimpleCommandExceptionType("snbt.parser.invalid_unquoted_start");
static ERROR_EXPECTED_UNQUOTED_STRING: SimpleCommandExceptionType = SimpleCommandExceptionType("snbt.parser.expected_unquoted_string");
static ERROR_INVALID_STRING_CONTENTS: SimpleCommandExceptionType = SimpleCommandExceptionType("snbt.parser.invalid_string_contents");
static ERROR_EXPECTED_BINARY_NUMERAL: SimpleCommandExceptionType = SimpleCommandExceptionType("snbt.parser.expected_binary_numeral");
static ERROR_UNDESCORE_NOT_ALLOWED: SimpleCommandExceptionType = SimpleCommandExceptionType("snbt.parser.underscore_not_allowed");
static ERROR_EXPECTED_DECIMAL_NUMERAL: SimpleCommandExceptionType = SimpleCommandExceptionType("snbt.parser.expected_decimal_numeral");
static ERROR_EXPECTED_HEX_NUMERAL: SimpleCommandExceptionType = SimpleCommandExceptionType("snbt.parser.expected_hex_numeral");
static ERROR_EMPTY_KEY: SimpleCommandExceptionType = SimpleCommandExceptionType("snbt.parser.empty_key");
static ERROR_LEADING_ZERO_NOT_ALLOWED: SimpleCommandExceptionType = SimpleCommandExceptionType("snbt.parser.leading_zero_not_allowed");
static ERROR_INFINITY_NOT_ALLOWED: SimpleCommandExceptionType = SimpleCommandExceptionType("snbt.parser.infinity_not_allowed");
static ERROR_EXPECTED_STRING_UUID: SimpleCommandExceptionType = SimpleCommandExceptionType("snbt.parser.expected_string_uuid");
static ERROR_EXPECTED_NUMBER_OR_BOOLEAN: SimpleCommandExceptionType = SimpleCommandExceptionType("snbt.parser.expected_number_or_boolean");

type State<'r, 'a> = ParseState<'r, 'a>;

fn simple(e: &'static SimpleCommandExceptionType) -> Delayed {
    Delayed::Simple(e)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Sign {
    Plus,
    Minus,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum TypeSuffix {
    Float,
    Double,
    Byte,
    Short,
    Int,
    Long,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SignedPrefix {
    Signed,
    Unsigned,
}

#[derive(Clone, Copy)]
struct IntegerSuffix {
    signed: Option<SignedPrefix>,
    ty: Option<TypeSuffix>,
}

impl IntegerSuffix {
    const EMPTY: IntegerSuffix = IntegerSuffix { signed: None, ty: None };
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Base {
    Binary,
    Decimal,
    Hex,
}

struct IntegerLiteral {
    sign: Sign,
    base: Base,
    digits: String,
    suffix: IntegerSuffix,
}

#[derive(Clone, Copy)]
enum Number {
    Byte(i8),
    Short(i16),
    Int(i32),
    Long(i64),
}

impl Number {
    fn to_nbt(self) -> Nbt {
        match self {
            Number::Byte(v) => Nbt::Byte(v),
            Number::Short(v) => Nbt::Short(v),
            Number::Int(v) => Nbt::Int(v),
            Number::Long(v) => Nbt::Long(v),
        }
    }

    fn long_value(self) -> i64 {
        match self {
            Number::Byte(v) => v as i64,
            Number::Short(v) => v as i64,
            Number::Int(v) => v as i64,
            Number::Long(v) => v,
        }
    }
}

impl IntegerLiteral {
    fn signed_or_default(&self) -> SignedPrefix {
        match (self.suffix.signed, self.base) {
            (Some(signed), _) => signed,
            (None, Base::Binary | Base::Hex) => SignedPrefix::Unsigned,
            (None, Base::Decimal) => SignedPrefix::Signed,
        }
    }

    fn cleanup_digits(&self, sign: Sign) -> String {
        let mut result = String::new();
        if sign == Sign::Minus {
            result.push('-');
        }
        result.extend(self.digits.chars().filter(|&c| c != '_'));
        result
    }

    fn create(&self, state: &mut State) -> Option<Number> {
        self.create_as(self.suffix.ty.unwrap_or(TypeSuffix::Int), state)
    }

    fn create_as(&self, ty: TypeSuffix, state: &mut State) -> Option<Number> {
        let signed = self.signed_or_default() == SignedPrefix::Signed;
        if !signed && self.sign == Sign::Minus {
            state.store_delayed(state.mark(), simple(&ERROR_EXPECTED_NON_NEGATIVE_NUMBER));
            return None;
        }
        let digits = self.cleanup_digits(self.sign);
        let radix = match self.base {
            Base::Binary => 2,
            Base::Decimal => 10,
            Base::Hex => 16,
        };
        let result = match (signed, ty) {
            (true, TypeSuffix::Byte) => java::parse_byte(&digits, radix).map(Number::Byte),
            (true, TypeSuffix::Short) => java::parse_short(&digits, radix).map(Number::Short),
            (true, TypeSuffix::Int) => java::parse_int(&digits, radix).map(Number::Int),
            (true, TypeSuffix::Long) => java::parse_long(&digits, radix).map(Number::Long),
            (false, TypeSuffix::Byte) => java::parse_unsigned_byte(&digits, radix).map(Number::Byte),
            (false, TypeSuffix::Short) => java::parse_unsigned_short(&digits, radix).map(Number::Short),
            (false, TypeSuffix::Int) => java::parse_unsigned_int(&digits, radix).map(Number::Int),
            (false, TypeSuffix::Long) => java::parse_unsigned_long(&digits, radix).map(Number::Long),
            (_, TypeSuffix::Float | TypeSuffix::Double) => {
                state.store_delayed(state.mark(), simple(&ERROR_EXPECTED_INTEGER_TYPE));
                return None;
            }
        };
        match result {
            Ok(number) => Some(number),
            Err(message) => {
                state.store_delayed(state.mark(), Delayed::Dynamic(&ERROR_NUMBER_PARSE_FAILURE, message));
                None
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ArrayPrefix {
    Byte,
    Int,
    Long,
}

impl ArrayPrefix {
    fn default_type(self) -> TypeSuffix {
        match self {
            ArrayPrefix::Byte => TypeSuffix::Byte,
            ArrayPrefix::Int => TypeSuffix::Int,
            ArrayPrefix::Long => TypeSuffix::Long,
        }
    }

    fn is_allowed(self, ty: TypeSuffix) -> bool {
        ty == self.default_type()
            || match self {
                ArrayPrefix::Byte => false,
                ArrayPrefix::Int => matches!(ty, TypeSuffix::Byte | TypeSuffix::Short),
                ArrayPrefix::Long => matches!(ty, TypeSuffix::Byte | TypeSuffix::Short | TypeSuffix::Int),
            }
    }

    fn empty(self) -> Nbt {
        match self {
            ArrayPrefix::Byte => Nbt::ByteArray(Vec::new()),
            ArrayPrefix::Int => Nbt::IntArray(Vec::new()),
            ArrayPrefix::Long => Nbt::LongArray(Vec::new()),
        }
    }

    fn create(self, entries: &[IntegerLiteral], state: &mut State) -> Option<Nbt> {
        let mut values = Vec::with_capacity(entries.len());
        for entry in entries {
            let ty = match entry.suffix.ty {
                None => self.default_type(),
                Some(ty) if self.is_allowed(ty) => ty,
                Some(_) => {
                    state.store_delayed(state.mark(), simple(&ERROR_INVALID_ARRAY_ELEMENT_TYPE));
                    return None;
                }
            };
            values.push(entry.create_as(ty, state)?.long_value());
        }
        Some(match self {
            ArrayPrefix::Byte => Nbt::ByteArray(values.into_iter().map(|v| v as i8).collect()),
            ArrayPrefix::Int => Nbt::IntArray(values.into_iter().map(|v| v as i32).collect()),
            ArrayPrefix::Long => Nbt::LongArray(values),
        })
    }
}

fn can_start_number(c: u16) -> bool {
    matches!(c, 0x2B | 0x2D | 0x2E | 0x30..=0x39)
}

fn is_allowed_to_start_unquoted_string(c: u16) -> bool {
    !can_start_number(c)
}

fn optional<T>(s: &mut State, rule: impl FnOnce(&mut State) -> Option<T>) -> Option<T> {
    let mark = s.mark();
    let result = rule(s);
    if result.is_none() {
        s.restore(mark);
    }
    result
}

fn repeated_with_trailing_separator<T>(s: &mut State, mut element: impl FnMut(&mut State) -> Option<T>) -> Vec<T> {
    let mut elements = Vec::new();
    let mut first = true;
    loop {
        let before_separator = s.mark();
        if !first && !s.character(',') {
            s.restore(before_separator);
            break;
        }
        let after_separator = s.mark();
        match element(s) {
            Some(e) => elements.push(e),
            None => {
                s.restore(after_separator);
                break;
            }
        }
        first = false;
    }
    elements
}

fn number_run(s: &mut State, accept: fn(u16) -> bool, no_value: &'static SimpleCommandExceptionType) -> Option<String> {
    s.input.skip_whitespace();
    let string = s.input.string();
    let start = s.input.cursor;
    let mut pos = start;
    while pos < string.len() && accept(string[pos]) {
        pos += 1;
    }
    if pos == start {
        s.store_delayed(s.mark(), simple(no_value));
        return None;
    }
    if string[start] != b'_' as u16 && string[pos - 1] != b'_' as u16 {
        s.input.cursor = pos;
        Some(utf16(&string[start..pos]))
    } else {
        s.store_delayed(s.mark(), simple(&ERROR_UNDESCORE_NOT_ALLOWED));
        None
    }
}

fn binary_numeral(s: &mut State) -> Option<String> {
    number_run(s, |c| matches!(c, 0x30 | 0x31 | 0x5F), &ERROR_EXPECTED_BINARY_NUMERAL)
}

fn decimal_numeral(s: &mut State) -> Option<String> {
    number_run(s, |c| matches!(c, 0x30..=0x39 | 0x5F), &ERROR_EXPECTED_DECIMAL_NUMERAL)
}

fn hex_numeral(s: &mut State) -> Option<String> {
    number_run(s, |c| matches!(c, 0x30..=0x39 | 0x41..=0x46 | 0x5F | 0x61..=0x66), &ERROR_EXPECTED_HEX_NUMERAL)
}

fn greedy_predicate(s: &mut State, min: usize, max: usize, accept: fn(u16) -> bool, error: Delayed) -> Option<Vec<u16>> {
    let string = s.input.string();
    let start = s.input.cursor;
    let mut pos = start;
    while pos < string.len() && accept(string[pos]) && pos - start < max {
        pos += 1;
    }
    if pos - start < min {
        s.store_delayed(s.mark(), error);
        return None;
    }
    s.input.cursor = pos;
    Some(string[start..pos].to_vec())
}

fn sign(s: &mut State) -> Option<Sign> {
    let mark = s.mark();
    if s.character('+') {
        return Some(Sign::Plus);
    }
    s.restore(mark);
    if s.character('-') {
        return Some(Sign::Minus);
    }
    s.restore(mark);
    None
}

fn integer_type(s: &mut State) -> Option<TypeSuffix> {
    let mark = s.mark();
    for (chars, ty) in [(['b', 'B'], TypeSuffix::Byte), (['s', 'S'], TypeSuffix::Short), (['i', 'I'], TypeSuffix::Int), (['l', 'L'], TypeSuffix::Long)] {
        if s.characters(&chars) {
            return Some(ty);
        }
        s.restore(mark);
    }
    None
}

fn integer_suffix(s: &mut State) -> Option<IntegerSuffix> {
    let mark = s.mark();
    for (chars, signed) in [(['u', 'U'], SignedPrefix::Unsigned), (['s', 'S'], SignedPrefix::Signed)] {
        if s.characters(&chars)
            && let Some(ty) = integer_type(s)
        {
            return Some(IntegerSuffix { signed: Some(signed), ty: Some(ty) });
        }
        s.restore(mark);
    }
    let ty = integer_type(s)?;
    Some(IntegerSuffix { signed: None, ty: Some(ty) })
}

fn integer_literal(s: &mut State) -> Option<IntegerLiteral> {
    let start = s.mark();
    let sign = optional(s, sign);
    let alternative = s.mark();
    let mut cut = false;
    let number = 'alternative: {
        if s.character('0') {
            cut = true;
            if let Some(number) = integer_after_zero(s) {
                break 'alternative Some(number);
            }
        }
        s.restore(alternative);
        if cut {
            break 'alternative None;
        }
        if let Some(digits) = decimal_numeral(s) {
            break 'alternative Some((Base::Decimal, digits));
        }
        s.restore(alternative);
        None
    };
    let Some((base, digits)) = number else {
        s.restore(start);
        return None;
    };
    let suffix = optional(s, integer_suffix).unwrap_or(IntegerSuffix::EMPTY);
    Some(IntegerLiteral { sign: sign.unwrap_or(Sign::Plus), base, digits, suffix })
}

fn integer_after_zero(s: &mut State) -> Option<(Base, String)> {
    let mark = s.mark();
    if s.characters(&['x', 'X']) {
        if let Some(digits) = hex_numeral(s) {
            return Some((Base::Hex, digits));
        }
        s.restore(mark);
        return None;
    }
    s.restore(mark);
    if s.characters(&['b', 'B'])
        && let Some(digits) = binary_numeral(s)
    {
        return Some((Base::Binary, digits));
    }
    s.restore(mark);
    if decimal_numeral(s).is_some() {
        s.store_delayed(s.mark(), simple(&ERROR_LEADING_ZERO_NOT_ALLOWED));
        s.restore(mark);
        return None;
    }
    s.restore(mark);
    Some((Base::Decimal, "0".into()))
}

fn float_type_suffix(s: &mut State) -> Option<TypeSuffix> {
    let mark = s.mark();
    if s.characters(&['f', 'F']) {
        return Some(TypeSuffix::Float);
    }
    s.restore(mark);
    if s.characters(&['d', 'D']) {
        return Some(TypeSuffix::Double);
    }
    s.restore(mark);
    None
}

fn float_exponent_part(s: &mut State) -> Option<(Sign, String)> {
    let start = s.mark();
    if !s.characters(&['e', 'E']) {
        s.restore(start);
        return None;
    }
    let sign = optional(s, sign).unwrap_or(Sign::Plus);
    match decimal_numeral(s) {
        Some(digits) => Some((sign, digits)),
        None => {
            s.restore(start);
            None
        }
    }
}

struct FloatParts {
    whole: Option<String>,
    fraction: Option<String>,
    exponent: Option<(Sign, String)>,
    suffix: Option<TypeSuffix>,
}

fn float_parts(s: &mut State) -> Option<FloatParts> {
    let mark = s.mark();
    if let Some(whole) = decimal_numeral(s)
        && s.character('.')
    {
        let fraction = optional(s, decimal_numeral);
        let exponent = optional(s, float_exponent_part);
        let suffix = optional(s, float_type_suffix);
        return Some(FloatParts { whole: Some(whole), fraction, exponent, suffix });
    }
    s.restore(mark);
    if s.character('.') {
        if let Some(fraction) = decimal_numeral(s) {
            let exponent = optional(s, float_exponent_part);
            let suffix = optional(s, float_type_suffix);
            return Some(FloatParts { whole: None, fraction: Some(fraction), exponent, suffix });
        }
        s.restore(mark);
        return None;
    }
    s.restore(mark);
    if let Some(whole) = decimal_numeral(s)
        && let Some(exponent) = float_exponent_part(s)
    {
        let suffix = optional(s, float_type_suffix);
        return Some(FloatParts { whole: Some(whole), fraction: None, exponent: Some(exponent), suffix });
    }
    s.restore(mark);
    if let Some(whole) = decimal_numeral(s) {
        let exponent = optional(s, float_exponent_part);
        if let Some(suffix) = float_type_suffix(s) {
            return Some(FloatParts { whole: Some(whole), fraction: None, exponent, suffix: Some(suffix) });
        }
    }
    s.restore(mark);
    None
}

fn float_literal(s: &mut State) -> Option<Nbt> {
    let start = s.mark();
    let sign = optional(s, sign).unwrap_or(Sign::Plus);
    let Some(parts) = float_parts(s) else {
        s.restore(start);
        return None;
    };
    create_float(sign, parts, s)
}

fn create_float(sign: Sign, parts: FloatParts, s: &mut State) -> Option<Nbt> {
    let clean = |text: &str| text.chars().filter(|&c| c != '_').collect::<String>();
    let mut result = String::new();
    if sign == Sign::Minus {
        result.push('-');
    }
    if let Some(whole) = &parts.whole {
        result.push_str(&clean(whole));
    }
    if let Some(fraction) = &parts.fraction {
        result.push('.');
        result.push_str(&clean(fraction));
    }
    if let Some((exponent_sign, digits)) = &parts.exponent {
        result.push('e');
        if *exponent_sign == Sign::Minus {
            result.push('-');
        }
        result.push_str(&clean(digits));
    }
    let value = match parts.suffix {
        Some(TypeSuffix::Float) => result.parse::<f32>().ok().filter(|v| v.is_finite()).map(|v| Nbt::Float(if v == 0.0 { 0.0 } else { v })),
        None | Some(TypeSuffix::Double) => result.parse::<f64>().ok().filter(|v| v.is_finite()).map(|v| Nbt::Double(if v == 0.0 { 0.0 } else { v })),
        Some(_) => {
            s.store_delayed(s.mark(), simple(&ERROR_EXPECTED_FLOAT_TYPE));
            return None;
        }
    };
    if value.is_none() {
        s.store_delayed(s.mark(), simple(&ERROR_INFINITY_NOT_ALLOWED));
    }
    value
}

fn is_hex_digit(c: u16) -> bool {
    matches!(c, 0x30..=0x39 | 0x41..=0x46 | 0x61..=0x66)
}

fn string_hex(s: &mut State, size: usize) -> Option<Vec<u16>> {
    greedy_predicate(s, size, size, is_hex_digit, Delayed::Dynamic(&ERROR_EXPECTED_HEX_ESCAPE, size.to_string()))
}

fn string_unicode_name(s: &mut State) -> Option<Vec<u16>> {
    let accept = |c: u16| matches!(c, 0x20 | 0x2D | 0x30..=0x39 | 0x41..=0x5A | 0x61..=0x7A);
    greedy_predicate(s, 1, usize::MAX, accept, simple(&ERROR_INVALID_CHARACTER_NAME))
}

enum Escape {
    Plain(&'static str),
    Hex(Vec<u16>),
    Name,
}

fn string_escape_sequence(s: &mut State) -> Option<Vec<u16>> {
    let mark = s.mark();
    let plain = [('b', "\u{8}"), ('s', " "), ('t', "\t"), ('n', "\n"), ('f', "\u{c}"), ('r', "\r"), ('\\', "\\"), ('\'', "'"), ('"', "\"")];
    let escape = 'alternative: {
        for (c, value) in plain {
            if s.character(c) {
                break 'alternative Some(Escape::Plain(value));
            }
            s.restore(mark);
        }
        for (c, size) in [('x', 2), ('u', 4), ('U', 8)] {
            if s.character(c)
                && let Some(hex) = string_hex(s, size)
            {
                break 'alternative Some(Escape::Hex(hex));
            }
            s.restore(mark);
        }
        if s.character('N') && s.character('{') && string_unicode_name(s).is_some() && s.character('}') {
            break 'alternative Some(Escape::Name);
        }
        s.restore(mark);
        None
    };
    match escape? {
        Escape::Plain(value) => Some(value.encode_utf16().collect()),
        Escape::Hex(hex) => {
            let code_point = hex.iter().fold(0u32, |v, &c| v << 4 | char::from_u32(c as u32).and_then(|c| c.to_digit(16)).unwrap_or(0)) as i32;
            if !(0..=0x10FFFF).contains(&code_point) {
                s.store_delayed(s.mark(), Delayed::Dynamic(&ERROR_INVALID_CODEPOINT, format!("U+{:08X}", code_point as u32)));
                return None;
            }
            Some(match char::from_u32(code_point as u32) {
                Some(c) => c.encode_utf16(&mut [0; 2]).to_vec(),
                None => vec![code_point as u16],
            })
        }
        Escape::Name => {
            s.store_delayed(s.mark(), simple(&ERROR_INVALID_CHARACTER_NAME));
            None
        }
    }
}

fn string_plain_contents(s: &mut State) -> Option<Vec<u16>> {
    greedy_predicate(s, 1, usize::MAX, |c| !matches!(c, 0x22 | 0x27 | 0x5C), simple(&ERROR_INVALID_STRING_CONTENTS))
}

fn quoted_string_chunk(s: &mut State, other: char) -> Option<Vec<u16>> {
    let mark = s.mark();
    if let Some(plain) = string_plain_contents(s) {
        return Some(plain);
    }
    s.restore(mark);
    if s.character('\\')
        && let Some(escaped) = string_escape_sequence(s)
    {
        return Some(escaped);
    }
    s.restore(mark);
    if s.character(other) {
        return Some(vec![other as u16]);
    }
    s.restore(mark);
    None
}

fn quoted_string_contents(s: &mut State, other: char) -> Vec<u16> {
    let mut contents = Vec::new();
    loop {
        let mark = s.mark();
        match quoted_string_chunk(s, other) {
            Some(chunk) => contents.extend(chunk),
            None => {
                s.restore(mark);
                return contents;
            }
        }
    }
}

fn quoted_string_literal(s: &mut State) -> Option<Vec<u16>> {
    let mark = s.mark();
    if s.character('"') {
        let contents = quoted_string_contents(s, '\'');
        if s.character('"') {
            return Some(contents);
        }
        s.restore(mark);
        return None;
    }
    s.restore(mark);
    if s.character('\'') {
        let contents = quoted_string_contents(s, '"');
        if s.character('\'') {
            return Some(contents);
        }
    }
    s.restore(mark);
    None
}

fn unquoted_string(s: &mut State) -> Option<String> {
    s.input.skip_whitespace();
    let cursor = s.mark();
    let value = s.input.read_unquoted_string();
    if value.is_empty() {
        s.store_delayed(cursor, simple(&ERROR_EXPECTED_UNQUOTED_STRING));
        return None;
    }
    Some(value)
}

fn unquoted_string_or_builtin(s: &mut State) -> Option<Nbt> {
    let contents = unquoted_string(s)?;
    let optional = s.mark();
    let mut arguments = None;
    if s.character('(') {
        arguments = Some(repeated_with_trailing_separator(s, literal));
        if !s.character(')') {
            s.restore(optional);
        }
    } else {
        s.restore(optional);
    }
    let first = contents.encode_utf16().next().unwrap();
    if !is_allowed_to_start_unquoted_string(first) {
        s.store_delayed(s.mark(), simple(&ERROR_INVALID_UNQUOTED_START));
        return None;
    }
    if let Some(arguments) = arguments {
        return match (contents.as_str(), arguments.as_slice()) {
            ("bool", [argument]) => builtin_bool(argument, s),
            ("uuid", [argument]) => builtin_uuid(argument, s),
            _ => {
                s.store_delayed(s.mark(), Delayed::Dynamic(&ERROR_NO_SUCH_OPERATION, format!("{contents}/{}", arguments.len())));
                None
            }
        };
    }
    if contents.eq_ignore_ascii_case("true") {
        Some(Nbt::Byte(1))
    } else if contents.eq_ignore_ascii_case("false") {
        Some(Nbt::Byte(0))
    } else {
        Some(Nbt::String(contents))
    }
}

fn builtin_bool(argument: &Nbt, s: &mut State) -> Option<Nbt> {
    match argument.as_f64() {
        Some(v) => Some(Nbt::Byte((v != 0.0) as i8)),
        None => {
            s.store_delayed(s.mark(), simple(&ERROR_EXPECTED_NUMBER_OR_BOOLEAN));
            None
        }
    }
}

fn builtin_uuid(argument: &Nbt, s: &mut State) -> Option<Nbt> {
    match argument.as_str().and_then(crate::commands::arguments::java_uuid_from_string) {
        Some(uuid) => Some(Nbt::IntArray((0..4).map(|i| (uuid >> (96 - 32 * i)) as u32 as i32).collect())),
        None => {
            s.store_delayed(s.mark(), simple(&ERROR_EXPECTED_STRING_UUID));
            None
        }
    }
}

fn map_key(s: &mut State) -> Option<Vec<u16>> {
    let mark = s.mark();
    if let Some(key) = quoted_string_literal(s) {
        return Some(key);
    }
    s.restore(mark);
    let key = unquoted_string(s).map(|k| k.encode_utf16().collect());
    if key.is_none() {
        s.restore(mark);
    }
    key
}

fn map_entry(s: &mut State) -> Option<(String, Nbt)> {
    let start = s.mark();
    let parsed = (|| {
        let key = map_key(s)?;
        if !s.character(':') {
            return None;
        }
        Some((key, literal(s)?))
    })();
    let Some((key, value)) = parsed else {
        s.restore(start);
        return None;
    };
    if key.is_empty() {
        s.store_delayed(s.mark(), simple(&ERROR_EMPTY_KEY));
        return None;
    }
    Some((utf16(&key), value))
}

fn map_literal(s: &mut State) -> Option<Nbt> {
    let start = s.mark();
    if !s.character('{') {
        s.restore(start);
        return None;
    }
    let entries = repeated_with_trailing_separator(s, map_entry);
    if !s.character('}') {
        s.restore(start);
        return None;
    }
    let mut fields: Vec<(String, Nbt)> = Vec::with_capacity(entries.len());
    for (key, value) in entries {
        match fields.iter_mut().find(|(k, _)| *k == key) {
            Some(field) => field.1 = value,
            None => fields.push((key, value)),
        }
    }
    Some(Nbt::Compound(fields))
}

fn array_prefix(s: &mut State) -> Option<ArrayPrefix> {
    let mark = s.mark();
    for (c, prefix) in [('B', ArrayPrefix::Byte), ('L', ArrayPrefix::Long), ('I', ArrayPrefix::Int)] {
        if s.character(c) {
            return Some(prefix);
        }
        s.restore(mark);
    }
    None
}

fn list_literal(s: &mut State) -> Option<Nbt> {
    let start = s.mark();
    if !s.character('[') {
        s.restore(start);
        return None;
    }
    let alternative = s.mark();
    let array = 'array: {
        if let Some(prefix) = array_prefix(s)
            && s.character(';')
        {
            break 'array Some((prefix, repeated_with_trailing_separator(s, integer_literal)));
        }
        s.restore(alternative);
        None
    };
    let entries = if array.is_none() { repeated_with_trailing_separator(s, literal) } else { Vec::new() };
    if !s.character(']') {
        s.restore(start);
        return None;
    }
    match array {
        Some((prefix, entries)) if entries.is_empty() => Some(prefix.empty()),
        Some((prefix, entries)) => prefix.create(&entries, s),
        None if entries.is_empty() => Some(Nbt::List(Vec::new())),
        None => Some(Nbt::List(entries)),
    }
}

pub fn literal(s: &mut State) -> Option<Nbt> {
    let mark = s.mark();
    let mut cut = false;
    if s.lookahead(|s| {
        s.input.skip_whitespace();
        s.input.can_read() && can_start_number(s.input.read())
    }) {
        if let Some(value) = float_literal(s) {
            return Some(value);
        }
        s.restore(mark);
        if let Some(integer) = integer_literal(s) {
            return integer.create(s).map(Number::to_nbt);
        }
    }
    s.restore(mark);
    for (lookahead, branch) in [(&['"', '\''][..], 0), (&['{'][..], 1), (&['['][..], 2)] {
        if s.lookahead(|s| s.characters(lookahead)) {
            cut = true;
            let value = match branch {
                0 => quoted_string_literal(s).map(|v| Nbt::String(utf16(&v))),
                1 => map_literal(s),
                _ => list_literal(s),
            };
            if value.is_some() {
                return value;
            }
        }
        s.restore(mark);
        if cut {
            return None;
        }
    }
    unquoted_string_or_builtin(s)
}

mod java {
    fn magnitude(digits: &str, radix: u32) -> Option<u128> {
        digits.chars().try_fold(0u128, |v, c| v.checked_mul(radix as u128)?.checked_add(c.to_digit(radix)? as u128))
    }

    fn for_input_string(s: &str, radix: u32) -> String {
        if radix == 10 { format!("For input string: \"{s}\"") } else { format!("For input string: \"{s}\" under radix {radix}") }
    }

    fn parse_signed(s: &str, radix: u32, min: i128, max: i128) -> Option<i128> {
        let (negative, digits) = match s.strip_prefix('-') {
            Some(rest) => (true, rest),
            None => (false, s),
        };
        if digits.is_empty() {
            return None;
        }
        let value = magnitude(digits, radix)? as i128;
        let value = if negative { -value } else { value };
        (min..=max).contains(&value).then_some(value)
    }

    pub fn parse_int(s: &str, radix: u32) -> Result<i32, String> {
        parse_signed(s, radix, i32::MIN as i128, i32::MAX as i128).map(|v| v as i32).ok_or_else(|| for_input_string(s, radix))
    }

    pub fn parse_long(s: &str, radix: u32) -> Result<i64, String> {
        parse_signed(s, radix, i64::MIN as i128, i64::MAX as i128).map(|v| v as i64).ok_or_else(|| for_input_string(s, radix))
    }

    pub fn parse_byte(s: &str, radix: u32) -> Result<i8, String> {
        let value = parse_int(s, radix)?;
        i8::try_from(value).map_err(|_| format!("Value out of range. Value:\"{s}\" Radix:{radix}"))
    }

    pub fn parse_short(s: &str, radix: u32) -> Result<i16, String> {
        let value = parse_int(s, radix)?;
        i16::try_from(value).map_err(|_| format!("Value out of range. Value:\"{s}\" Radix:{radix}"))
    }

    pub fn parse_unsigned_byte(s: &str, radix: u32) -> Result<i8, String> {
        let value = parse_int(s, radix)?;
        if value >> 8 == 0 { Ok(value as u8 as i8) } else { Err(format!("out of range: {value}")) }
    }

    pub fn parse_unsigned_short(s: &str, radix: u32) -> Result<i16, String> {
        let value = parse_int(s, radix)?;
        if value >> 16 == 0 { Ok(value as u16 as i16) } else { Err(format!("out of range: {value}")) }
    }

    pub fn parse_unsigned_int(s: &str, radix: u32) -> Result<i32, String> {
        let short = s.chars().count() <= 5 || (radix == 10 && s.chars().count() <= 9);
        if short {
            return parse_int(s, radix);
        }
        match magnitude(s, radix) {
            Some(v) if v <= u32::MAX as u128 => Ok(v as u32 as i32),
            _ => Err(format!("String value {s} exceeds range of unsigned int.")),
        }
    }

    pub fn parse_unsigned_long(s: &str, radix: u32) -> Result<i64, String> {
        if s.is_empty() {
            return Err(for_input_string(s, radix));
        }
        match magnitude(s, radix) {
            Some(v) if v <= u64::MAX as u128 => Ok(v as u64 as i64),
            _ => Err(format!("String value {s} exceeds range of unsigned long.")),
        }
    }
}
