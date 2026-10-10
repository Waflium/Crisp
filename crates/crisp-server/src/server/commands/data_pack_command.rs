use std::sync::Arc;

use serde_json::{Value, json};

use super::list_players_command::format_list;
use super::reload_command::reload_packs;
use crate::commands::CommandFn;
use crate::commands::brigadier::{CommandContext, CommandSyntaxException, Dynamic2CommandExceptionType, DynamicCommandExceptionType, tr};
use crate::commands::shared_suggestion_provider;
use crate::commands::source::CommandSourceStack;
use crate::commands::suggestion::{Suggestions, SuggestionsBuilder};
use crate::network::chat::component_utils;
use crate::network::chat::{Arg, Component};
use crate::server::Server;
use crate::server::packs::metadata::PackFormat;
use crate::server::packs::repository::Pack;
use crate::server::packs::{PackSource, escape_if_required};
use crate::world::flag::print_missing_flags;

static ERROR_UNKNOWN_PACK: DynamicCommandExceptionType = DynamicCommandExceptionType(|id| tr("commands.datapack.unknown", vec![id]));
static ERROR_PACK_ALREADY_ENABLED: DynamicCommandExceptionType = DynamicCommandExceptionType(|id| tr("commands.datapack.enable.failed", vec![id]));
static ERROR_PACK_ALREADY_DISABLED: DynamicCommandExceptionType = DynamicCommandExceptionType(|id| tr("commands.datapack.disable.failed", vec![id]));
static ERROR_CANNOT_DISABLE_FEATURE: DynamicCommandExceptionType =
    DynamicCommandExceptionType(|id| tr("commands.datapack.disable.failed.feature", vec![id]));
static ERROR_PACK_FEATURES_NOT_ENABLED: Dynamic2CommandExceptionType =
    Dynamic2CommandExceptionType(|id, flags| tr("commands.datapack.enable.failed.no_flags", vec![id, flags]));
static ERROR_PACK_INVALID_NAME: DynamicCommandExceptionType = DynamicCommandExceptionType(|id| tr("commands.datapack.create.invalid_name", vec![id]));
static ERROR_PACK_INVALID_FULL_NAME: DynamicCommandExceptionType =
    DynamicCommandExceptionType(|id| tr("commands.datapack.create.invalid_full_name", vec![id]));
static ERROR_PACK_ALREADY_EXISTS: DynamicCommandExceptionType = DynamicCommandExceptionType(|id| tr("commands.datapack.create.already_exists", vec![id]));
static ERROR_PACK_IO_FAILURE: DynamicCommandExceptionType = DynamicCommandExceptionType(|id| tr("commands.datapack.create.io_failure", vec![id]));

pub fn executors() -> Vec<(String, CommandFn)> {
    vec![
        ("datapack enable name".into(), enable_default as CommandFn),
        ("datapack enable name after existing".into(), enable_after),
        ("datapack enable name before existing".into(), enable_before),
        ("datapack enable name last".into(), enable_last),
        ("datapack enable name first".into(), enable_first),
        ("datapack disable name".into(), disable_pack),
        ("datapack list".into(), list_packs),
        ("datapack list available".into(), list_available_packs),
        ("datapack list enabled".into(), list_enabled_packs),
        ("datapack create id description".into(), create_pack),
    ]
}

pub fn suggest_selected_packs(server: &Server, _context: &CommandContext, _source: &CommandSourceStack, mut builder: SuggestionsBuilder) -> Result<Suggestions, CommandSyntaxException> {
    shared_suggestion_provider::suggest(server.pack_repository.selected_ids().into_iter().map(escape_if_required), &mut builder);
    Ok(builder.build())
}

pub fn suggest_unselected_packs(server: &Server, _context: &CommandContext, _source: &CommandSourceStack, mut builder: SuggestionsBuilder) -> Result<Suggestions, CommandSyntaxException> {
    let repository = &server.pack_repository;
    let selected_ids = repository.selected_ids();
    let enabled_features = server.data_configuration.enabled_features;
    let unselected = repository
        .available_packs()
        .filter(|pack| pack.requested_features().is_subset_of(enabled_features))
        .map(|pack| pack.id())
        .filter(|id| !selected_ids.contains(id))
        .map(escape_if_required);
    shared_suggestion_provider::suggest(unselected, &mut builder);
    Ok(builder.build())
}

fn is_valid_path_segment(segment: &str) -> bool {
    segment != ".." && segment != "." && !segment.is_empty() && segment.bytes().all(|c| matches!(c, b'-' | b'.' | b'_' | b'a'..=b'z' | b'0'..=b'9'))
}

fn is_path_part_portable(name: &str) -> bool {
    if name.ends_with('.') {
        return false;
    }
    let base = name.split('.').next().unwrap_or_default().to_ascii_uppercase();
    let numbered = |prefix: &str| base.len() == 4 && base.starts_with(prefix) && matches!(base.as_bytes()[3], b'1'..=b'9');
    !(matches!(base.as_str(), "COM" | "CLOCK$" | "CON" | "PRN" | "AUX" | "NUL") || numbered("COM") || numbered("LPT"))
}

fn create_pack(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let id = context.get_string("id").to_string();
    let description = component_utils::update_for_entity(server, source, context.get_raw_component("description"), None)?;
    let Some(world_dir) = &server.world_dir else { return Err(ERROR_PACK_IO_FAILURE.create(id)) };
    let datapack_dir = world_dir.root.join("datapacks");
    if !is_valid_path_segment(&id) {
        return Err(ERROR_PACK_INVALID_NAME.create(id));
    }
    if !is_path_part_portable(&id) {
        return Err(ERROR_PACK_INVALID_FULL_NAME.create(id));
    }
    let pack_dir = datapack_dir.join(&id);
    if pack_dir.exists() {
        return Err(ERROR_PACK_ALREADY_EXISTS.create(id));
    }
    let format = PackFormat::CURRENT.major;
    let top_mcmeta: Value = json!({ "pack": { "description": description.to_json_value(), "min_format": format, "max_format": format } });
    let written = std::fs::create_dir(&pack_dir)
        .and_then(|_| std::fs::create_dir(pack_dir.join("data")))
        .and_then(|_| std::fs::write(pack_dir.join("pack.mcmeta"), serde_json::to_string_pretty(&top_mcmeta).unwrap()));
    if let Err(e) = written {
        let absolute = std::path::absolute(&datapack_dir).unwrap_or(datapack_dir);
        warn!("Failed to create pack at {}\njava.io.IOException: {e}", absolute.display());
        return Err(ERROR_PACK_IO_FAILURE.create(id));
    }
    source.send_success(server, || tr("commands.datapack.create.success", vec![Arg::String(id.clone())]), true);
    Ok(1)
}

fn enable_pack(
    server: &mut Server,
    source: &CommandSourceStack,
    unopened: Arc<Pack>,
    inserter: impl FnOnce(&mut Server, &mut Vec<Arc<Pack>>, Arc<Pack>) -> Result<(), CommandSyntaxException>,
) -> Result<i32, CommandSyntaxException> {
    let mut selected = server.pack_repository.selected_packs().to_vec();
    inserter(server, &mut selected, unopened.clone())?;
    source.send_success(server, || tr("commands.datapack.modify.enable", vec![Arg::Component(unopened.chat_link(true))]), true);
    let ids: Vec<String> = selected.iter().map(|pack| pack.id().to_string()).collect();
    reload_packs(server, &ids);
    Ok(selected.len() as i32)
}

fn index_of(list: &[Arc<Pack>], pack: &Pack) -> usize {
    list.iter().position(|p| p.id() == pack.id()).unwrap()
}

fn enable_default(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let pack = get_pack(server, context, "name", true)?;
    enable_pack(server, source, pack, |_, list, pack| {
        pack.default_position().insert(list, pack, |p| p.selection_config, false);
        Ok(())
    })
}

fn enable_after(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let pack = get_pack(server, context, "name", true)?;
    enable_pack(server, source, pack, |server, list, pack| {
        let existing = get_pack(server, context, "existing", false)?;
        list.insert(index_of(list, &existing) + 1, pack);
        Ok(())
    })
}

fn enable_before(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let pack = get_pack(server, context, "name", true)?;
    enable_pack(server, source, pack, |server, list, pack| {
        let existing = get_pack(server, context, "existing", false)?;
        list.insert(index_of(list, &existing), pack);
        Ok(())
    })
}

fn enable_last(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let pack = get_pack(server, context, "name", true)?;
    enable_pack(server, source, pack, |_, list, pack| {
        list.push(pack);
        Ok(())
    })
}

fn enable_first(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let pack = get_pack(server, context, "name", true)?;
    enable_pack(server, source, pack, |_, list, pack| {
        list.insert(0, pack);
        Ok(())
    })
}

fn disable_pack(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let unopened = get_pack(server, context, "name", false)?;
    let mut selected = server.pack_repository.selected_packs().to_vec();
    if let Some(index) = selected.iter().position(|p| p.id() == unopened.id()) {
        selected.remove(index);
    }
    source.send_success(server, || tr("commands.datapack.modify.disable", vec![Arg::Component(unopened.chat_link(true))]), true);
    let ids: Vec<String> = selected.iter().map(|pack| pack.id().to_string()).collect();
    reload_packs(server, &ids);
    Ok(selected.len() as i32)
}

fn list_packs(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    Ok(list_enabled_packs(server, context, source)? + list_available_packs(server, context, source)?)
}

fn list_available_packs(server: &mut Server, _context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    server.pack_repository.reload();
    let enabled_features = server.data_configuration.enabled_features;
    let unselected_packs: Vec<Arc<Pack>> = server
        .pack_repository
        .available_packs()
        .filter(|p| !server.pack_repository.selected_packs().iter().any(|s| s.id() == p.id()) && p.requested_features().is_subset_of(enabled_features))
        .cloned()
        .collect();
    if unselected_packs.is_empty() {
        source.send_success(server, || Component::translatable("commands.datapack.list.available.none"), false);
    } else {
        source.send_success(
            server,
            || {
                let links = unselected_packs.iter().map(|p| p.chat_link(false)).collect();
                tr("commands.datapack.list.available.success", vec![Arg::Int(unselected_packs.len() as i32), Arg::Component(format_list(links))])
            },
            false,
        );
    }
    Ok(unselected_packs.len() as i32)
}

fn list_enabled_packs(server: &mut Server, _context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    server.pack_repository.reload();
    let selected_packs = server.pack_repository.selected_packs().to_vec();
    if selected_packs.is_empty() {
        source.send_success(server, || Component::translatable("commands.datapack.list.enabled.none"), false);
    } else {
        source.send_success(
            server,
            || {
                let links = selected_packs.iter().map(|p| p.chat_link(true)).collect();
                tr("commands.datapack.list.enabled.success", vec![Arg::Int(selected_packs.len() as i32), Arg::Component(format_list(links))])
            },
            false,
        );
    }
    Ok(selected_packs.len() as i32)
}

fn get_pack(server: &Server, context: &CommandContext, name: &str, enabling: bool) -> Result<Arc<Pack>, CommandSyntaxException> {
    let id = context.get_string(name);
    let repository = &server.pack_repository;
    let Some(pack) = repository.pack(id) else { return Err(ERROR_UNKNOWN_PACK.create(id)) };
    let enabled = repository.selected_packs().iter().any(|p| p.id() == id);
    if enabling && enabled {
        return Err(ERROR_PACK_ALREADY_ENABLED.create(id));
    }
    if !enabling && !enabled {
        return Err(ERROR_PACK_ALREADY_DISABLED.create(id));
    }
    let available_features = server.data_configuration.enabled_features;
    let requested_features = pack.requested_features();
    if !enabling && !requested_features.is_empty() && pack.pack_source() == PackSource::Feature {
        return Err(ERROR_CANNOT_DISABLE_FEATURE.create(id));
    }
    if !requested_features.is_subset_of(available_features) {
        return Err(ERROR_PACK_FEATURES_NOT_ENABLED.create(id, print_missing_flags(available_features, requested_features)));
    }
    Ok(pack.clone())
}
