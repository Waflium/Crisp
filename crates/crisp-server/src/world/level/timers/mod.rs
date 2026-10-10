pub mod timer_queue;

use crisp_protocol::nbt::Nbt;

use crate::server::Server;
use crate::server::server_function_manager::ServerFunctionManager;

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum TimerCallback {
    Function(String),
    FunctionTag(String),
}

impl TimerCallback {
    pub fn handle(&self, server: &mut Server) {
        match self {
            TimerCallback::Function(function_id) => {
                if let Some(function) = server.functions.get(function_id) {
                    let sender = ServerFunctionManager::game_loop_sender(server);
                    ServerFunctionManager::execute(server, &function, &sender);
                }
            }
            TimerCallback::FunctionTag(tag_id) => {
                for function in server.functions.get_tag(tag_id).to_vec() {
                    let sender = ServerFunctionManager::game_loop_sender(server);
                    ServerFunctionManager::execute(server, &function, &sender);
                }
            }
        }
    }

    fn parse(nbt: &Nbt) -> Option<TimerCallback> {
        let id = crate::resources::identifier::read(nbt.get("id")?.as_str()?).ok()?;
        match crate::resources::identifier::read(nbt.get("type")?.as_str()?).ok()?.as_str() {
            "minecraft:function" => Some(TimerCallback::Function(id)),
            "minecraft:function_tag" => Some(TimerCallback::FunctionTag(id)),
            _ => None,
        }
    }

    fn to_nbt(&self) -> Nbt {
        let (kind, id) = match self {
            TimerCallback::Function(id) => ("minecraft:function", id),
            TimerCallback::FunctionTag(id) => ("minecraft:function_tag", id),
        };
        Nbt::Compound(vec![("id".into(), Nbt::String(id.clone())), ("type".into(), Nbt::String(kind.into()))])
    }
}
