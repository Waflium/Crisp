use crisp_data::RegistryTags;

use super::packs::resources::MultiPackResourceManager;
use super::reloadable_server_registries::{self, LoadResult, ReloadableRegistries};
use super::server_function_library::{ServerFunctionLibrary, worker_name};
use crate::tags::PendingTags;
use crate::world::item::crafting::recipe_manager::RecipeManager;

pub struct ReloadableServerResources {
    postponed_tags: &'static [RegistryTags],
    registries: Option<ReloadableRegistries>,
    recipe_manager: Option<RecipeManager>,
    pub function_library: ServerFunctionLibrary,
}

impl ReloadableServerResources {
    pub fn load_resources(resource_manager: &MultiPackResourceManager, postponed_tags: PendingTags, function_compilation_permission: i32) -> Result<ReloadableServerResources, String> {
        let postponed_tags = crisp_data::prepare_tags(postponed_tags);
        let loaded = std::thread::scope(|scope| {
            std::thread::Builder::new()
                .name(worker_name())
                .spawn_scoped(scope, || reloadable_server_registries::load(resource_manager, postponed_tags, Some(reloadable_server_registries::get())))
                .expect("worker thread")
                .join()
                .expect("reloadable registries")
        });
        let (registries, recipe_manager) = match loaded {
            Ok(LoadResult::Unchanged) => (None, None),
            Ok(LoadResult::Loaded { registries, recipes }) => (Some(registries), Some(RecipeManager::new(recipes))),
            Err(_) => return Err("net.minecraft.ReportedException: Registry Loading".to_string()),
        };
        let function_library = ServerFunctionLibrary::reload(resource_manager, function_compilation_permission);
        Ok(ReloadableServerResources { postponed_tags, registries, recipe_manager, function_library })
    }

    pub fn update_components_and_static_registry_tags(&mut self) {
        crisp_data::bind_tags(self.postponed_tags);
        if let Some(registries) = self.registries.take() {
            reloadable_server_registries::bind(registries);
        }
    }

    pub fn finalize_recipe_loading(&mut self) {
        if let Some(recipe_manager) = self.recipe_manager.take() {
            RecipeManager::bind(recipe_manager);
        }
    }
}
