use super::command_response_tracker::{CommandResponseTracker, ElementType};
use crate::advancements::advancement::Advancement;
use crate::advancements::advancement_holder::AdvancementHolder;
use crate::advancements::advancement_node::AdvancementNode;
use crate::advancements::advancement_tree::AdvancementTree;
use crate::commands::brigadier::{CommandContext, CommandSyntaxException, Dynamic2CommandExceptionType, Dynamic3CommandExceptionType, DynamicCommandExceptionType, tr};
use crate::commands::shared_suggestion_provider;
use crate::commands::source::CommandSourceStack;
use crate::commands::suggestion::{Suggestions, SuggestionsBuilder};
use crate::network::chat::{Arg, Component};
use crate::server::Server;
use crate::server::player_advancements::PlayerAdvancements;
use crate::server::server_advancement_manager::ServerAdvancementManager;
use crate::world::entity::EntityId;

type Result<T> = std::result::Result<T, CommandSyntaxException>;

static ERROR_INVALID_ADVANCEMENT: DynamicCommandExceptionType = DynamicCommandExceptionType(|value| tr("advancement.advancementNotFound", vec![value]));
static ERROR_CRITERION_NOT_FOUND: Dynamic2CommandExceptionType =
    Dynamic2CommandExceptionType(|name, criterion| tr("commands.advancement.criterionNotFound", vec![name, criterion]));

#[derive(Clone, Copy, PartialEq, Eq)]
enum Action {
    Grant,
    Revoke,
}

#[derive(Clone, Copy)]
enum Mode {
    Only,
    Through,
    From,
    Until,
}

impl Mode {
    fn parents(self) -> bool {
        matches!(self, Mode::Through | Mode::Until)
    }

    fn children(self) -> bool {
        matches!(self, Mode::Through | Mode::From)
    }
}

static GRANT_SINGLE_ONE: Dynamic2CommandExceptionType = Dynamic2CommandExceptionType(|a, b| tr("commands.advancement.grant.one.to.one.failure", vec![a, b]));
static GRANT_SINGLE_MANY: Dynamic2CommandExceptionType = Dynamic2CommandExceptionType(|a, b| tr("commands.advancement.grant.one.to.many.failure", vec![a, b]));
static GRANT_MANY_ONE: Dynamic2CommandExceptionType = Dynamic2CommandExceptionType(|a, b| tr("commands.advancement.grant.many.to.one.failure", vec![a, b]));
static GRANT_MANY_MANY: Dynamic2CommandExceptionType = Dynamic2CommandExceptionType(|a, b| tr("commands.advancement.grant.many.to.many.failure", vec![a, b]));
static GRANT_CRITERION_ONE: Dynamic3CommandExceptionType =
    Dynamic3CommandExceptionType(|a, b, c| tr("commands.advancement.grant.criterion.to.one.failure", vec![a, b, c]));
static GRANT_CRITERION_MANY: Dynamic3CommandExceptionType =
    Dynamic3CommandExceptionType(|a, b, c| tr("commands.advancement.grant.criterion.to.many.failure", vec![a, b, c]));
static REVOKE_SINGLE_ONE: Dynamic2CommandExceptionType = Dynamic2CommandExceptionType(|a, b| tr("commands.advancement.revoke.one.to.one.failure", vec![a, b]));
static REVOKE_SINGLE_MANY: Dynamic2CommandExceptionType = Dynamic2CommandExceptionType(|a, b| tr("commands.advancement.revoke.one.to.many.failure", vec![a, b]));
static REVOKE_MANY_ONE: Dynamic2CommandExceptionType = Dynamic2CommandExceptionType(|a, b| tr("commands.advancement.revoke.many.to.one.failure", vec![a, b]));
static REVOKE_MANY_MANY: Dynamic2CommandExceptionType = Dynamic2CommandExceptionType(|a, b| tr("commands.advancement.revoke.many.to.many.failure", vec![a, b]));
static REVOKE_CRITERION_ONE: Dynamic3CommandExceptionType =
    Dynamic3CommandExceptionType(|a, b, c| tr("commands.advancement.revoke.criterion.to.one.failure", vec![a, b, c]));
static REVOKE_CRITERION_MANY: Dynamic3CommandExceptionType =
    Dynamic3CommandExceptionType(|a, b, c| tr("commands.advancement.revoke.criterion.to.many.failure", vec![a, b, c]));

impl Action {
    fn key(self) -> &'static str {
        match self {
            Action::Grant => "grant",
            Action::Revoke => "revoke",
        }
    }

    fn single_advancement_errors(self) -> (&'static Dynamic2CommandExceptionType, &'static Dynamic2CommandExceptionType) {
        match self {
            Action::Grant => (&GRANT_SINGLE_ONE, &GRANT_SINGLE_MANY),
            Action::Revoke => (&REVOKE_SINGLE_ONE, &REVOKE_SINGLE_MANY),
        }
    }

    fn multiple_advancements_errors(self) -> (&'static Dynamic2CommandExceptionType, &'static Dynamic2CommandExceptionType) {
        match self {
            Action::Grant => (&GRANT_MANY_ONE, &GRANT_MANY_MANY),
            Action::Revoke => (&REVOKE_MANY_ONE, &REVOKE_MANY_MANY),
        }
    }

    fn criterion_errors(self) -> (&'static Dynamic3CommandExceptionType, &'static Dynamic3CommandExceptionType) {
        match self {
            Action::Grant => (&GRANT_CRITERION_ONE, &GRANT_CRITERION_MANY),
            Action::Revoke => (&REVOKE_CRITERION_ONE, &REVOKE_CRITERION_MANY),
        }
    }

    fn perform_all(self, server: &mut Server, player: EntityId, advancements: &[&'static AdvancementHolder], show_advancements: bool) -> i32 {
        let mut count = 0;
        if !show_advancements {
            let p = server.player_mut(player);
            p.advancements.flush_dirty(&p.connection, true);
        }
        for &advancement in advancements {
            if self.perform(server, player, advancement) {
                count += 1;
            }
        }
        if !show_advancements {
            let p = server.player_mut(player);
            p.advancements.flush_dirty(&p.connection, false);
        }
        server.run_level_tasks();
        count
    }

    fn perform(self, server: &mut Server, player: EntityId, advancement: &'static AdvancementHolder) -> bool {
        let level = server.entity_level_mut(player);
        let progress = level.player_mut(player).advancements.get_or_start_progress(advancement);
        match self {
            Action::Grant => {
                if progress.is_done() {
                    return false;
                }
                for criterion in progress.get_remaining_criteria() {
                    PlayerAdvancements::award(level, player, advancement, &criterion);
                }
            }
            Action::Revoke => {
                if !progress.has_progress() {
                    return false;
                }
                for criterion in progress.get_completed_criteria() {
                    level.player_mut(player).advancements.revoke(advancement, &criterion);
                }
            }
        }
        true
    }

    fn perform_criterion(self, server: &mut Server, player: EntityId, advancement: &'static AdvancementHolder, criterion: &str) -> bool {
        let level = server.entity_level_mut(player);
        let result = match self {
            Action::Grant => PlayerAdvancements::award(level, player, advancement, criterion),
            Action::Revoke => level.player_mut(player).advancements.revoke(advancement, criterion),
        };
        server.run_level_tasks();
        result
    }
}

fn get_advancement(context: &CommandContext, name: &str) -> Result<&'static AdvancementHolder> {
    let key = context.get_identifier(name);
    ServerAdvancementManager::get_bound().get(key).ok_or_else(|| ERROR_INVALID_ADVANCEMENT.create(key.to_string()))
}

fn add_children(tree: &AdvancementTree, parent: &AdvancementNode, output: &mut Vec<&'static AdvancementHolder>) {
    for child in tree.children(parent) {
        output.push(child.holder);
        add_children(tree, child, output);
    }
}

fn get_advancements(target: &'static AdvancementHolder, mode: Mode) -> Vec<&'static AdvancementHolder> {
    let tree = ServerAdvancementManager::get_bound().tree();
    let Some(target_node) = tree.get(&target.id) else { return vec![target] };
    let mut advancements = Vec::new();
    if mode.parents() {
        let mut parent = tree.parent(target_node);
        while let Some(node) = parent {
            advancements.push(node.holder);
            parent = tree.parent(node);
        }
    }
    advancements.push(target);
    if mode.children() {
        add_children(tree, target_node, &mut advancements);
    }
    advancements
}

fn perform(server: &mut Server, source: &CommandSourceStack, players: &[EntityId], action: Action, advancements: &[&'static AdvancementHolder], show_advancements: bool) -> Result<i32> {
    let mut tracker = CommandResponseTracker::new();
    for &player in players {
        let count = action.perform_all(server, player, advancements, show_advancements);
        tracker.track_value(player, count);
    }
    let advancement_count = advancements.len() as i32;
    let key = action.key();
    if advancement_count == 1 {
        let advancement_name = Advancement::name(advancements[0]);
        if tracker.total_value() == 0 {
            let (single, multiple) = action.single_advancement_errors();
            return Err(tracker.dispatch_any(
                |player, _| single.create(advancement_name.clone(), server.entity_display_name(player)),
                |player_count, _| multiple.create(advancement_name.clone(), player_count),
            ));
        }
        return tracker.send_feedback_of(
            server,
            source,
            true,
            ElementType::NonZero,
            None,
            |player, _| {
                Component::translatable_with(&format!("commands.advancement.{key}.one.to.one.success"), vec![advancement_name.clone().into(), server.entity_display_name(player).into()])
            },
            |player_count, _| Component::translatable_with(&format!("commands.advancement.{key}.one.to.many.success"), vec![advancement_name.clone().into(), Arg::Int(player_count)]),
        );
    }
    if tracker.total_value() == 0 {
        let (single, multiple) = action.multiple_advancements_errors();
        return Err(tracker.dispatch_any(
            |player, _| single.create(Arg::Int(advancement_count), server.entity_display_name(player)),
            |player_count, _| multiple.create(Arg::Int(advancement_count), player_count),
        ));
    }
    tracker.send_feedback_of(
        server,
        source,
        true,
        ElementType::NonZero,
        None,
        |player, _| Component::translatable_with(&format!("commands.advancement.{key}.many.to.one.success"), vec![Arg::Int(advancement_count), server.entity_display_name(player).into()]),
        |player_count, _| Component::translatable_with(&format!("commands.advancement.{key}.many.to.many.success"), vec![Arg::Int(advancement_count), Arg::Int(player_count)]),
    )
}

fn perform_criterion(server: &mut Server, source: &CommandSourceStack, players: &[EntityId], action: Action, holder: &'static AdvancementHolder, criterion: &str) -> Result<i32> {
    let name = Advancement::name(holder);
    if holder.value.criterion(criterion).is_none() {
        return Err(ERROR_CRITERION_NOT_FOUND.create(name, criterion));
    }
    let mut tracker = CommandResponseTracker::new();
    for &player in players {
        let success = action.perform_criterion(server, player, holder, criterion);
        tracker.track_value(player, success as i32);
    }
    let key = action.key();
    if tracker.total_value() == 0 {
        let (single, multiple) = action.criterion_errors();
        return Err(tracker.dispatch_any(
            |player, _| single.create(criterion, name.clone(), server.entity_display_name(player)),
            |player_count, _| multiple.create(criterion, name.clone(), player_count),
        ));
    }
    tracker.send_feedback_of(
        server,
        source,
        true,
        ElementType::NonZero,
        None,
        |player, _| {
            Component::translatable_with(&format!("commands.advancement.{key}.criterion.to.one.success"), vec![criterion.into(), name.clone().into(), server.entity_display_name(player).into()])
        },
        |player_count, _| Component::translatable_with(&format!("commands.advancement.{key}.criterion.to.many.success"), vec![criterion.into(), name.clone().into(), Arg::Int(player_count)]),
    )
}

fn run(server: &mut Server, context: &CommandContext, source: &CommandSourceStack, action: Action, mode: Mode) -> Result<i32> {
    let players = context.get_players(server, source, "targets")?;
    let advancement = get_advancement(context, "advancement")?;
    perform(server, source, &players, action, &get_advancements(advancement, mode), true)
}

fn run_criterion(server: &mut Server, context: &CommandContext, source: &CommandSourceStack, action: Action) -> Result<i32> {
    let players = context.get_players(server, source, "targets")?;
    let advancement = get_advancement(context, "advancement")?;
    let criterion = context.get_string("criterion").to_string();
    perform_criterion(server, source, &players, action, advancement, &criterion)
}

pub fn grant_only(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32> {
    run(server, context, source, Action::Grant, Mode::Only)
}

pub fn grant_from(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32> {
    run(server, context, source, Action::Grant, Mode::From)
}

pub fn grant_until(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32> {
    run(server, context, source, Action::Grant, Mode::Until)
}

pub fn grant_through(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32> {
    run(server, context, source, Action::Grant, Mode::Through)
}

pub fn grant_criterion(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32> {
    run_criterion(server, context, source, Action::Grant)
}

pub fn grant_everything(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32> {
    let players = context.get_players(server, source, "targets")?;
    let advancements: Vec<&'static AdvancementHolder> = ServerAdvancementManager::get_bound().get_all_advancements().collect();
    perform(server, source, &players, Action::Grant, &advancements, false)
}

pub fn revoke_only(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32> {
    run(server, context, source, Action::Revoke, Mode::Only)
}

pub fn revoke_from(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32> {
    run(server, context, source, Action::Revoke, Mode::From)
}

pub fn revoke_until(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32> {
    run(server, context, source, Action::Revoke, Mode::Until)
}

pub fn revoke_through(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32> {
    run(server, context, source, Action::Revoke, Mode::Through)
}

pub fn revoke_criterion(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32> {
    run_criterion(server, context, source, Action::Revoke)
}

pub fn revoke_everything(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32> {
    let players = context.get_players(server, source, "targets")?;
    let advancements: Vec<&'static AdvancementHolder> = ServerAdvancementManager::get_bound().get_all_advancements().collect();
    perform(server, source, &players, Action::Revoke, &advancements, true)
}

pub fn suggest_criteria(_server: &Server, context: &CommandContext, _source: &CommandSourceStack, mut builder: SuggestionsBuilder) -> Result<Suggestions> {
    let advancement = get_advancement(context, "advancement")?;
    shared_suggestion_provider::suggest(advancement.value.criteria.iter().map(|(name, _)| name.as_str()), &mut builder);
    Ok(builder.build())
}
