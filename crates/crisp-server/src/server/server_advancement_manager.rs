use std::sync::{LazyLock, RwLock};

use indexmap::IndexMap;

use crate::advancements::advancement_holder::AdvancementHolder;
use crate::advancements::advancement_tree::AdvancementTree;

pub struct ServerAdvancementManager {
    advancements: IndexMap<&'static str, &'static AdvancementHolder>,
    tree: AdvancementTree,
}

static EMPTY: LazyLock<ServerAdvancementManager> = LazyLock::new(|| ServerAdvancementManager { advancements: IndexMap::new(), tree: AdvancementTree::default() });
static BOUND: RwLock<Option<&'static ServerAdvancementManager>> = RwLock::new(None);

impl ServerAdvancementManager {
    pub fn new(advancements: &[&'static AdvancementHolder]) -> ServerAdvancementManager {
        let advancements: IndexMap<&'static str, &'static AdvancementHolder> = advancements.iter().map(|&holder| (holder.id.as_str(), holder)).collect();
        let mut tree = AdvancementTree::default();
        let holders: Vec<&'static AdvancementHolder> = advancements.values().copied().collect();
        tree.add_all(&holders);
        tree.reposition_nodes();
        ServerAdvancementManager { advancements, tree }
    }

    pub fn get_bound() -> &'static ServerAdvancementManager {
        match *BOUND.read().expect("advancements") {
            Some(manager) => manager,
            None => &EMPTY,
        }
    }

    pub fn bind(manager: ServerAdvancementManager) -> &'static ServerAdvancementManager {
        let manager: &'static ServerAdvancementManager = Box::leak(Box::new(manager));
        *BOUND.write().expect("advancements") = Some(manager);
        manager
    }

    pub fn get(&self, id: &str) -> Option<&'static AdvancementHolder> {
        self.advancements.get(id).copied()
    }

    pub fn tree(&self) -> &AdvancementTree {
        &self.tree
    }

    pub fn get_all_advancements(&self) -> impl Iterator<Item = &'static AdvancementHolder> + '_ {
        self.advancements.values().copied()
    }

    pub fn len(&self) -> usize {
        self.advancements.len()
    }

    pub fn is_empty(&self) -> bool {
        self.advancements.is_empty()
    }
}
