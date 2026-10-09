use crisp_protocol::nbt::Nbt;

use crate::commands::arguments::{ERROR_INVALID_ID, parse_identifier};
use crate::commands::brigadier::{CommandSyntaxException, StringReader};
use crate::nbt;
use crate::util::parsing::packrat::{Delayed, ParseState, Reason};

pub trait Context {
    type Predicate;
    type ComponentType;
    type PredicateType;

    fn for_element_type(&self, reader: &StringReader, id: &str) -> Result<Self::Predicate, CommandSyntaxException>;
    fn for_tag_type(&self, reader: &StringReader, id: &str) -> Result<Self::Predicate, CommandSyntaxException>;
    fn lookup_component_type(&self, reader: &StringReader, id: &str) -> Result<Self::ComponentType, CommandSyntaxException>;
    fn create_component_test(&self, reader: &StringReader, component: &Self::ComponentType, value: &Nbt) -> Result<Self::Predicate, CommandSyntaxException>;
    fn create_component_presence_test(&self, component: &Self::ComponentType) -> Self::Predicate;
    fn lookup_predicate_type(&self, reader: &StringReader, id: &str) -> Result<Self::PredicateType, CommandSyntaxException>;
    fn create_predicate_test(&self, reader: &StringReader, predicate: &Self::PredicateType, value: &Nbt) -> Result<Self::Predicate, CommandSyntaxException>;
    fn negate(&self, value: Self::Predicate) -> Self::Predicate;
    fn any_of(&self, alternatives: Vec<Self::Predicate>) -> Self::Predicate;
}

type State<'r, 'a> = ParseState<'r, 'a>;

pub fn parse<C: Context>(reader: &mut StringReader, context: &C) -> Result<Vec<C::Predicate>, CommandSyntaxException> {
    let mut state = ParseState::new(reader);
    let result = top(&mut state, context);
    state.finish(result)
}

fn top<C: Context>(s: &mut State, c: &C) -> Option<Vec<C::Predicate>> {
    let mark = s.mark();
    if let Some(ty) = type_(s, c)
        && s.character('[')
    {
        let mark_conditions = s.mark();
        let conditions = conditions(s, c);
        if conditions.is_none() {
            s.restore(mark_conditions);
        }
        if s.character(']') {
            return Some(ty.into_iter().chain(conditions.unwrap_or_default()).collect());
        }
        s.restore(mark);
        return None;
    }
    s.restore(mark);
    let ty = type_(s, c)?;
    Some(ty.into_iter().collect())
}

fn type_<C: Context>(s: &mut State, c: &C) -> Option<Option<C::Predicate>> {
    let mark = s.mark();
    if let Some(element) = resource_lookup(s, |reader, id| c.for_element_type(reader, id)) {
        return Some(Some(element));
    }
    s.restore(mark);
    if s.character('#') {
        if let Some(tag) = resource_lookup(s, |reader, id| c.for_tag_type(reader, id)) {
            return Some(Some(tag));
        }
        s.restore(mark);
        return None;
    }
    s.restore(mark);
    if s.character('*') {
        return Some(None);
    }
    s.restore(mark);
    None
}

fn conditions<C: Context>(s: &mut State, c: &C) -> Option<Vec<C::Predicate>> {
    let start = s.mark();
    let Some(alternatives) = alternatives(s, c) else {
        s.restore(start);
        return None;
    };
    let condition = c.any_of(alternatives);
    let mark = s.mark();
    let mut rest = None;
    if s.character(',') {
        rest = conditions(s, c);
    }
    if rest.is_none() {
        s.restore(mark);
    }
    Some(std::iter::once(condition).chain(rest.unwrap_or_default()).collect())
}

fn alternatives<C: Context>(s: &mut State, c: &C) -> Option<Vec<C::Predicate>> {
    let start = s.mark();
    let Some(term) = term(s, c) else {
        s.restore(start);
        return None;
    };
    let mark = s.mark();
    let mut rest = None;
    if s.character('|') {
        rest = alternatives(s, c);
    }
    if rest.is_none() {
        s.restore(mark);
    }
    Some(std::iter::once(term).chain(rest.unwrap_or_default()).collect())
}

fn term<C: Context>(s: &mut State, c: &C) -> Option<C::Predicate> {
    let mark = s.mark();
    if let Some(test) = test(s, c) {
        return Some(test);
    }
    s.restore(mark);
    if s.character('!')
        && let Some(test) = test(s, c)
    {
        return Some(c.negate(test));
    }
    s.restore(mark);
    None
}

enum Test<C: Context> {
    Component(C::ComponentType, Option<Nbt>),
    Predicate(C::PredicateType, Nbt),
}

fn test<C: Context>(s: &mut State, c: &C) -> Option<C::Predicate> {
    let mark = s.mark();
    let parsed = 'alternative: {
        if let Some(component) = resource_lookup(s, |reader, id| c.lookup_component_type(reader, id))
            && s.character('=')
        {
            match tag(s) {
                Some(value) => break 'alternative Some(Test::<C>::Component(component, Some(value))),
                None => {
                    s.restore(mark);
                    break 'alternative None;
                }
            }
        }
        s.restore(mark);
        if let Some(predicate) = resource_lookup(s, |reader, id| c.lookup_predicate_type(reader, id))
            && s.character('~')
        {
            match tag(s) {
                Some(value) => break 'alternative Some(Test::Predicate(predicate, value)),
                None => {
                    s.restore(mark);
                    break 'alternative None;
                }
            }
        }
        s.restore(mark);
        if let Some(component) = resource_lookup(s, |reader, id| c.lookup_component_type(reader, id)) {
            break 'alternative Some(Test::Component(component, None));
        }
        s.restore(mark);
        None
    }?;
    let result = match parsed {
        Test::Predicate(predicate, value) => c.create_predicate_test(s.input, &predicate, &value),
        Test::Component(component, Some(value)) => c.create_component_test(s.input, &component, &value),
        Test::Component(component, None) => Ok(c.create_component_presence_test(&component)),
    };
    match result {
        Ok(predicate) => Some(predicate),
        Err(e) => {
            s.store(s.mark(), Reason::Exception(e));
            None
        }
    }
}

fn identifier(s: &mut State) -> Option<String> {
    let reader = &mut *s.input;
    reader.skip_whitespace();
    let start = reader.cursor;
    while reader.can_read() && crate::commands::arguments::is_allowed_in_identifier(reader.peek()) {
        reader.skip();
    }
    let raw = reader.substring(start, reader.cursor);
    if raw.is_empty() {
        return None;
    }
    let id = parse_identifier(&raw);
    if id.is_none() {
        reader.cursor = start;
    }
    id
}

fn resource_lookup<V>(s: &mut State, validate: impl FnOnce(&StringReader, &str) -> Result<V, CommandSyntaxException>) -> Option<V> {
    s.input.skip_whitespace();
    let mark = s.mark();
    match identifier(s) {
        Some(id) => match validate(s.input, &id) {
            Ok(value) => Some(value),
            Err(e) => {
                s.store(mark, Reason::Exception(e));
                None
            }
        },
        None => {
            s.store_delayed(mark, Delayed::Simple(&ERROR_INVALID_ID));
            None
        }
    }
}

fn tag(s: &mut State) -> Option<Nbt> {
    s.input.skip_whitespace();
    let mark = s.mark();
    match nbt::parse_as_argument(s.input) {
        Ok(value) => Some(value),
        Err(e) => {
            s.store(mark, Reason::Exception(e));
            None
        }
    }
}
