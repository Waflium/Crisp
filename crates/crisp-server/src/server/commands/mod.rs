mod attribute_command;
mod ban_player_commands;
mod boss_bar_commands;
#[cfg(test)]
mod block_item_command_tests;
mod clear_inventory_commands;
mod clone_commands;
mod damage_command;
mod command_response_tracker;
mod data;
mod de_op_commands;
mod default_game_mode_commands;
mod difficulty_command;
mod effect_commands;
#[cfg(test)]
mod effect_command_tests;
mod emote_commands;
mod execute_command;
mod fill_command;
mod force_load_command;
mod give_command;
#[cfg(test)]
mod entity_command_tests;
pub mod game_mode_command;
mod game_rule_command;
mod help_command;
mod kick_command;
mod kill_command;
pub mod list_players_command;
mod look_at;
mod msg_command;
mod op_command;
mod play_sound_command;
mod random_command;
mod rotate_command;
mod save_commands;
mod say_command;
mod scoreboard_command;
#[cfg(test)]
mod scoreboard_command_tests;
mod seed_command;
mod set_block_command;
mod set_player_idle_timeout_command;
mod set_spawn_command;
mod set_world_spawn_command;
mod stop_sound_command;
mod stop_command;
mod stopwatch_command;
mod summon_command;
mod swing_command;
mod tag_command;
mod team_msg_command;
mod team_command;
mod teleport_command;
mod tell_raw_command;
mod tick_command;
mod time_command;
mod title_command;
mod transfer_command;
mod trigger_command;
mod version_command;
mod weather_command;
mod whitelist_command;
mod world_border_command;
#[cfg(test)]
mod world_border_command_tests;
#[cfg(test)]
mod suggestion_tests;

use crate::commands::{CommandFn, ModifierFn};

pub fn executors() -> Vec<(String, CommandFn)> {
    let mut all: Vec<(String, CommandFn)> = EXECUTORS.iter().map(|&(path, f)| (path.to_string(), f)).collect();
    all.extend(game_rule_command::executors());
    all.extend(fill_command::executors());
    all.extend(clone_commands::executors());
    all.extend(data::executors());
    all.extend(execute_command::executors());
    all.extend(scoreboard_command::executors());
    all.extend(team_command::executors());
    all.extend(play_sound_command::executors());
    all.extend(boss_bar_commands::executors());
    all.extend(stop_sound_command::executors());
    all.extend(effect_commands::executors());
    all.extend(attribute_command::executors());
    all.extend(world_border_command::executors());
    all
}

static EXECUTORS: &[(&str, CommandFn)] = &[
    ("execute if block pos block", execute_command::if_block_run),
    ("execute if blocks start end destination all", execute_command::if_blocks_run),
    ("execute if blocks start end destination masked", execute_command::if_blocks_run),
    ("execute unless block pos block", execute_command::if_block_run),
    ("execute unless blocks start end destination all", execute_command::if_blocks_run),
    ("execute unless blocks start end destination masked", execute_command::if_blocks_run),
    ("execute if dimension dimension", execute_command::if_dimension_run),
    ("execute if entity entities", execute_command::if_entity_run),
    ("execute if loaded pos", execute_command::if_loaded_run),
    ("execute unless dimension dimension", execute_command::if_dimension_run),
    ("execute unless entity entities", execute_command::if_entity_run),
    ("execute unless loaded pos", execute_command::if_loaded_run),
    ("ban targets", ban_player_commands::ban_players),
    ("ban targets reason", ban_player_commands::ban_players),
    ("ban-ip target", ban_player_commands::ban_ip),
    ("ban-ip target reason", ban_player_commands::ban_ip),
    ("banlist", ban_player_commands::ban_list),
    ("banlist ips", ban_player_commands::ban_list_ips),
    ("banlist players", ban_player_commands::ban_list_players),
    ("pardon targets", ban_player_commands::pardon_players),
    ("pardon-ip target", ban_player_commands::pardon_ip),
    ("bossbar add id name", boss_bar_commands::create_bar),
    ("bossbar get id max", boss_bar_commands::get_max),
    ("bossbar get id players", boss_bar_commands::get_players),
    ("bossbar get id value", boss_bar_commands::get_value),
    ("bossbar get id visible", boss_bar_commands::get_visible),
    ("bossbar list", boss_bar_commands::list_bars),
    ("bossbar remove id", boss_bar_commands::remove_bar),
    ("bossbar set id max max", boss_bar_commands::set_max),
    ("bossbar set id name name", boss_bar_commands::set_name),
    ("bossbar set id players", boss_bar_commands::set_players),
    ("bossbar set id players targets", boss_bar_commands::set_players),
    ("bossbar set id value value", boss_bar_commands::set_value),
    ("bossbar set id visible visible", boss_bar_commands::set_visible),
    ("whitelist add targets", whitelist_command::add_players),
    ("whitelist list", whitelist_command::show_list),
    ("whitelist off", whitelist_command::disable),
    ("whitelist on", whitelist_command::enable),
    ("whitelist reload", whitelist_command::reload),
    ("whitelist remove targets", whitelist_command::remove_players),
    ("clear", clear_inventory_commands::clear_self),
    ("clear targets", clear_inventory_commands::clear_targets),
    ("clear targets item", clear_inventory_commands::clear_item),
    ("clear targets item maxCount", clear_inventory_commands::clear_item_count),
    ("forceload add from", force_load_command::add),
    ("forceload add from to", force_load_command::add),
    ("forceload query", force_load_command::list_force_load),
    ("forceload query pos", force_load_command::query_force_load),
    ("forceload remove all", force_load_command::remove_all),
    ("forceload remove from", force_load_command::remove),
    ("forceload remove from to", force_load_command::remove),
    ("give targets item", give_command::give),
    ("give targets item count", give_command::give_count),
    ("setblock pos block", set_block_command::replace),
    ("setblock pos block destroy", set_block_command::destroy),
    ("setblock pos block keep", set_block_command::keep),
    ("setblock pos block replace", set_block_command::replace),
    ("setblock pos block strict", set_block_command::strict),
    ("help", help_command::help),
    ("help command", help_command::help_command),
    ("damage target amount", damage_command::damage_generic),
    ("damage target amount damageType", damage_command::damage_typed),
    ("damage target amount damageType at location", damage_command::damage_at),
    ("damage target amount damageType by entity", damage_command::damage_by),
    ("damage target amount damageType by entity from cause", damage_command::damage_by),
    ("defaultgamemode gamemode", default_game_mode_commands::set_mode),
    ("deop targets", de_op_commands::deop_players),
    ("difficulty", difficulty_command::query),
    ("difficulty peaceful", difficulty_command::set_difficulty),
    ("difficulty easy", difficulty_command::set_difficulty),
    ("difficulty normal", difficulty_command::set_difficulty),
    ("difficulty hard", difficulty_command::set_difficulty),
    ("gamemode gamemode", game_mode_command::set_mode_self),
    ("gamemode gamemode target", game_mode_command::set_mode_targets),
    ("kick targets", kick_command::kick),
    ("kick targets reason", kick_command::kick_with_reason),
    ("kill", kill_command::kill_self),
    ("kill targets", kill_command::kill_targets),
    ("me action", emote_commands::emote),
    ("msg targets message", msg_command::msg),
    ("op targets", op_command::op_players),
    ("say message", say_command::say),
    ("setworldspawn", set_world_spawn_command::set_spawn_here),
    ("setworldspawn pos", set_world_spawn_command::set_spawn_at),
    ("setworldspawn pos rotation", set_world_spawn_command::set_spawn_at_rotated),
    ("spawnpoint", set_spawn_command::set_spawn_self),
    ("spawnpoint targets", set_spawn_command::set_spawn_targets),
    ("spawnpoint targets pos", set_spawn_command::set_spawn_targets_at),
    ("spawnpoint targets pos rotation", set_spawn_command::set_spawn_targets_at_rotated),
    ("teleport location", teleport_command::teleport_to_location),
    ("teleport destination", teleport_command::teleport_to_destination),
    ("teleport targets location", teleport_command::teleport_targets_to_location),
    ("teleport targets location rotation", teleport_command::teleport_targets_to_location_rotated),
    ("teleport targets location facing entity facingEntity", teleport_command::teleport_targets_facing_entity),
    ("teleport targets location facing entity facingEntity facingAnchor", teleport_command::teleport_targets_facing_entity_anchor),
    ("teleport targets location facing facingLocation", teleport_command::teleport_targets_facing_location),
    ("teleport targets destination", teleport_command::teleport_targets_to_destination),
    ("list", list_players_command::list_players),
    ("list uuids", list_players_command::list_players_with_uuids),
    ("random roll range", random_command::roll),
    ("random roll range sequence", random_command::roll),
    ("random value range", random_command::value),
    ("random value range sequence", random_command::value),
    ("random reset *", random_command::reset_all_sequences),
    ("random reset * seed", random_command::reset_all_sequences_and_set_new_defaults),
    ("random reset * seed includeWorldSeed", random_command::reset_all_sequences_and_set_new_defaults),
    ("random reset * seed includeWorldSeed includeSequenceId", random_command::reset_all_sequences_and_set_new_defaults),
    ("random reset sequence", random_command::reset_sequence),
    ("random reset sequence seed", random_command::reset_sequence_with),
    ("random reset sequence seed includeWorldSeed", random_command::reset_sequence_with),
    ("random reset sequence seed includeWorldSeed includeSequenceId", random_command::reset_sequence_with),
    ("rotate target rotation", rotate_command::rotate),
    ("rotate target facing entity facingEntity", rotate_command::rotate_facing_entity),
    ("rotate target facing entity facingEntity facingAnchor", rotate_command::rotate_facing_entity_anchor),
    ("rotate target facing facingLocation", rotate_command::rotate_facing_location),
    ("save-all", save_commands::save_all),
    ("save-all flush", save_commands::save_all_flush),
    ("save-off", save_commands::save_off),
    ("save-on", save_commands::save_on),
    ("seed", seed_command::seed),
    ("setidletimeout minutes", set_player_idle_timeout_command::set_idle_timeout),
    ("stop", stop_command::stop),
    ("stopwatch create id", stopwatch_command::create_stopwatch),
    ("stopwatch query id", stopwatch_command::query_stopwatch),
    ("stopwatch query id scale", stopwatch_command::query_stopwatch),
    ("stopwatch remove id", stopwatch_command::remove_stopwatch),
    ("stopwatch restart id", stopwatch_command::restart_stopwatch),
    ("summon entity", summon_command::summon),
    ("summon entity pos", summon_command::summon_at),
    ("summon entity pos nbt", summon_command::summon_with_nbt),
    ("tick freeze", tick_command::freeze),
    ("tick query", tick_command::tick_query),
    ("tick rate rate", tick_command::set_ticking_rate),
    ("tick sprint stop", tick_command::stop_sprinting),
    ("tick sprint time", tick_command::sprint),
    ("tick step", tick_command::step),
    ("tick step stop", tick_command::stop_stepping),
    ("tick step time", tick_command::step),
    ("tick unfreeze", tick_command::unfreeze),
    ("time add time", time_command::add_time),
    ("time of clock add time", time_command::add_time),
    ("time of clock pause", time_command::pause),
    ("time of clock query time", time_command::query_time),
    ("time of clock query timeline", time_command::query_timeline_ticks),
    ("time of clock query timeline repetition", time_command::query_timeline_repetitions),
    ("time of clock rate rate", time_command::set_rate),
    ("time of clock resume", time_command::resume),
    ("time of clock set time", time_command::set_total_ticks),
    ("time of clock set timemarker", time_command::set_time_to_time_marker),
    ("time pause", time_command::pause),
    ("time query gametime", time_command::query_game_time),
    ("time query time", time_command::query_time),
    ("time query timeline", time_command::query_timeline_ticks),
    ("time query timeline repetition", time_command::query_timeline_repetitions),
    ("time rate rate", time_command::set_rate),
    ("time resume", time_command::resume),
    ("time set time", time_command::set_total_ticks),
    ("time set timemarker", time_command::set_time_to_time_marker),
    ("swing", swing_command::swing_self),
    ("swing targets", swing_command::swing_targets),
    ("swing targets mainhand", swing_command::swing_hand),
    ("swing targets mainhand animation", swing_command::swing_hand),
    ("swing targets mainhand animation duration", swing_command::swing_hand),
    ("swing targets offhand", swing_command::swing_hand),
    ("swing targets offhand animation", swing_command::swing_hand),
    ("swing targets offhand animation duration", swing_command::swing_hand),
    ("tag targets add name", tag_command::add_tag),
    ("tag targets list", tag_command::list_tags),
    ("tag targets remove name", tag_command::remove_tag),
    ("teammsg message", team_msg_command::team_msg),
    ("tellraw targets message", tell_raw_command::tell_raw),
    ("title targets actionbar title", title_command::show_action_bar),
    ("title targets clear", title_command::clear_title),
    ("title targets reset", title_command::reset_title),
    ("title targets subtitle title", title_command::show_subtitle),
    ("title targets times fadeIn stay fadeOut", title_command::set_times),
    ("title targets title title", title_command::show_title_text),
    ("transfer hostname", transfer_command::transfer),
    ("transfer hostname port", transfer_command::transfer),
    ("transfer hostname port players", transfer_command::transfer),
    ("trigger objective", trigger_command::simple_trigger),
    ("version", version_command::version),
    ("trigger objective add value", trigger_command::add_value),
    ("trigger objective set value", trigger_command::set_value),
    ("weather clear", weather_command::set_clear),
    ("weather clear duration", weather_command::set_clear),
    ("weather rain", weather_command::set_rain),
    ("weather rain duration", weather_command::set_rain),
    ("weather thunder", weather_command::set_thunder),
    ("weather thunder duration", weather_command::set_thunder),
];

pub fn modifiers() -> Vec<(String, ModifierFn)> {
    let mut all: Vec<(String, ModifierFn)> = MODIFIERS.iter().map(|&(path, f)| (path.to_string(), f)).collect();
    all.extend(execute_command::modifiers());
    all
}

static MODIFIERS: &[(&str, ModifierFn)] = &[
    ("execute if block pos block", execute_command::if_block),
    ("execute if blocks start end destination all", execute_command::if_blocks),
    ("execute if blocks start end destination masked", execute_command::if_blocks),
    ("execute unless block pos block", execute_command::if_block),
    ("execute unless blocks start end destination all", execute_command::if_blocks),
    ("execute unless blocks start end destination masked", execute_command::if_blocks),
    ("execute align axes", execute_command::align),
    ("execute anchored anchor", execute_command::anchored),
    ("execute as targets", execute_command::as_targets),
    ("execute at targets", execute_command::at_targets),
    ("execute facing entity targets anchor", execute_command::facing_entity),
    ("execute facing pos", execute_command::facing_pos),
    ("execute if dimension dimension", execute_command::if_dimension),
    ("execute if entity entities", execute_command::if_entity),
    ("execute if loaded pos", execute_command::if_loaded),
    ("execute in dimension", execute_command::in_dimension),
    ("execute positioned as targets", execute_command::positioned_as),
    ("execute positioned pos", execute_command::positioned),
    ("execute rotated as targets", execute_command::rotated_as),
    ("execute rotated rot", execute_command::rotated),
    ("execute summon entity", execute_command::summon),
    ("execute unless dimension dimension", execute_command::if_dimension),
    ("execute unless entity entities", execute_command::if_entity),
    ("execute unless loaded pos", execute_command::if_loaded),
];

#[cfg(test)]
mod tests {
    use crate::{LevelType, ServerConfig};
    use crate::commands::perform_prefixed_command;
    use crate::commands::source::CommandSourceStack;
    use crate::server::Server;
    use crate::world::clock::ServerClockManager;

    fn run(server: &mut Server, command: &str) -> Vec<String> {
        server.console_log.borrow_mut().clear();
        let source = CommandSourceStack::server(server);
        perform_prefixed_command(server, &source, command);
        server.console_log.borrow().clone()
    }

    fn overworld_ticks(server: &Server) -> i64 {
        server.clocks.clocks[ServerClockManager::id_of("minecraft:overworld").unwrap()].total_ticks
    }

    #[test]
    fn time_set_ticks_and_markers() {
        let mut server = Server::new(ServerConfig { level_type: LevelType::Flat, ..ServerConfig::default() }).unwrap();
        assert_eq!(run(&mut server, "time set 1000"), ["Set minecraft:overworld to 1000 tick(s)"]);
        assert_eq!(overworld_ticks(&server), 1000);
        run(&mut server, "time set 0.5d");
        assert_eq!(overworld_ticks(&server), 12000);
        assert_eq!(run(&mut server, "time set noon"), ["Set minecraft:overworld to time marker minecraft:noon"]);
        assert_eq!(overworld_ticks(&server), 24000 + 6000);
        assert_eq!(run(&mut server, "/time set noon"), ["Clock minecraft:overworld is already at time marker minecraft:noon"]);
    }

    #[test]
    fn syntax_errors_show_their_context() {
        let mut server = Server::new(ServerConfig { level_type: LevelType::Flat, ..ServerConfig::default() }).unwrap();
        assert_eq!(run(&mut server, "foo"), ["Unknown or incomplete command. See below for error", "foo<--[HERE]"]);
        assert_eq!(run(&mut server, "time set 1x"), ["Time marker minecraft:1x does not exist for clock minecraft:overworld"]);
        assert_eq!(run(&mut server, "time add 1x"), ["Invalid unit", "...ime add 1x<--[HERE]"]);
        assert_eq!(run(&mut server, "setidletimeout -5"), ["Integer must not be less than 0: found -5", "...letimeout -5<--[HERE]"]);
    }

    #[test]
    fn help_matches_vanilla() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.vanilla/extracted");
        let mut server = Server::new(crate::ServerConfig { level_type: crate::LevelType::Flat, ..crate::ServerConfig::default() }).unwrap();
        for level in [0, 2, 4] {
            let expected = std::fs::read_to_string(dir.join(format!("help_{level}.txt"))).expect("run cargo xtask vanilla");
            let mut source = CommandSourceStack::server(&server);
            source.permission = level;
            let mut lines: Vec<String> = super::help_command::smart_usage(crisp_data::commands::CommandNode::root(), &source).iter().map(|l| format!("/{l}")).collect();
            if level >= 2 {
                let execute = crate::commands::node_at("execute");
                lines.extend(super::help_command::smart_usage(execute, &source).iter().map(|l| format!("/execute {l}")));
            }
            assert_eq!(lines, expected.lines().collect::<Vec<_>>(), "permission level {level}");
        }
        let lines = run(&mut server, "help");
        assert!(lines.contains(&"/time (set|add|pause|resume|rate|query|of)".to_string()), "{lines:?}");
        assert_eq!(run(&mut server, "help nosuchcommand"), ["Unknown command or insufficient permissions"]);
    }

    #[test]
    fn bans_and_whitelist() {
        let mut server = Server::new(crate::ServerConfig { level_type: crate::LevelType::Flat, online_mode: false, ..crate::ServerConfig::default() }).unwrap();
        assert_eq!(run(&mut server, "banlist"), ["There are no bans"]);
        assert_eq!(run(&mut server, "ban Bob"), ["Banned bob: Banned by an operator."]);
        assert_eq!(run(&mut server, "ban Bob"), ["Nothing changed. The player is already banned"]);
        assert_eq!(run(&mut server, "ban-ip 10.0.0.1 spam"), ["Banned IP 10.0.0.1: spam"]);
        assert_eq!(run(&mut server, "ban-ip nobody"), ["Invalid IP address or unknown player"]);
        let list = run(&mut server, "banlist");
        assert_eq!(list[0], "There are 2 ban(s):");
        assert!(list.contains(&"bob was banned by Server: Banned by an operator.".to_string()), "{list:?}");
        assert!(list.contains(&"10.0.0.1 was banned by Server: spam".to_string()), "{list:?}");
        assert_eq!(run(&mut server, "pardon Bob"), ["Unbanned bob"]);
        assert_eq!(run(&mut server, "whitelist add Bob"), ["Added bob to the whitelist"]);
        assert_eq!(run(&mut server, "whitelist list"), ["There are 1 whitelisted player(s): bob"]);
    }

    #[test]
    fn execute_modifiers_and_conditions() {
        let mut server = Server::new(crate::ServerConfig { level_type: crate::LevelType::Flat, ..crate::ServerConfig::default() }).unwrap();
        assert_eq!(run(&mut server, "execute if entity @e"), ["Test failed"]);
        assert_eq!(run(&mut server, "execute unless entity @e"), ["Test passed"]);
        assert_eq!(run(&mut server, "execute if entity @e run seed").len(), 0);
        assert_eq!(run(&mut server, "execute in minecraft:the_nether run seed"), ["Unknown dimension 'minecraft:the_nether'"]);
        run(&mut server, "execute positioned 10.7 70 -3.2 align xz run setworldspawn");
        assert_eq!((server.spawn.x, server.spawn.y, server.spawn.z), (10, 70, -4));
        crate::server::level::testing::player(&mut server.level, crisp_core::Vec3::new(5.5, -60.0, 5.5), 0.0, 0.0);
        assert_eq!(run(&mut server, "execute if entity @a"), ["Test passed. Count: 1"]);
        server.tick();
        assert_eq!(run(&mut server, "execute if block 0 -61 0 minecraft:grass_block"), ["Test passed"]);
        assert_eq!(run(&mut server, "execute unless block 0 -61 0 #minecraft:dirt"), ["Test passed"]);
        assert_eq!(run(&mut server, "execute if blocks 0 -64 0 2 -61 2 5 -64 5 all"), ["Test passed. Count: 36"]);
        assert_eq!(run(&mut server, "execute if blocks 0 -64 0 2 -60 2 5 -64 5 masked"), ["Test passed. Count: 36"]);
        assert_eq!(run(&mut server, "execute unless blocks 0 -64 0 2 -60 2 5 -63 5 all"), ["Test passed"]);
        run(&mut server, "execute at @a run setworldspawn");
        assert_eq!((server.spawn.x, server.spawn.y, server.spawn.z), (5, -60, 5));
    }

    #[test]
    fn game_rules() {
        let mut server = Server::new(ServerConfig { level_type: LevelType::Flat, ..ServerConfig::default() }).unwrap();
        assert_eq!(run(&mut server, "gamerule random_tick_speed"), ["Game rule random_tick_speed is currently set to 3"]);
        assert_eq!(run(&mut server, "gamerule minecraft:random_tick_speed 10"), ["Game rule random_tick_speed is now set to 10"]);
        assert_eq!(server.level.game_rules.get_int(crisp_data::game_rule::RANDOM_TICK_SPEED), 10);
        assert_eq!(run(&mut server, "gamerule random_tick_speed 10"), ["Game rule random_tick_speed is already set to 10"]);
        assert_eq!(run(&mut server, "gamerule keep_inventory true"), ["Game rule keep_inventory is now set to true"]);
    }

    #[test]
    fn data_storage() {
        let mut server = Server::new(crate::ServerConfig { level_type: crate::LevelType::Flat, ..crate::ServerConfig::default() }).unwrap();
        assert_eq!(run(&mut server, "data get storage t:s"), ["Storage t:s has the following contents: {}"]);
        assert_eq!(run(&mut server, "data merge storage t:s {z:1,list:[1,2]}"), ["Modified storage t:s"]);
        assert_eq!(run(&mut server, "data merge storage t:s {z:1}"), ["Nothing changed. The specified properties already have these values"]);
        assert_eq!(run(&mut server, "data get storage t:s"), ["Storage t:s has the following contents: {z: 1, list: [1, 2]}"]);
        assert_eq!(run(&mut server, "data get storage t:s list"), ["Storage t:s has the following contents: [1, 2]"]);
        assert_eq!(run(&mut server, "data get storage t:s list[]"), ["This argument accepts a single NBT value"]);
        assert_eq!(run(&mut server, "data get storage t:s z 2.5"), ["z in storage t:s after scale factor of 2.50 is 2"]);
        assert_eq!(run(&mut server, "data get storage t:s list 1"), ["Can't get list; only numeric tags are allowed"]);
        assert_eq!(run(&mut server, "data modify storage t:s list append value 3"), ["Modified storage t:s"]);
        assert_eq!(run(&mut server, "data modify storage t:s list insert -2 from storage t:s z"), ["Modified storage t:s"]);
        assert_eq!(run(&mut server, "data modify storage t:s name set string storage t:s list[0] 0 1"), ["Modified storage t:s"]);
        assert_eq!(run(&mut server, "data modify storage t:s name set string storage t:s list 0 1"), ["Expected a value: got [1,2,1,3]"]);
        assert_eq!(run(&mut server, "data modify storage t:s name set string storage t:s z 0 5"), ["Invalid substring indices: 0 to 5"]);
        assert_eq!(run(&mut server, "data get storage t:s"), ["Storage t:s has the following contents: {name: \"1\", z: 1, list: [1, 2, 1, 3]}"]);
        assert_eq!(run(&mut server, "data modify storage t:s z merge value {a:1}"), ["Expected an object: got 1"]);
        assert_eq!(run(&mut server, "data remove storage t:s list[{}]"), ["Nothing changed. The specified properties already have these values"]);
        assert_eq!(run(&mut server, "data remove storage t:s list[1]"), ["Modified storage t:s"]);
        assert_eq!(run(&mut server, "data get storage t:s list"), ["Storage t:s has the following contents: [1, 1, 3]"]);
        assert_eq!(run(&mut server, "data get storage t:s nope"), ["Found no elements matching nope"]);
        assert_eq!(run(&mut server, "data modify storage t:s a.b merge value {}"), ["Nothing changed. The specified properties already have these values"]);
        assert_eq!(run(&mut server, "data get storage t:s a"), ["Storage t:s has the following contents: {b: {}}"]);
        assert_eq!(run(&mut server, "data modify storage t:t a.b merge value {}"), ["Nothing changed. The specified properties already have these values"]);
        assert_eq!(run(&mut server, "data get storage t:t"), ["Storage t:t has the following contents: {}"]);
        run(&mut server, "data remove storage t:s z");
        run(&mut server, "data remove storage t:s name");
        run(&mut server, "data remove storage t:s list");
        run(&mut server, "data remove storage t:s a");
        assert_eq!(server.command_storage.keys(), Vec::<String>::new());
    }

    #[test]
    fn execute_store_and_if_data() {
        let mut server = Server::new(crate::ServerConfig { level_type: crate::LevelType::Flat, ..crate::ServerConfig::default() }).unwrap();
        run(&mut server, "data merge storage t:s {list:[1,2],big:40000}");
        assert_eq!(run(&mut server, "execute store result storage t:s n int 3 run data get storage t:s list"), ["Storage t:s has the following contents: [1, 2]"]);
        run(&mut server, "execute store result storage t:s s short 1 run data get storage t:s big");
        run(&mut server, "execute store result storage t:s f float 0.5 run data get storage t:s list");
        assert_eq!(run(&mut server, "execute store success storage t:s ok byte 1 run execute if entity @e"), ["Test failed"]);
        assert_eq!(run(&mut server, "execute store result storage t:s list[5] int 1 run data get storage t:s big").len(), 1);
        assert_eq!(run(&mut server, "data get storage t:s"), ["Storage t:s has the following contents: {big: 40000, s: -25536s, f: 1.0f, list: [1, 2], ok: 0b, n: 6}"]);
        assert_eq!(run(&mut server, "execute if data storage t:s list[]"), ["Test passed. Count: 2"]);
        assert_eq!(run(&mut server, "execute unless data storage t:s list[]"), ["Test failed. Count: 2"]);
        assert_eq!(run(&mut server, "execute positioned 0 0 0 unless data storage t:s nope"), ["Test passed"]);
        assert_eq!(run(&mut server, "execute positioned 0 0 0 if data storage t:s nope"), ["Test failed"]);
        assert_eq!(run(&mut server, "execute positioned 0 0 0 if data storage t:s n run data get storage t:s n"), ["Storage t:s has the following contents: 6"]);
        assert_eq!(run(&mut server, "execute positioned 0 0 0 if entity @e").len(), 1);
    }

    #[test]
    fn idle_players_are_kicked() {
        let mut server = Server::new(crate::ServerConfig { level_type: crate::LevelType::Flat, ..crate::ServerConfig::default() }).unwrap();
        let id = crate::server::level::testing::player(&mut server.level, crisp_core::Vec3::new(5.5, -60.0, 5.5), 0.0, 0.0);
        assert_eq!(run(&mut server, "setidletimeout 1"), ["The player idle timeout is now 1 minute(s)"]);
        assert!(!server.kick_if_idle(id));
        server.level.player_mut(id).last_action_time -= std::time::Duration::from_secs(61);
        assert!(server.kick_if_idle(id));
        assert!(server.level.player(id).disconnected);
    }

    #[test]
    fn summon() {
        let mut server = Server::new(crate::ServerConfig { level_type: crate::LevelType::Flat, ..crate::ServerConfig::default() }).unwrap();
        assert_eq!(run(&mut server, "summon tnt 0 -60 0 {fuse:5}"), ["Summoned new Primed TNT"]);
        assert_eq!(run(&mut server, "summon item 0 -60 0"), ["Unable to summon entity due to duplicate UUIDs"]);
        assert_eq!(run(&mut server, "summon item 0 -60 0 {Item:{id:\"stone\",count:2}}"), ["Summoned new Stone"]);
        assert_eq!(run(&mut server, "summon player"), ["Can't summon entity of type minecraft:player"]);
        assert_eq!(run(&mut server, "summon tnt 0 30000001 0"), ["Invalid position for summon"]);
        assert_eq!(server.level.entities.len(), 2);
    }
}

pub fn suggestion_providers() -> Vec<(String, crate::commands::SuggestFn)> {
    use crate::commands::SuggestFn;
    let score_holders: SuggestFn = crate::commands::arguments::score_holder_argument::suggest_score_holders;
    let boss_bar: SuggestFn = boss_bar_commands::suggest_boss_bar;
    let storage: SuggestFn = data::data_accessor::suggest_storage;
    let stopwatches: SuggestFn = stopwatch_command::suggest_stopwatches;
    let random_sequence: SuggestFn = random_command::suggest_random_sequence;
    let time_markers: SuggestFn = time_command::suggest_time_markers;
    let timelines: SuggestFn = time_command::suggest_timelines;
    let modifiers: SuggestFn = attribute_command::suggest_modifiers;
    let mut all: Vec<(String, SuggestFn)> = Vec::new();
    for condition in ["if", "unless"] {
        all.push((format!("execute {condition} score target"), score_holders));
        for comparison in ["=", "<", "<=", ">", ">="] {
            all.push((format!("execute {condition} score target targetObjective {comparison} source"), score_holders));
        }
        all.push((format!("execute {condition} data storage source"), storage));
        all.push((format!("execute {condition} stopwatch id"), stopwatches));
    }
    for store in ["result", "success"] {
        all.push((format!("execute store {store} score targets"), score_holders));
        all.push((format!("execute store {store} bossbar id"), boss_bar));
        all.push((format!("execute store {store} storage target"), storage));
    }
    for subcommand in ["remove", "set", "get"] {
        all.push((format!("bossbar {subcommand} id"), boss_bar));
    }
    for subcommand in ["merge", "get", "remove", "modify"] {
        all.push((format!("data {subcommand} storage target"), storage));
    }
    for target in ["entity", "block", "storage"] {
        for operation in ["insert index", "prepend", "append", "set", "merge"] {
            for source in ["from", "string"] {
                all.push((format!("data modify {target} target targetPath {operation} {source} storage source"), storage));
            }
        }
    }
    for subcommand in ["query", "restart", "remove"] {
        all.push((format!("stopwatch {subcommand} id"), stopwatches));
    }
    for path in ["random value range sequence", "random roll range sequence", "random reset sequence"] {
        all.push((path.to_string(), random_sequence));
    }
    for path in [
        "scoreboard players list target",
        "scoreboard players set targets",
        "scoreboard players get target",
        "scoreboard players add targets",
        "scoreboard players remove targets",
        "scoreboard players reset targets",
        "scoreboard players enable targets",
        "scoreboard players display name targets",
        "scoreboard players display numberformat targets",
        "scoreboard players operation targets",
        "scoreboard players operation targets targetObjective operation source",
        "team join team members",
        "team leave members",
    ] {
        all.push((path.to_string(), score_holders));
    }
    all.push(("scoreboard players enable targets objective".into(), scoreboard_command::suggest_triggers));
    all.push(("time set timemarker".into(), time_markers));
    all.push(("time of clock set timemarker".into(), time_markers));
    all.push(("time query timeline".into(), timelines));
    all.push(("time of clock query timeline".into(), timelines));
    all.push(("tick rate rate".into(), tick_command::suggest_rate));
    all.push(("tick step time".into(), tick_command::suggest_step));
    all.push(("tick sprint time".into(), tick_command::suggest_sprint));
    all.push(("trigger objective".into(), trigger_command::suggest_objectives));
    all.push(("tag targets remove name".into(), tag_command::suggest_tags));
    all.push(("attribute target attribute modifier remove id".into(), modifiers));
    all.push(("attribute target attribute modifier value get id".into(), modifiers));
    all.push(("op targets".into(), op_command::suggest_non_operators));
    all.push(("deop targets".into(), de_op_commands::suggest_operators));
    all.push(("pardon targets".into(), ban_player_commands::suggest_banned_players));
    all.push(("pardon-ip target".into(), ban_player_commands::suggest_banned_ips));
    all.push(("whitelist add targets".into(), whitelist_command::suggest_unlisted_players));
    all.push(("whitelist remove targets".into(), whitelist_command::suggest_listed_players));
    all
}
