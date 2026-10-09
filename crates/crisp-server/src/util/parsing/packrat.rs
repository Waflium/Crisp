use crate::commands::brigadier::{
    CommandSyntaxException, DISPATCHER_PARSE_EXCEPTION, DynamicCommandExceptionType, LITERAL_INCORRECT, SimpleCommandExceptionType, StringReader,
};

#[derive(Clone)]
pub enum Delayed {
    Simple(&'static SimpleCommandExceptionType),
    Dynamic(&'static DynamicCommandExceptionType, String),
}

impl Delayed {
    fn create(&self, input: &[u16], cursor: usize) -> CommandSyntaxException {
        let mut reader = StringReader::new(input);
        reader.cursor = cursor;
        match self {
            Delayed::Simple(e) => e.create_with_context(&reader),
            Delayed::Dynamic(e, arg) => e.create_with_context(&reader, arg.as_str()),
        }
    }
}

#[derive(Clone)]
pub enum Reason {
    Delayed(Delayed),
    Exception(CommandSyntaxException),
}

#[derive(Default)]
struct ErrorCollector {
    entries: Vec<Reason>,
    last_cursor: Option<usize>,
}

impl ErrorCollector {
    fn store(&mut self, cursor: usize, reason: Reason) {
        if self.last_cursor.is_none_or(|last| cursor > last) {
            self.last_cursor = Some(cursor);
            self.entries.clear();
        }
        if self.last_cursor == Some(cursor) {
            self.entries.push(reason);
        }
    }
}

pub struct ParseState<'r, 'a> {
    pub input: &'r mut StringReader<'a>,
    errors: ErrorCollector,
    silent: u32,
}

impl<'r, 'a> ParseState<'r, 'a> {
    pub fn new(input: &'r mut StringReader<'a>) -> ParseState<'r, 'a> {
        ParseState { input, errors: ErrorCollector::default(), silent: 0 }
    }

    pub fn mark(&self) -> usize {
        self.input.cursor
    }

    pub fn restore(&mut self, mark: usize) {
        self.input.cursor = mark;
    }

    pub fn store(&mut self, cursor: usize, reason: Reason) {
        if self.silent == 0 {
            self.errors.store(cursor, reason);
        }
    }

    pub fn store_delayed(&mut self, cursor: usize, delayed: Delayed) {
        self.store(cursor, Reason::Delayed(delayed));
    }

    pub fn lookahead(&mut self, parse: impl FnOnce(&mut Self) -> bool) -> bool {
        let mark = self.mark();
        self.silent += 1;
        let result = parse(self);
        self.silent -= 1;
        self.restore(mark);
        result
    }

    pub fn characters(&mut self, values: &[char]) -> bool {
        self.input.skip_whitespace();
        let cursor = self.mark();
        if self.input.can_read() {
            let c = self.input.read();
            if values.iter().any(|&v| v as u32 == c as u32) {
                return true;
            }
        }
        let joined = values.iter().map(|c| c.to_string()).collect::<Vec<_>>().join("|");
        self.store_delayed(cursor, Delayed::Dynamic(&LITERAL_INCORRECT, joined));
        false
    }

    pub fn character(&mut self, value: char) -> bool {
        self.characters(&[value])
    }

    pub fn finish<T>(self, result: Option<T>) -> Result<T, CommandSyntaxException> {
        if let Some(result) = result {
            return Ok(result);
        }
        let cursor = self.errors.last_cursor.unwrap_or(0);
        let input = self.input.string();
        match self.errors.entries.into_iter().next() {
            Some(Reason::Delayed(delayed)) => Err(delayed.create(input, cursor)),
            Some(Reason::Exception(e)) => Err(e),
            None => Err(DISPATCHER_PARSE_EXCEPTION.create("Failed to parse")),
        }
    }
}
