use super::ArgumentValue;
use super::selector::entity_selector::{EntitySelector, join_names};
use super::selector::entity_selector_parser::{ERROR_MISSING_SELECTOR_TYPE, EntitySelectorParser, UNKNOWN_SELECTOR_TYPE_KEY, allow_selectors};
use crate::commands::brigadier::{CommandContext, CommandSyntaxException, Dynamic2CommandExceptionType, StringReader, tr, utf16};
use crate::commands::source::CommandSourceStack;
use crate::network::chat::player_chat_message::PlayerChatMessage;
use crate::network::chat::{Component, Contents};
use crate::server::Server;

static TOO_LONG: Dynamic2CommandExceptionType = Dynamic2CommandExceptionType(|length, max_length| tr("argument.message.too_long", vec![length, max_length]));

#[derive(Clone, PartialEq, Debug)]
pub struct Message {
    pub text: String,
    parts: Vec<Part>,
}

#[derive(Clone, PartialEq, Debug)]
struct Part {
    start: usize,
    end: usize,
    selector: EntitySelector,
}

impl Message {
    pub fn parse_text(reader: &mut StringReader, allow_selectors: bool) -> Result<Message, CommandSyntaxException> {
        if reader.remaining_length() > 256 {
            return Err(TOO_LONG.create(reader.remaining_length() as i32, 256));
        }
        let text = reader.remaining();
        if !allow_selectors {
            reader.cursor = reader.total_length();
            return Ok(Message { text, parts: Vec::new() });
        }
        let mut parts = Vec::new();
        let offset = reader.cursor;
        while reader.can_read() {
            if reader.peek() != b'@' as u16 {
                reader.skip();
                continue;
            }
            let start = reader.cursor;
            match EntitySelectorParser::new(reader, true).parse() {
                Ok(selector) => parts.push(Part { start: start - offset, end: reader.cursor - offset, selector }),
                Err(e) => {
                    let key = match &e.message.contents {
                        Contents::Translatable { key, .. } => key.as_str(),
                        _ => "",
                    };
                    if key != ERROR_MISSING_SELECTOR_TYPE.0 && key != UNKNOWN_SELECTOR_TYPE_KEY {
                        return Err(e);
                    }
                    reader.cursor = start + 1;
                }
            }
        }
        Ok(Message { text, parts })
    }

    pub fn resolve_component(&self, server: &Server, source: &CommandSourceStack) -> Result<Component, CommandSyntaxException> {
        self.to_component(server, source, allow_selectors(source))
    }

    pub fn to_component(&self, server: &Server, source: &CommandSourceStack, allow_selectors: bool) -> Result<Component, CommandSyntaxException> {
        if self.parts.is_empty() || !allow_selectors {
            return Ok(Component::literal(self.text.as_str()));
        }
        let text: Vec<u16> = self.text.encode_utf16().collect();
        let mut result = Component::literal(utf16(&text[..self.parts[0].start]));
        let mut read_to = self.parts[0].start;
        for part in &self.parts {
            let component = join_names(server, &part.selector.find_entities(server, source)?);
            if read_to < part.start {
                result = result.append(Component::literal(utf16(&text[read_to..part.start])));
            }
            result = result.append(component);
            read_to = part.end;
        }
        if read_to < text.len() {
            result = result.append(Component::literal(utf16(&text[read_to..])));
        }
        Ok(result)
    }
}

impl CommandContext {
    pub fn get_message_argument(&self, name: &str) -> &Message {
        match self.argument(name) {
            ArgumentValue::Message(message) => message,
            other => panic!("{name} is {other:?}"),
        }
    }

    pub fn resolve_chat_message(
        &self,
        server: &mut Server,
        source: &CommandSourceStack,
        name: &str,
        task: impl FnOnce(&mut Server, PlayerChatMessage) + 'static,
    ) -> Result<(), CommandSyntaxException> {
        let message = self.get_message_argument(name);
        let formatted = message.resolve_component(server, source)?;
        match source.signing_context.as_ref().and_then(|c| Some((c.chain, c.arguments.get(name)?))) {
            Some((chain, signed)) => {
                let signed = signed.with_unsigned_content(formatted);
                let decorated = signed.decorated_content();
                server.chain(chain, move |server| task(server, signed.with_unsigned_content(decorated)));
            }
            None => {
                let argument = PlayerChatMessage::system(&message.text).with_unsigned_content(formatted);
                let decorated = argument.decorated_content();
                task(server, argument.with_unsigned_content(decorated));
            }
        }
        Ok(())
    }

    pub fn get_message(&self, server: &Server, source: &CommandSourceStack, name: &str) -> Result<Component, CommandSyntaxException> {
        self.get_message_argument(name).resolve_component(server, source)
    }
}
