use crate::commands::brigadier::{
    CommandSyntaxException, DynamicCommandExceptionType, READER_INVALID_DOUBLE, READER_INVALID_FLOAT, READER_INVALID_INT, SimpleCommandExceptionType,
    StringReader,
};

pub static ERROR_EMPTY: SimpleCommandExceptionType = SimpleCommandExceptionType("argument.range.empty");
pub static ERROR_SWAPPED: SimpleCommandExceptionType = SimpleCommandExceptionType("argument.range.swapped");

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Bounds<T> {
    pub min: Option<T>,
    pub max: Option<T>,
}

impl<T: Copy + PartialOrd> Bounds<T> {
    pub fn are_swapped(&self) -> bool {
        matches!((self.min, self.max), (Some(min), Some(max)) if min > max)
    }

    fn map<U>(&self, f: impl Fn(T) -> U) -> Bounds<U> {
        Bounds { min: self.min.map(&f), max: self.max.map(&f) }
    }

    fn from_reader(
        reader: &mut StringReader,
        converter: fn(&str) -> Option<T>,
        parse_exc: &DynamicCommandExceptionType,
    ) -> Result<Bounds<T>, CommandSyntaxException> {
        if !reader.can_read() {
            return Err(ERROR_EMPTY.create_with_context(reader));
        }
        let start = reader.cursor;
        let result = (|| {
            let min = read_number(reader, converter, parse_exc)?;
            let max = if reader.can_read_n(2) && reader.peek() == b'.' as u16 && reader.peek_at(1) == b'.' as u16 {
                reader.skip();
                reader.skip();
                read_number(reader, converter, parse_exc)?
            } else {
                min
            };
            if min.is_none() && max.is_none() {
                return Err(ERROR_EMPTY.create_with_context(reader));
            }
            Ok(Bounds { min, max })
        })();
        result.map_err(|mut e| {
            reader.cursor = start;
            e.cursor = start as i32;
            e
        })
    }
}

fn read_number<T>(reader: &mut StringReader, converter: fn(&str) -> Option<T>, parse_exc: &DynamicCommandExceptionType) -> Result<Option<T>, CommandSyntaxException> {
    let start = reader.cursor;
    while reader.can_read() && is_allowed_input_char(reader) {
        reader.skip();
    }
    let number = reader.substring(start, reader.cursor);
    if number.is_empty() {
        return Ok(None);
    }
    match converter(&number) {
        Some(value) => Ok(Some(value)),
        None => Err(parse_exc.create_with_context(reader, number)),
    }
}

fn is_allowed_input_char(reader: &StringReader) -> bool {
    let c = reader.peek();
    if (b'0' as u16..=b'9' as u16).contains(&c) || c == b'-' as u16 {
        true
    } else if c != b'.' as u16 {
        false
    } else {
        !reader.can_read_n(2) || reader.peek_at(1) != b'.' as u16
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Ints {
    pub bounds: Bounds<i32>,
}

impl Ints {
    pub fn matches(&self, value: i32) -> bool {
        if self.bounds.min.is_some_and(|min| min > value) {
            return false;
        }
        self.bounds.max.is_none_or(|max| max >= value)
    }

    pub fn from_reader(reader: &mut StringReader) -> Result<Ints, CommandSyntaxException> {
        let start = reader.cursor;
        let bounds = Bounds::from_reader(reader, |s| s.parse().ok(), &READER_INVALID_INT)?;
        if bounds.are_swapped() {
            reader.cursor = start;
            return Err(ERROR_SWAPPED.create_with_context(reader));
        }
        Ok(Ints { bounds })
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Doubles {
    pub bounds: Bounds<f64>,
    bounds_sqr: Bounds<f64>,
}

impl Doubles {
    fn new(bounds: Bounds<f64>) -> Doubles {
        Doubles { bounds, bounds_sqr: bounds.map(|v| v * v) }
    }

    pub fn matches_sqr(&self, value_sqr: f64) -> bool {
        if self.bounds_sqr.min.is_some_and(|min| min > value_sqr) {
            return false;
        }
        self.bounds_sqr.max.is_none_or(|max| !(max < value_sqr))
    }

    pub fn from_reader(reader: &mut StringReader) -> Result<Doubles, CommandSyntaxException> {
        let start = reader.cursor;
        let bounds = Bounds::from_reader(reader, |s| s.parse().ok(), &READER_INVALID_DOUBLE)?;
        if bounds.are_swapped() {
            reader.cursor = start;
            return Err(ERROR_SWAPPED.create_with_context(reader));
        }
        Ok(Doubles::new(bounds))
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct FloatDegrees {
    pub bounds: Bounds<f32>,
}

impl FloatDegrees {
    pub fn from_reader(reader: &mut StringReader) -> Result<FloatDegrees, CommandSyntaxException> {
        Ok(FloatDegrees { bounds: Bounds::from_reader(reader, |s| s.parse().ok(), &READER_INVALID_FLOAT)? })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ints(input: &str) -> Result<(Ints, usize), String> {
        let units: Vec<u16> = input.encode_utf16().collect();
        let mut reader = StringReader::new(&units);
        Ints::from_reader(&mut reader).map(|r| (r, reader.cursor)).map_err(|e| e.get_message())
    }

    fn doubles(input: &str) -> Result<Doubles, String> {
        let units: Vec<u16> = input.encode_utf16().collect();
        Doubles::from_reader(&mut StringReader::new(&units)).map_err(|e| e.get_message())
    }

    #[test]
    fn reads_ranges() {
        let (range, cursor) = ints("1..5]").unwrap();
        assert_eq!((range.bounds, cursor), (Bounds { min: Some(1), max: Some(5) }, 4));
        assert_eq!(ints("..-2").unwrap().0.bounds, Bounds { min: None, max: Some(-2) });
        assert_eq!(ints("3").unwrap().0.bounds, Bounds { min: Some(3), max: Some(3) });
        assert_eq!(doubles("1.5..").unwrap().bounds, Bounds { min: Some(1.5), max: None });
        assert!(doubles("..2").unwrap().matches_sqr(4.0) && !doubles("..2").unwrap().matches_sqr(4.01));
    }

    #[test]
    fn errors_point_at_the_range() {
        assert_eq!(ints("").unwrap_err(), "Expected value or range of values at position 0: <--[HERE]");
        assert_eq!(ints("..").unwrap_err(), "Expected value or range of values at position 0: <--[HERE]");
        assert_eq!(ints("1-2").unwrap_err(), "Invalid integer '1-2' at position 0: <--[HERE]");
        assert_eq!(ints("5..1").unwrap_err(), "Min cannot be bigger than max at position 0: <--[HERE]");
        assert_eq!(doubles("1.2.3").unwrap_err(), "Invalid double '1.2.3' at position 0: <--[HERE]");
    }
}
