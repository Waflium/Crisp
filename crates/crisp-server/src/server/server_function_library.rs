use std::collections::HashMap;
use std::sync::Arc;

use crate::commands::functions::CommandFunction;

#[derive(Default)]
pub struct ServerFunctionLibrary {
    functions: HashMap<String, Arc<CommandFunction>>,
    tags: HashMap<String, Vec<Arc<CommandFunction>>>,
}

impl ServerFunctionLibrary {
    pub fn get(&self, id: &str) -> Option<Arc<CommandFunction>> {
        self.functions.get(id).cloned()
    }

    pub fn get_tag(&self, id: &str) -> &[Arc<CommandFunction>] {
        self.tags.get(id).map_or(&[], Vec::as_slice)
    }
}
