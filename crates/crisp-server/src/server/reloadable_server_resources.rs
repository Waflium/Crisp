use super::packs::resources::MultiPackResourceManager;
use super::server_function_library::ServerFunctionLibrary;
use crate::tags::PendingTags;

pub struct ReloadableServerResources {
    postponed_tags: PendingTags,
    pub function_library: ServerFunctionLibrary,
}

impl ReloadableServerResources {
    pub fn load_resources(resource_manager: &MultiPackResourceManager, postponed_tags: PendingTags, function_compilation_permission: i32) -> ReloadableServerResources {
        ReloadableServerResources { postponed_tags, function_library: ServerFunctionLibrary::reload(resource_manager, function_compilation_permission) }
    }

    pub fn update_static_registry_tags(&self) {
        crisp_data::bind_tags(self.postponed_tags.clone());
    }
}
