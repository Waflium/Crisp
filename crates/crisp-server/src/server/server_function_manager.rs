use std::sync::Arc;

use super::Server;
use super::server_function_library::ServerFunctionLibrary;
use crate::commands::execute_command_in_context;
use crate::commands::functions::CommandFunction;
use crate::commands::source::{CommandResultCallback, CommandSourceStack};

const TICK_FUNCTION_TAG: &str = "minecraft:tick";
const LOAD_FUNCTION_TAG: &str = "minecraft:load";

pub struct ServerFunctionManager {
    ticking: Vec<Arc<CommandFunction>>,
    post_reload: bool,
    library: ServerFunctionLibrary,
}

impl ServerFunctionManager {
    pub fn new(library: ServerFunctionLibrary) -> ServerFunctionManager {
        let mut manager = ServerFunctionManager { ticking: Vec::new(), post_reload: false, library };
        manager.post_reload();
        manager
    }

    pub fn tick(server: &mut Server) {
        if !server.level.tick_rate_manager.runs_normally() {
            return;
        }
        if server.functions.post_reload {
            server.functions.post_reload = false;
            let functions = server.functions.library.get_tag(LOAD_FUNCTION_TAG).to_vec();
            ServerFunctionManager::execute_tag_functions(server, &functions);
        }
        let ticking = server.functions.ticking.clone();
        ServerFunctionManager::execute_tag_functions(server, &ticking);
    }

    fn execute_tag_functions(server: &mut Server, functions: &[Arc<CommandFunction>]) {
        for function in functions {
            let sender = ServerFunctionManager::game_loop_sender(server);
            ServerFunctionManager::execute(server, function, &sender);
        }
    }

    pub fn execute(server: &mut Server, function: &CommandFunction, sender: &CommandSourceStack) {
        if let Ok(function) = function.instantiate(None) {
            execute_command_in_context(server, |context| context.queue_initial_function_call(function, sender, CommandResultCallback::EMPTY));
        }
    }

    pub fn replace_library(&mut self, library: ServerFunctionLibrary) {
        self.library = library;
        self.post_reload();
    }

    fn post_reload(&mut self) {
        self.ticking = self.library.get_tag(TICK_FUNCTION_TAG).to_vec();
        self.post_reload = true;
    }

    pub fn game_loop_sender(server: &Server) -> CommandSourceStack {
        CommandSourceStack::server(server).with_permission(2).with_suppressed_output()
    }

    pub fn get(&self, id: &str) -> Option<Arc<CommandFunction>> {
        self.library.get(id)
    }

    pub fn get_tag(&self, id: &str) -> &[Arc<CommandFunction>] {
        self.library.get_tag(id)
    }

    pub fn function_names(&self) -> impl Iterator<Item = &str> {
        self.library.function_names()
    }

    pub fn tag_names(&self) -> impl Iterator<Item = &str> {
        self.library.available_tags()
    }
}
