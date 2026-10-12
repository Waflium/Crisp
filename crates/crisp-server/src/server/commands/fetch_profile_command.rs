use crisp_protocol::nbt::Nbt;

use crate::commands::brigadier::{CommandContext, CommandSyntaxException, DynamicCommandExceptionType, tr};
use crate::commands::source::CommandSourceStack;
use crate::network::chat::{ChatFormatting, ClickEvent, Component, Contents, Style};
use crate::network::login::Profile;
use crate::server::Server;
use crate::server::players::cached_user_name_to_id_resolver::is_valid_player_name;
use crate::world::entity::EntityId;
use crate::world::level::storage::{format_uuid, uuid_ints};

static NO_PROFILE: DynamicCommandExceptionType = DynamicCommandExceptionType(|id| tr("commands.fetchprofile.no_profile", vec![id]));

type Result<T> = std::result::Result<T, CommandSyntaxException>;

fn encode_stored_game_profile(profile: &Profile) -> std::result::Result<Nbt, String> {
    let name_length = profile.name.encode_utf16().count();
    if name_length > 16 {
        return Err(format!("String \"{}\" is too long: {name_length} > 16", profile.name));
    }
    if !is_valid_player_name(&profile.name) {
        return Err(format!("Player name contained disallowed characters: '{}'", profile.name));
    }
    let mut fields = vec![("id".to_string(), uuid_ints(profile.uuid)), ("name".to_string(), Nbt::String(profile.name.clone()))];
    if !profile.properties.is_empty() {
        let properties = profile
            .properties
            .iter()
            .map(|property| {
                let mut entry = vec![("name".to_string(), Nbt::String(property.name.clone())), ("value".to_string(), Nbt::String(property.value.clone()))];
                if let Some(signature) = &property.signature {
                    entry.push(("signature".to_string(), Nbt::String(signature.clone())));
                }
                Nbt::Compound(entry)
            })
            .collect();
        fields.push(("properties".to_string(), Nbt::List(properties)));
    }
    Ok(Nbt::Compound(fields))
}

fn report_resolved_profile(server: &Server, sender: &CommandSourceStack, message_id: &str, argument: Component, profile: &Profile) {
    let encoded_profile = match encode_stored_game_profile(profile) {
        Ok(encoded_profile) => encoded_profile,
        Err(error) => {
            sender.send_failure(server, Component::translatable_with("commands.fetchprofile.failed_to_serialize", vec![error.into()]));
            return;
        }
    };
    let encoded_profile_as_string = crate::nbt::to_string(&encoded_profile);
    let head_component = Component { contents: Contents::Other(vec![("player".to_string(), encoded_profile)]), style: Style::default(), siblings: Vec::new() };
    let encoded_component_as_string = crate::nbt::to_string(&head_component.to_nbt());
    sender.send_success(
        server,
        || {
            let wrap = |c: Component| Component::translatable_with("chat.square_brackets", vec![c.with_color(ChatFormatting::Green).into()]);
            let elements = [
                Component::translatable("commands.fetchprofile.copy_component").with_click(ClickEvent::CopyToClipboard(encoded_profile_as_string.clone())),
                Component::translatable("commands.fetchprofile.give_item")
                    .with_click(ClickEvent::RunCommand(format!("give @s minecraft:player_head[profile={encoded_profile_as_string}]"))),
                Component::translatable("commands.fetchprofile.summon_mannequin")
                    .with_click(ClickEvent::RunCommand(format!("summon minecraft:mannequin ~ ~ ~ {{profile:{encoded_profile_as_string}}}"))),
                Component::translatable_with("commands.fetchprofile.copy_text", vec![head_component.with_color(ChatFormatting::White).into()])
                    .with_click(ClickEvent::CopyToClipboard(encoded_component_as_string)),
            ];
            let mut clickable = Component::empty();
            for (index, element) in elements.into_iter().enumerate() {
                if index != 0 {
                    clickable = clickable.append(Component::literal(" "));
                }
                clickable = clickable.append(wrap(element));
            }
            Component::translatable_with(message_id, vec![argument.into(), clickable.into()])
        },
        false,
    );
}

pub fn resolve_name(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32> {
    let name = context.get_string("name").to_string();
    let (services, name_to_id_cache, resolver) = (server.services.clone(), server.name_to_id_cache.clone(), server.profile_resolver.clone());
    let source = source.clone();
    resolve_on_io_pool(server, move || {
        let name_component = Component::literal(name.as_str());
        let result = resolver.fetch_by_name(&services, &name_to_id_cache, &name);
        Box::new(move |server: &mut Server| match result {
            Some(profile) => report_resolved_profile(server, &source, "commands.fetchprofile.name.success", name_component, &profile),
            None => source.send_failure(server, Component::translatable_with("commands.fetchprofile.name.failure", vec![name_component.into()])),
        })
    });
    Ok(1)
}

pub fn resolve_id(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32> {
    let id = context.get_uuid("id");
    let (services, resolver) = (server.services.clone(), server.profile_resolver.clone());
    let source = source.clone();
    resolve_on_io_pool(server, move || {
        let id_component = Component::literal(format_uuid(id));
        let result = resolver.fetch_by_id(&services, id);
        Box::new(move |server: &mut Server| match result {
            Some(profile) => report_resolved_profile(server, &source, "commands.fetchprofile.id.success", id_component, &profile),
            None => source.send_failure(server, Component::translatable_with("commands.fetchprofile.id.failure", vec![id_component.into()])),
        })
    });
    Ok(1)
}

fn resolve_on_io_pool(server: &mut Server, task: impl FnOnce() -> Box<dyn FnOnce(&mut Server) + Send + Sync> + Send + 'static) {
    match server.task_sender.clone() {
        Some(sender) => {
            std::thread::spawn(move || {
                let _ = sender.send(task());
            });
        }
        None => task()(server),
    }
}

pub fn print_for_entity(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32> {
    let entity = context.get_entity(server, source, "entity")?;
    if !server.entity_level(entity).is_player(entity) {
        return Err(NO_PROFILE.create(server.entity_display_name(entity)));
    }
    print_for_avatar(server, source, entity);
    Ok(1)
}

pub fn print_for_avatar(server: &Server, source: &CommandSourceStack, avatar: EntityId) {
    let profile = server.player(avatar).profile.clone();
    report_resolved_profile(server, source, "commands.fetchprofile.entity.success", server.entity_display_name(avatar), &profile);
}
