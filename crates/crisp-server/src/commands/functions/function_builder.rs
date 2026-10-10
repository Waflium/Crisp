
use std::sync::Arc;

use super::macro_function::{Entry, MacroEntry, MacroFunction};
use super::string_template::StringTemplate;
use super::{CommandFunction, PlainTextFunction};
use crate::commands::execution::tasks::BuildContexts;
use crate::commands::source::CommandSourceStack;

pub struct FunctionBuilder {
    plain_entries: Option<Vec<BuildContexts>>,
    macro_entries: Option<Vec<Entry>>,
    macro_arguments: Vec<String>,
}

impl Default for FunctionBuilder {
    fn default() -> FunctionBuilder {
        FunctionBuilder { plain_entries: Some(Vec::new()), macro_entries: None, macro_arguments: Vec::new() }
    }
}

impl FunctionBuilder {
    pub fn add_command(&mut self, command: BuildContexts) {
        match &mut self.macro_entries {
            Some(entries) => entries.push(Entry::PlainText(command)),
            None => self.plain_entries.as_mut().unwrap().push(command),
        }
    }

    fn get_argument_index(&mut self, id: &str) -> usize {
        match self.macro_arguments.iter().position(|a| a == id) {
            Some(index) => index,
            None => {
                self.macro_arguments.push(id.to_string());
                self.macro_arguments.len() - 1
            }
        }
    }

    pub fn add_macro(&mut self, command: &str, line: usize, compilation_context: &CommandSourceStack) -> Result<(), String> {
        let template = StringTemplate::from_string(command).map_err(|cause| format!("Can't parse function line {line}: '{command}'\nCaused by: {cause}"))?;
        if let Some(plain_entries) = self.plain_entries.take() {
            self.macro_entries = Some(plain_entries.into_iter().map(Entry::PlainText).collect());
        }
        let parameters = template.variables.iter().map(|v| self.get_argument_index(v)).collect();
        let entry = MacroEntry { template, parameters, compilation_context: compilation_context.clone() };
        self.macro_entries.as_mut().unwrap().push(Entry::Macro(entry));
        Ok(())
    }

    pub fn build(self, id: &str) -> CommandFunction {
        match self.macro_entries {
            Some(entries) => CommandFunction::Macro(MacroFunction::new(id.to_string(), entries, self.macro_arguments)),
            None => CommandFunction::PlainText(Arc::new(PlainTextFunction { id: id.to_string(), entries: self.plain_entries.unwrap() })),
        }
    }
}
