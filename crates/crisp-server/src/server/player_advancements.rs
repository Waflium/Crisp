use std::collections::HashSet;
use std::path::PathBuf;

use crisp_core::random::java_string_hash;
use crisp_data::game_rule;
use crisp_data::packets::play::clientbound;
use indexmap::IndexMap;
use serde_json::{Map, Value};

use super::advancements::advancement_visibility_evaluator;
use super::level::ServerLevel;
use super::server_advancement_manager::ServerAdvancementManager;
use crate::advancements::advancement_holder::AdvancementHolder;
use crate::advancements::advancement_node::AdvancementNode;
use crate::advancements::advancement_progress::AdvancementProgress;
use crate::advancements::criterion_trigger_instance::CriterionTriggerInstance;
use crate::advancements::triggers::criterion_trigger::CriterionTrigger;
use crate::network::connection::Outbound;
use crate::util::java_collections::{JavaHashCode, JavaObjectHashSet, identifier_hash};
use crate::world::entity::EntityId;

#[derive(Clone, Copy)]
pub struct TriggerInstanceKey {
    pub advancement: &'static AdvancementHolder,
    pub criterion: &'static str,
}

#[derive(Clone, Copy)]
pub struct Listener {
    pub key: TriggerInstanceKey,
    pub instance: &'static CriterionTriggerInstance,
}

impl PartialEq for Listener {
    fn eq(&self, other: &Listener) -> bool {
        self.key.advancement == other.key.advancement && self.key.criterion == other.key.criterion
    }
}

impl JavaHashCode for Listener {
    fn java_hash_code(&self) -> i32 {
        identifier_hash(&self.key.advancement.id).wrapping_mul(31).wrapping_add(java_string_hash(self.key.criterion))
    }
}

pub struct PlayerAdvancements {
    player_save_path: Option<PathBuf>,
    manager: &'static ServerAdvancementManager,
    progress: IndexMap<&'static str, (&'static AdvancementHolder, AdvancementProgress)>,
    visible: HashSet<&'static str>,
    progress_changed: JavaObjectHashSet<&'static AdvancementHolder>,
    roots_to_update: JavaObjectHashSet<&'static AdvancementNode>,
    last_selected_tab: Option<&'static AdvancementHolder>,
    is_first_packet: bool,
    active_triggers: Vec<Option<JavaObjectHashSet<Listener>>>,
}

impl Default for PlayerAdvancements {
    fn default() -> PlayerAdvancements {
        PlayerAdvancements {
            player_save_path: None,
            manager: ServerAdvancementManager::get_bound(),
            progress: IndexMap::new(),
            visible: HashSet::new(),
            progress_changed: JavaObjectHashSet::default(),
            roots_to_update: JavaObjectHashSet::default(),
            last_selected_tab: None,
            is_first_packet: true,
            active_triggers: (0..CriterionTrigger::COUNT).map(|_| None).collect(),
        }
    }
}

impl PlayerAdvancements {
    pub fn new(manager: &'static ServerAdvancementManager, player_save_path: Option<PathBuf>) -> PlayerAdvancements {
        let mut advancements = PlayerAdvancements { player_save_path, manager, ..PlayerAdvancements::default() };
        advancements.load(manager);
        advancements
    }

    pub fn clear_triggers(&mut self) {
        self.active_triggers.iter_mut().for_each(|listeners| *listeners = None);
    }

    pub fn reload(&mut self, manager: &'static ServerAdvancementManager) {
        self.clear_triggers();
        self.progress.clear();
        self.visible.clear();
        self.roots_to_update.clear();
        self.progress_changed.clear();
        self.is_first_packet = true;
        self.last_selected_tab = None;
        self.manager = manager;
        self.load(manager);
    }

    fn register_all_listeners(&mut self, manager: &'static ServerAdvancementManager) {
        for advancement in manager.get_all_advancements() {
            self.register_listeners(advancement);
        }
    }

    fn load(&mut self, manager: &'static ServerAdvancementManager) {
        if let Some(path) = self.player_save_path.clone()
            && path.is_file()
        {
            match std::fs::read_to_string(&path) {
                Ok(text) => match parse_data(&text) {
                    Ok(data) => self.apply_from(manager, data),
                    Err(error) => error!("Couldn't parse player advancements in {}\ncom.google.gson.JsonParseException: {error}", path.display()),
                },
                Err(error) => error!("Couldn't access player advancements in {}\n{error}", path.display()),
            }
        }
        self.register_all_listeners(manager);
    }

    pub fn save(&self) {
        let Some(path) = &self.player_save_path else { return };
        let mut map = Map::new();
        for (holder, progress) in self.progress.values() {
            if progress.has_progress() {
                map.insert(holder.id.clone(), progress.encode());
            }
        }
        map.insert("DataVersion".into(), Value::from(crisp_data::WORLD_VERSION));
        let result = path
            .parent()
            .map_or(Ok(()), std::fs::create_dir_all)
            .and_then(|_| std::fs::write(path, serde_json::to_string_pretty(&Value::Object(map)).expect("advancement json")));
        if let Err(error) = result {
            error!("Couldn't save player advancements to {}\n{error}", path.display());
        }
    }

    fn apply_from(&mut self, manager: &'static ServerAdvancementManager, mut data: Vec<(String, AdvancementProgress)>) {
        data.sort_by(|(_, a), (_, b)| a.compare_to(b));
        for (id, progress) in data {
            match manager.get(&id) {
                None => warn!(
                    "Ignored advancement '{id}' in progress file {} - it doesn't exist anymore?",
                    self.player_save_path.as_ref().map(|path| path.display().to_string()).unwrap_or_default()
                ),
                Some(advancement) => {
                    self.start_progress(advancement, progress);
                    self.progress_changed.insert(advancement);
                    self.mark_for_visibility_update(advancement);
                }
            }
        }
    }

    pub fn award(level: &mut ServerLevel, player: EntityId, holder: &'static AdvancementHolder, criterion: &str) -> bool {
        let mut result = false;
        let advancements = &mut level.player_mut(player).advancements;
        let progress = advancements.get_or_start_progress(holder);
        let was_done = progress.is_done();
        if progress.grant_progress(criterion) {
            let is_done = progress.is_done();
            advancements.unregister_listeners(holder);
            advancements.progress_changed.insert(holder);
            result = true;
            if !was_done && is_done {
                holder.value.rewards.grant(level, player);
                if let Some(display) = &holder.value.display
                    && display.announce_to_chat
                    && level.game_rules.get_bool(game_rule::SHOW_ADVANCEMENT_MESSAGES)
                {
                    let message = display.ty.create_announcement(level, holder, player);
                    level.broadcast_system_message(&message);
                }
            }
        }
        let advancements = &mut level.player_mut(player).advancements;
        if !was_done && advancements.get_or_start_progress(holder).is_done() {
            advancements.mark_for_visibility_update(holder);
        }
        result
    }

    pub fn revoke(&mut self, advancement: &'static AdvancementHolder, criterion: &str) -> bool {
        let mut result = false;
        let progress = self.get_or_start_progress(advancement);
        let was_done = progress.is_done();
        if progress.revoke_progress(criterion) {
            self.register_listeners(advancement);
            self.progress_changed.insert(advancement);
            result = true;
        }
        if was_done && !self.get_or_start_progress(advancement).is_done() {
            self.mark_for_visibility_update(advancement);
        }
        result
    }

    fn mark_for_visibility_update(&mut self, advancement: &'static AdvancementHolder) {
        let tree = self.manager.tree();
        if let Some(node) = tree.get(&advancement.id) {
            self.roots_to_update.insert(tree.root(node));
        }
    }

    fn register_listeners(&mut self, holder: &'static AdvancementHolder) {
        let advancement_progress = self.get_or_start_progress(holder);
        if advancement_progress.is_done() {
            return;
        }
        let pending: Vec<&'static str> = holder
            .value
            .criteria
            .iter()
            .filter(|(name, _)| advancement_progress.get_criterion(name).is_some_and(|progress| !progress.is_done()))
            .map(|(name, _)| name.as_str())
            .collect();
        for name in pending {
            let criterion = holder.value.criterion(name).expect("criterion of its own advancement");
            self.add_listener(criterion.trigger, Listener { key: TriggerInstanceKey { advancement: holder, criterion: name }, instance: &criterion.trigger_instance });
        }
    }

    fn unregister_listeners(&mut self, holder: &'static AdvancementHolder) {
        let advancement_progress = self.get_or_start_progress(holder);
        let done = advancement_progress.is_done();
        let finished: Vec<&'static str> = holder
            .value
            .criteria
            .iter()
            .filter(|(name, _)| advancement_progress.get_criterion(name).is_some_and(|progress| progress.is_done() || done))
            .map(|(name, _)| name.as_str())
            .collect();
        for name in finished {
            let criterion = holder.value.criterion(name).expect("criterion of its own advancement");
            self.remove_listener(criterion.trigger, Listener { key: TriggerInstanceKey { advancement: holder, criterion: name }, instance: &criterion.trigger_instance });
        }
    }

    pub fn flush_dirty(&mut self, connection: &Outbound, show_advancements: bool) {
        if self.is_first_packet || !self.roots_to_update.is_empty() || !self.progress_changed.is_empty() {
            let mut progress: JavaObjectHashSet<&'static AdvancementHolder> = JavaObjectHashSet::default();
            let mut added: JavaObjectHashSet<&'static AdvancementNode> = JavaObjectHashSet::default();
            let mut removed: JavaObjectHashSet<&'static AdvancementHolder> = JavaObjectHashSet::default();
            let roots: Vec<&'static AdvancementNode> = self.roots_to_update.iter().collect();
            for root in roots {
                self.update_tree_visibility(root, &mut added, &mut removed);
            }
            self.roots_to_update.clear();
            for holder in self.progress_changed.iter() {
                if self.visible.contains(holder.id.as_str()) {
                    progress.insert(holder);
                }
            }
            self.progress_changed.clear();
            if !progress.is_empty() || !added.is_empty() || !removed.is_empty() {
                let is_first_packet = self.is_first_packet;
                connection.send(clientbound::UPDATE_ADVANCEMENTS, |w| {
                    w.bool(is_first_packet);
                    w.varint(added.len() as i32);
                    for node in added.iter() {
                        node.holder.write(w);
                        w.f32(node.x);
                        w.f32(node.y);
                    }
                    w.varint(removed.len() as i32);
                    for holder in removed.iter() {
                        w.string(&holder.id);
                    }
                    w.varint(progress.len() as i32);
                    for holder in progress.iter() {
                        w.string(&holder.id);
                        self.progress[holder.id.as_str()].1.write(w);
                    }
                    w.bool(show_advancements);
                });
            }
        }
        self.is_first_packet = false;
    }

    pub fn set_selected_tab(&mut self, connection: &Outbound, holder: Option<&'static AdvancementHolder>) {
        let old = self.last_selected_tab;
        self.last_selected_tab = holder.filter(|holder| holder.value.is_root() && holder.value.display.is_some());
        let changed = match (old, self.last_selected_tab) {
            (Some(old), Some(new)) => !std::ptr::eq(old, new),
            (None, None) => false,
            _ => true,
        };
        if changed {
            let tab = self.last_selected_tab.map(|holder| holder.id.as_str());
            connection.send(clientbound::SELECT_ADVANCEMENTS_TAB, |w| {
                w.bool(tab.is_some());
                if let Some(tab) = tab {
                    w.string(tab);
                }
            });
        }
    }

    pub fn get_or_start_progress(&mut self, advancement: &'static AdvancementHolder) -> &mut AdvancementProgress {
        if !self.progress.contains_key(advancement.id.as_str()) {
            self.start_progress(advancement, AdvancementProgress::default());
        }
        &mut self.progress.get_mut(advancement.id.as_str()).expect("started progress").1
    }

    fn start_progress(&mut self, holder: &'static AdvancementHolder, mut progress: AdvancementProgress) {
        progress.update(&holder.value.requirements);
        self.progress.insert(holder.id.as_str(), (holder, progress));
    }

    fn update_tree_visibility(&mut self, root: &'static AdvancementNode, added: &mut JavaObjectHashSet<&'static AdvancementNode>, removed: &mut JavaObjectHashSet<&'static AdvancementHolder>) {
        let tree = self.manager.tree();
        let mut results: Vec<(&'static AdvancementNode, bool)> = Vec::new();
        advancement_visibility_evaluator::evaluate_visibility(
            tree,
            root,
            |node| self.progress.get(node.holder.id.as_str()).is_some_and(|(_, progress)| progress.is_done()),
            |node, should_be_visible| results.push((node, should_be_visible)),
        );
        for (node, should_be_visible) in results {
            let advancement = node.holder;
            if should_be_visible {
                if self.visible.insert(advancement.id.as_str()) {
                    added.insert(node);
                    if self.progress.contains_key(advancement.id.as_str()) {
                        self.progress_changed.insert(advancement);
                    }
                }
            } else if self.visible.remove(advancement.id.as_str()) {
                removed.insert(advancement);
            }
        }
    }

    pub fn get_trigger_map_for_type(&self, trigger: CriterionTrigger) -> Option<&JavaObjectHashSet<Listener>> {
        self.active_triggers[trigger.index()].as_ref()
    }

    fn add_listener(&mut self, trigger: CriterionTrigger, listener: Listener) {
        self.active_triggers[trigger.index()].get_or_insert_with(JavaObjectHashSet::default).insert(listener);
    }

    fn remove_listener(&mut self, trigger: CriterionTrigger, listener: Listener) {
        let slot = &mut self.active_triggers[trigger.index()];
        if let Some(listeners) = slot {
            listeners.remove(&listener);
            if listeners.is_empty() {
                *slot = None;
            }
        }
    }
}

fn parse_data(text: &str) -> Result<Vec<(String, AdvancementProgress)>, String> {
    let json: Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
    let Value::Object(mut fields) = json else { return Err(format!("Not a JSON object: {json}")) };
    fields.shift_remove("DataVersion");
    let mut data = Vec::with_capacity(fields.len());
    for (id, value) in fields {
        let id = crate::resources::identifier::read(&id)?;
        data.push((id, AdvancementProgress::decode(&value)?));
    }
    Ok(data)
}
