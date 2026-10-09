use crisp_data::commands::{CommandNode, CommandNodeKind};

use super::arguments::{self, ArgumentValue};
use super::source::CommandSourceStack;
use crate::network::chat::{Arg, ChatFormatting, Component};

#[derive(Clone)]
pub struct StringReader<'a> {
    string: &'a [u16],
    pub cursor: usize,
}

impl<'a> StringReader<'a> {
    pub fn new(string: &'a [u16]) -> StringReader<'a> {
        StringReader { string, cursor: 0 }
    }

    pub fn string(&self) -> &'a [u16] {
        self.string
    }

    pub fn total_length(&self) -> usize {
        self.string.len()
    }

    pub fn remaining_length(&self) -> usize {
        self.string.len() - self.cursor
    }

    pub fn can_read_n(&self, length: usize) -> bool {
        self.cursor + length <= self.string.len()
    }

    pub fn can_read(&self) -> bool {
        self.can_read_n(1)
    }

    pub fn peek(&self) -> u16 {
        self.string[self.cursor]
    }

    pub fn peek_at(&self, offset: usize) -> u16 {
        self.string[self.cursor + offset]
    }

    pub fn read(&mut self) -> u16 {
        let c = self.string[self.cursor];
        self.cursor += 1;
        c
    }

    pub fn skip(&mut self) {
        self.cursor += 1;
    }

    pub fn read_so_far(&self) -> String {
        utf16(&self.string[..self.cursor])
    }

    pub fn remaining(&self) -> String {
        utf16(&self.string[self.cursor..])
    }

    pub fn substring(&self, start: usize, end: usize) -> String {
        utf16(&self.string[start..end])
    }

    pub fn is_allowed_number(c: u16) -> bool {
        is(c, b'0'..=b'9') || c == b'.' as u16 || c == b'-' as u16
    }

    pub fn is_quoted_string_start(c: u16) -> bool {
        c == b'"' as u16 || c == b'\'' as u16
    }

    pub fn skip_whitespace(&mut self) {
        while self.can_read() && is_whitespace(self.peek()) {
            self.skip();
        }
    }

    fn read_number(&mut self) -> (usize, String) {
        let start = self.cursor;
        while self.can_read() && Self::is_allowed_number(self.peek()) {
            self.skip();
        }
        (start, self.substring(start, self.cursor))
    }

    pub fn read_int(&mut self) -> Result<i32, CommandSyntaxException> {
        let (start, number) = self.read_number();
        if number.is_empty() {
            return Err(READER_EXPECTED_INT.create_with_context(self));
        }
        number.parse().map_err(|_| {
            self.cursor = start;
            READER_INVALID_INT.create_with_context(self, number)
        })
    }

    pub fn read_long(&mut self) -> Result<i64, CommandSyntaxException> {
        let (start, number) = self.read_number();
        if number.is_empty() {
            return Err(READER_EXPECTED_LONG.create_with_context(self));
        }
        number.parse().map_err(|_| {
            self.cursor = start;
            READER_INVALID_LONG.create_with_context(self, number)
        })
    }

    pub fn read_double(&mut self) -> Result<f64, CommandSyntaxException> {
        let (start, number) = self.read_number();
        if number.is_empty() {
            return Err(READER_EXPECTED_DOUBLE.create_with_context(self));
        }
        number.parse().map_err(|_| {
            self.cursor = start;
            READER_INVALID_DOUBLE.create_with_context(self, number)
        })
    }

    pub fn read_float(&mut self) -> Result<f32, CommandSyntaxException> {
        let (start, number) = self.read_number();
        if number.is_empty() {
            return Err(READER_EXPECTED_FLOAT.create_with_context(self));
        }
        number.parse().map_err(|_| {
            self.cursor = start;
            READER_INVALID_FLOAT.create_with_context(self, number)
        })
    }

    pub fn is_allowed_in_unquoted_string(c: u16) -> bool {
        is(c, b'0'..=b'9')
            || is(c, b'A'..=b'Z')
            || is(c, b'a'..=b'z')
            || c == b'_' as u16
            || c == b'-' as u16
            || c == b'.' as u16
            || c == b'+' as u16
    }

    pub fn read_unquoted_string(&mut self) -> String {
        let start = self.cursor;
        while self.can_read() && Self::is_allowed_in_unquoted_string(self.peek()) {
            self.skip();
        }
        self.substring(start, self.cursor)
    }

    pub fn read_quoted_string(&mut self) -> Result<String, CommandSyntaxException> {
        if !self.can_read() {
            return Ok(String::new());
        }
        let next = self.peek();
        if !Self::is_quoted_string_start(next) {
            return Err(READER_EXPECTED_START_OF_QUOTE.create_with_context(self));
        }
        self.skip();
        self.read_string_until(next)
    }

    pub fn read_string_until(&mut self, terminator: u16) -> Result<String, CommandSyntaxException> {
        let mut result = Vec::new();
        let mut escaped = false;
        while self.can_read() {
            let c = self.read();
            if escaped {
                if c != terminator && c != b'\\' as u16 {
                    self.cursor -= 1;
                    return Err(READER_INVALID_ESCAPE.create_with_context(self, utf16(&[c])));
                }
                result.push(c);
                escaped = false;
            } else if c == b'\\' as u16 {
                escaped = true;
            } else if c == terminator {
                return Ok(utf16(&result));
            } else {
                result.push(c);
            }
        }
        Err(READER_EXPECTED_END_OF_QUOTE.create_with_context(self))
    }

    pub fn read_string(&mut self) -> Result<String, CommandSyntaxException> {
        if !self.can_read() {
            return Ok(String::new());
        }
        let next = self.peek();
        if Self::is_quoted_string_start(next) {
            self.skip();
            return self.read_string_until(next);
        }
        Ok(self.read_unquoted_string())
    }

    pub fn read_boolean(&mut self) -> Result<bool, CommandSyntaxException> {
        let start = self.cursor;
        let value = self.read_string()?;
        if value.is_empty() {
            return Err(READER_EXPECTED_BOOL.create_with_context(self));
        }
        match value.as_str() {
            "true" => Ok(true),
            "false" => Ok(false),
            _ => {
                self.cursor = start;
                Err(READER_INVALID_BOOL.create_with_context(self, value))
            }
        }
    }

    pub fn expect(&mut self, c: char) -> Result<(), CommandSyntaxException> {
        if self.can_read() && self.peek() == c as u16 {
            self.skip();
            Ok(())
        } else {
            Err(READER_EXPECTED_SYMBOL.create_with_context(self, c.to_string()))
        }
    }

    pub fn at(&self, c: char) -> bool {
        self.can_read() && self.peek() == c as u16
    }
}

fn is(c: u16, range: std::ops::RangeInclusive<u8>) -> bool {
    c >= *range.start() as u16 && c <= *range.end() as u16
}

pub fn utf16(units: &[u16]) -> String {
    String::from_utf16_lossy(units)
}

pub fn is_whitespace(c: u16) -> bool {
    match c {
        0x09..=0x0D | 0x1C..=0x1F => true,
        0x00A0 | 0x2007 | 0x202F => false,
        _ => char::from_u32(c as u32).is_some_and(|ch| {
            matches!(ch, ' ' | '\u{1680}' | '\u{2000}'..='\u{2006}' | '\u{2008}'..='\u{200A}' | '\u{2028}' | '\u{2029}' | '\u{205F}' | '\u{3000}')
        }),
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct CommandSyntaxException {
    pub message: Component,
    pub input: Option<String>,
    pub cursor: i32,
}

impl CommandSyntaxException {
    pub fn new(message: Component) -> CommandSyntaxException {
        CommandSyntaxException { message, input: None, cursor: -1 }
    }

    pub fn with_context(message: Component, reader: &StringReader) -> CommandSyntaxException {
        CommandSyntaxException { message, input: Some(reader.substring(0, reader.total_length())), cursor: reader.cursor as i32 }
    }

    pub fn get_message(&self) -> String {
        let mut message = self.message.get_string();
        if let Some(context) = self.context() {
            message = format!("{message} at position {}: {context}", self.cursor);
        }
        message
    }

    fn context(&self) -> Option<String> {
        let input: Vec<u16> = self.input.as_ref()?.encode_utf16().collect();
        if self.cursor < 0 {
            return None;
        }
        let cursor = input.len().min(self.cursor as usize);
        let mut out = String::new();
        if cursor > 10 {
            out.push_str("...");
        }
        out.push_str(&utf16(&input[cursor.saturating_sub(10)..cursor]));
        out.push_str("<--[HERE]");
        Some(out)
    }
}

pub struct SimpleCommandExceptionType(pub &'static str);

impl SimpleCommandExceptionType {
    pub fn create(&self) -> CommandSyntaxException {
        CommandSyntaxException::new(Component::translatable(self.0))
    }

    pub fn create_with_context(&self, reader: &StringReader) -> CommandSyntaxException {
        CommandSyntaxException::with_context(Component::translatable(self.0), reader)
    }
}

pub struct DynamicCommandExceptionType(pub fn(Arg) -> Component);

impl DynamicCommandExceptionType {
    pub fn create(&self, arg: impl Into<Arg>) -> CommandSyntaxException {
        CommandSyntaxException::new((self.0)(arg.into()))
    }

    pub fn create_with_context(&self, reader: &StringReader, arg: impl Into<Arg>) -> CommandSyntaxException {
        CommandSyntaxException::with_context((self.0)(arg.into()), reader)
    }
}

pub struct Dynamic2CommandExceptionType(pub fn(Arg, Arg) -> Component);

impl Dynamic2CommandExceptionType {
    pub fn create(&self, a: impl Into<Arg>, b: impl Into<Arg>) -> CommandSyntaxException {
        CommandSyntaxException::new((self.0)(a.into(), b.into()))
    }

    pub fn create_with_context(&self, reader: &StringReader, a: impl Into<Arg>, b: impl Into<Arg>) -> CommandSyntaxException {
        CommandSyntaxException::with_context((self.0)(a.into(), b.into()), reader)
    }
}

pub struct Dynamic3CommandExceptionType(pub fn(Arg, Arg, Arg) -> Component);

impl Dynamic3CommandExceptionType {
    pub fn create(&self, a: impl Into<Arg>, b: impl Into<Arg>, c: impl Into<Arg>) -> CommandSyntaxException {
        CommandSyntaxException::new((self.0)(a.into(), b.into(), c.into()))
    }

    pub fn create_with_context(&self, reader: &StringReader, a: impl Into<Arg>, b: impl Into<Arg>, c: impl Into<Arg>) -> CommandSyntaxException {
        CommandSyntaxException::with_context((self.0)(a.into(), b.into(), c.into()), reader)
    }
}

pub fn tr(key: &str, args: Vec<Arg>) -> Component {
    Component::translatable_with(key, args)
}

pub static DOUBLE_TOO_SMALL: Dynamic2CommandExceptionType = Dynamic2CommandExceptionType(|found, min| tr("argument.double.low", vec![min, found]));
pub static DOUBLE_TOO_BIG: Dynamic2CommandExceptionType = Dynamic2CommandExceptionType(|found, max| tr("argument.double.big", vec![max, found]));
pub static FLOAT_TOO_SMALL: Dynamic2CommandExceptionType = Dynamic2CommandExceptionType(|found, min| tr("argument.float.low", vec![min, found]));
pub static FLOAT_TOO_BIG: Dynamic2CommandExceptionType = Dynamic2CommandExceptionType(|found, max| tr("argument.float.big", vec![max, found]));
pub static INTEGER_TOO_SMALL: Dynamic2CommandExceptionType = Dynamic2CommandExceptionType(|found, min| tr("argument.integer.low", vec![min, found]));
pub static INTEGER_TOO_BIG: Dynamic2CommandExceptionType = Dynamic2CommandExceptionType(|found, max| tr("argument.integer.big", vec![max, found]));
pub static LONG_TOO_SMALL: Dynamic2CommandExceptionType = Dynamic2CommandExceptionType(|found, min| tr("argument.long.low", vec![min, found]));
pub static LONG_TOO_BIG: Dynamic2CommandExceptionType = Dynamic2CommandExceptionType(|found, max| tr("argument.long.big", vec![max, found]));
pub static LITERAL_INCORRECT: DynamicCommandExceptionType = DynamicCommandExceptionType(|expected| tr("argument.literal.incorrect", vec![expected]));
pub static READER_EXPECTED_START_OF_QUOTE: SimpleCommandExceptionType = SimpleCommandExceptionType("parsing.quote.expected.start");
pub static READER_EXPECTED_END_OF_QUOTE: SimpleCommandExceptionType = SimpleCommandExceptionType("parsing.quote.expected.end");
pub static READER_INVALID_ESCAPE: DynamicCommandExceptionType = DynamicCommandExceptionType(|c| tr("parsing.quote.escape", vec![c]));
pub static READER_INVALID_BOOL: DynamicCommandExceptionType = DynamicCommandExceptionType(|v| tr("parsing.bool.invalid", vec![v]));
pub static READER_INVALID_INT: DynamicCommandExceptionType = DynamicCommandExceptionType(|v| tr("parsing.int.invalid", vec![v]));
pub static READER_EXPECTED_INT: SimpleCommandExceptionType = SimpleCommandExceptionType("parsing.int.expected");
pub static READER_INVALID_LONG: DynamicCommandExceptionType = DynamicCommandExceptionType(|v| tr("parsing.long.invalid", vec![v]));
pub static READER_EXPECTED_LONG: SimpleCommandExceptionType = SimpleCommandExceptionType("parsing.long.expected");
pub static READER_INVALID_DOUBLE: DynamicCommandExceptionType = DynamicCommandExceptionType(|v| tr("parsing.double.invalid", vec![v]));
pub static READER_EXPECTED_DOUBLE: SimpleCommandExceptionType = SimpleCommandExceptionType("parsing.double.expected");
pub static READER_INVALID_FLOAT: DynamicCommandExceptionType = DynamicCommandExceptionType(|v| tr("parsing.float.invalid", vec![v]));
pub static READER_EXPECTED_FLOAT: SimpleCommandExceptionType = SimpleCommandExceptionType("parsing.float.expected");
pub static READER_EXPECTED_BOOL: SimpleCommandExceptionType = SimpleCommandExceptionType("parsing.bool.expected");
pub static READER_EXPECTED_SYMBOL: DynamicCommandExceptionType = DynamicCommandExceptionType(|s| tr("parsing.expected", vec![s]));
pub static DISPATCHER_UNKNOWN_COMMAND: SimpleCommandExceptionType = SimpleCommandExceptionType("command.unknown.command");
pub static DISPATCHER_UNKNOWN_ARGUMENT: SimpleCommandExceptionType = SimpleCommandExceptionType("command.unknown.argument");
pub static DISPATCHER_EXPECTED_ARGUMENT_SEPARATOR: SimpleCommandExceptionType = SimpleCommandExceptionType("command.expected.separator");
pub static DISPATCHER_PARSE_EXCEPTION: DynamicCommandExceptionType = DynamicCommandExceptionType(|m| tr("command.exception", vec![m]));

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct StringRange {
    pub start: usize,
    pub end: usize,
}

impl StringRange {
    pub fn at(pos: usize) -> StringRange {
        StringRange { start: pos, end: pos }
    }

    pub fn encompassing(a: StringRange, b: StringRange) -> StringRange {
        StringRange { start: a.start.min(b.start), end: a.end.max(b.end) }
    }

    pub fn is_empty(&self) -> bool {
        self.start == self.end
    }
}

#[derive(Clone, Debug)]
pub struct ParsedArgument {
    pub range: StringRange,
    pub result: ArgumentValue,
}

#[derive(Clone, Copy, Debug)]
pub struct ParsedCommandNode {
    pub node: u16,
    pub range: StringRange,
}

#[derive(Clone)]
pub struct CommandContextBuilder {
    pub arguments: Vec<(&'static str, ParsedArgument)>,
    pub root_node: u16,
    pub nodes: Vec<ParsedCommandNode>,
    pub command: Option<u16>,
    pub child: Option<Box<CommandContextBuilder>>,
    pub range: StringRange,
    pub modifier: Option<u16>,
    pub forks: bool,
}

impl CommandContextBuilder {
    pub fn new(root_node: u16, start: usize) -> CommandContextBuilder {
        CommandContextBuilder {
            arguments: Vec::new(),
            root_node,
            nodes: Vec::new(),
            command: None,
            child: None,
            range: StringRange::at(start),
            modifier: None,
            forks: false,
        }
    }

    pub fn with_argument(&mut self, name: &'static str, argument: ParsedArgument) {
        match self.arguments.iter_mut().find(|(n, _)| *n == name) {
            Some(entry) => entry.1 = argument,
            None => self.arguments.push((name, argument)),
        }
    }

    pub fn with_node(&mut self, node: &CommandNode, range: StringRange) {
        self.nodes.push(ParsedCommandNode { node: node.id, range });
        self.range = StringRange::encompassing(self.range, range);
        self.modifier = node.modifier.then_some(node.id);
        self.forks = node.fork;
    }

    pub fn find_suggestion_context(&self, cursor: usize) -> Option<(u16, usize)> {
        if self.range.start > cursor {
            return None;
        }
        if self.range.end < cursor {
            return match (&self.child, self.nodes.last()) {
                (Some(child), _) => child.find_suggestion_context(cursor),
                (None, Some(last)) => Some((last.node, last.range.end + 1)),
                (None, None) => Some((self.root_node, self.range.start)),
            };
        }
        let mut previous = self.root_node;
        for node in &self.nodes {
            if node.range.start <= cursor && cursor <= node.range.end {
                return Some((previous, node.range.start));
            }
            previous = node.node;
        }
        Some((previous, self.range.start))
    }

    pub fn copy(&self) -> CommandContextBuilder {
        CommandContextBuilder { modifier: None, ..self.clone() }
    }

    pub fn build(&self, input: &str) -> CommandContext {
        CommandContext {
            input: input.to_string(),
            arguments: self.arguments.clone(),
            command: self.command,
            root_node: self.root_node,
            nodes: self.nodes.clone(),
            range: self.range,
            child: self.child.as_ref().map(|c| Box::new(c.build(input))),
            modifier: self.modifier,
            forks: self.forks,
        }
    }
}

#[derive(Clone, Debug)]
pub struct CommandContext {
    pub input: String,
    pub arguments: Vec<(&'static str, ParsedArgument)>,
    pub command: Option<u16>,
    pub root_node: u16,
    pub nodes: Vec<ParsedCommandNode>,
    pub range: StringRange,
    pub child: Option<Box<CommandContext>>,
    pub modifier: Option<u16>,
    pub forks: bool,
}

impl CommandContext {
    pub fn argument(&self, name: &str) -> &ArgumentValue {
        match self.arguments.iter().find(|(n, _)| *n == name) {
            Some((_, a)) => &a.result,
            None => panic!("No such argument '{name}' exists on this command"),
        }
    }

    pub fn has_argument(&self, name: &str) -> bool {
        self.arguments.iter().any(|(n, _)| *n == name)
    }
}

pub struct ParseResults<'a> {
    pub context: CommandContextBuilder,
    pub reader: StringReader<'a>,
    pub exceptions: Vec<(u16, CommandSyntaxException)>,
}

pub fn parse<'a>(command: StringReader<'a>, source: &CommandSourceStack) -> ParseResults<'a> {
    let context = CommandContextBuilder::new(CommandNode::root().id, command.cursor);
    parse_nodes(CommandNode::root(), command, context, source)
}

fn relevant_nodes(node: &'static CommandNode, input: &StringReader) -> Vec<&'static CommandNode> {
    let has_literals = node.children().any(|c| c.kind == CommandNodeKind::Literal);
    let arguments = || node.children().filter(|c| c.kind == CommandNodeKind::Argument).collect();
    if !has_literals {
        return arguments();
    }
    let mut end = input.cursor;
    while end < input.total_length() && input.string()[end] != b' ' as u16 {
        end += 1;
    }
    let text = &input.string()[input.cursor..end];
    match node.children().find(|c| c.kind == CommandNodeKind::Literal && c.name.encode_utf16().eq(text.iter().copied())) {
        Some(literal) => vec![literal],
        None => arguments(),
    }
}

fn parse_nodes<'a>(
    node: &'static CommandNode,
    original_reader: StringReader<'a>,
    context_so_far: CommandContextBuilder,
    source: &CommandSourceStack,
) -> ParseResults<'a> {
    let mut errors: Vec<(u16, CommandSyntaxException)> = Vec::new();
    let mut potentials: Vec<ParseResults<'a>> = Vec::new();
    let cursor = original_reader.cursor;
    for child in relevant_nodes(node, &original_reader) {
        if !source.has_permission(child.permission as i32) {
            continue;
        }
        let mut context = context_so_far.copy();
        let mut reader = original_reader.clone();
        let parsed = parse_node(child, &mut reader, &mut context, source).and_then(|()| {
            if reader.can_read() && reader.peek() != b' ' as u16 {
                Err(DISPATCHER_EXPECTED_ARGUMENT_SEPARATOR.create_with_context(&reader))
            } else {
                Ok(())
            }
        });
        if let Err(e) = parsed {
            errors.push((child.id, e));
            continue;
        }
        context.command = child.executable.then_some(child.id);
        if reader.can_read_n(if child.redirect.is_none() { 2 } else { 1 }) {
            reader.skip();
            if let Some(redirect) = child.redirect {
                let target = CommandNode::get(redirect);
                let child_context = CommandContextBuilder::new(target.id, reader.cursor);
                let parse = parse_nodes(target, reader, child_context, source);
                context.child = Some(Box::new(parse.context));
                return ParseResults { context, reader: parse.reader, exceptions: parse.exceptions };
            }
            potentials.push(parse_nodes(child, reader, context, source));
        } else {
            potentials.push(ParseResults { context, reader, exceptions: Vec::new() });
        }
    }
    if potentials.is_empty() {
        let mut reader = original_reader;
        reader.cursor = cursor;
        return ParseResults { context: context_so_far, reader, exceptions: errors };
    }
    if potentials.len() > 1 {
        potentials.sort_by_key(|p| (p.reader.can_read(), !p.exceptions.is_empty()));
    }
    potentials.swap_remove(0)
}

fn parse_node(node: &'static CommandNode, reader: &mut StringReader, context: &mut CommandContextBuilder, source: &CommandSourceStack) -> Result<(), CommandSyntaxException> {
    let start = reader.cursor;
    match node.kind {
        CommandNodeKind::Literal => {
            let literal: Vec<u16> = node.name.encode_utf16().collect();
            if reader.can_read_n(literal.len()) {
                let end = start + literal.len();
                if reader.string()[start..end] == literal[..] {
                    reader.cursor = end;
                    if !reader.can_read() || reader.peek() == b' ' as u16 {
                        context.with_node(node, StringRange { start, end });
                        return Ok(());
                    }
                    reader.cursor = start;
                }
            }
            Err(LITERAL_INCORRECT.create_with_context(reader, node.name))
        }
        CommandNodeKind::Argument => {
            let result = arguments::parse(node, reader, source)?;
            let range = StringRange { start, end: reader.cursor };
            context.with_argument(node.name, ParsedArgument { range, result });
            context.with_node(node, range);
            Ok(())
        }
        CommandNodeKind::Root => unreachable!("the root is never a child"),
    }
}

pub struct ContextChain {
    pub modifiers: Vec<CommandContext>,
    pub executable: CommandContext,
}

impl ContextChain {
    pub fn try_flatten(root: CommandContext) -> Option<ContextChain> {
        let mut modifiers = Vec::new();
        let mut current = root;
        loop {
            match current.child.take() {
                None => return current.command.is_some().then_some(ContextChain { modifiers, executable: current }),
                Some(child) => {
                    modifiers.push(current);
                    current = *child;
                }
            }
        }
    }
}

pub fn error_context(e: &CommandSyntaxException, command: &str) -> Option<Component> {
    use crate::network::chat::ClickEvent;
    let input: Vec<u16> = e.input.as_ref()?.encode_utf16().collect();
    if e.cursor < 0 {
        return None;
    }
    let cursor = input.len().min(e.cursor as usize);
    let mut context = Component::empty().with_color(ChatFormatting::Gray).with_click(ClickEvent::SuggestCommand(format!("/{command}")));
    if cursor > 10 {
        context = context.append(Component::literal("..."));
    }
    context = context.append(Component::literal(utf16(&input[cursor.saturating_sub(10)..cursor])));
    if cursor < input.len() {
        context = context.append(Component::literal(utf16(&input[cursor..])).with_color(ChatFormatting::Red).underlined());
    }
    Some(context.append(Component::translatable("command.context.here").with_color(ChatFormatting::Red).italic()))
}
