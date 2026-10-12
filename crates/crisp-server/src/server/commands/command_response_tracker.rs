use crate::commands::brigadier::CommandSyntaxException;
use crate::commands::source::CommandSourceStack;
use crate::network::chat::Component;
use crate::server::Server;

pub struct CommandResponseTracker<E> {
    total_value: i32,
    only_element: Option<E>,
    element_count: i32,
    only_non_zero_element: Option<E>,
    non_zero_element_count: i32,
}

impl<E: Clone> CommandResponseTracker<E> {
    pub fn new() -> CommandResponseTracker<E> {
        CommandResponseTracker { total_value: 0, only_element: None, element_count: 0, only_non_zero_element: None, non_zero_element_count: 0 }
    }

    pub fn track(&mut self, element: E) {
        self.track_value(element, 1);
    }

    pub fn track_value(&mut self, element: E, value: i32) {
        self.total_value += value;
        self.element_count += 1;
        self.only_element = if self.element_count == 1 { Some(element.clone()) } else { None };
        if value != 0 {
            self.non_zero_element_count += 1;
            self.only_non_zero_element = if self.non_zero_element_count == 1 { Some(element) } else { None };
        }
    }

    pub fn total_value(&self) -> i32 {
        self.total_value
    }

    pub fn element_count(&self, element_type: ElementType) -> i32 {
        match element_type {
            ElementType::Any => self.element_count,
            ElementType::NonZero => self.non_zero_element_count,
        }
    }

    pub fn dispatch_any<R>(&self, on_single: impl FnOnce(E, i32) -> R, on_multiple: impl FnOnce(i32, i32) -> R) -> R {
        match &self.only_element {
            Some(element) => on_single(element.clone(), self.total_value),
            None => on_multiple(self.element_count, self.total_value),
        }
    }

    pub fn send_feedback(
        self,
        server: &Server,
        source: &CommandSourceStack,
        broadcast: bool,
        on_single: impl FnOnce(E, i32) -> Component,
        on_multiple: impl FnOnce(i32, i32) -> Component,
    ) -> Result<i32, CommandSyntaxException> {
        self.send_feedback_of(server, source, broadcast, ElementType::NonZero, None, on_single, on_multiple)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn send_feedback_of(
        self,
        server: &Server,
        source: &CommandSourceStack,
        broadcast: bool,
        element_type: ElementType,
        on_zero: Option<fn() -> CommandSyntaxException>,
        on_single: impl FnOnce(E, i32) -> Component,
        on_multiple: impl FnOnce(i32, i32) -> Component,
    ) -> Result<i32, CommandSyntaxException> {
        let total = self.total_value;
        let (first, count) = match element_type {
            ElementType::Any => (self.only_element, self.element_count),
            ElementType::NonZero => (self.only_non_zero_element, self.non_zero_element_count),
        };
        if let Some(on_zero) = on_zero
            && count == 0
        {
            return Err(on_zero());
        }
        source.send_success(
            server,
            || match first {
                Some(element) => on_single(element, total),
                None => on_multiple(count, total),
            },
            broadcast,
        );
        Ok(total)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ElementType {
    Any,
    NonZero,
}
