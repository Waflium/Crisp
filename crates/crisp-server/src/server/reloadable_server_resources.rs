use super::packs::resources::MultiPackResourceManager;
use super::server_function_library::ServerFunctionLibrary;

pub struct ReloadableServerResources {
    pub function_library: ServerFunctionLibrary,
}

impl ReloadableServerResources {
    pub fn load_resources(resource_manager: &MultiPackResourceManager, function_compilation_permission: i32) -> ReloadableServerResources {
        ReloadableServerResources { function_library: ServerFunctionLibrary::reload(resource_manager, function_compilation_permission) }
    }
}
