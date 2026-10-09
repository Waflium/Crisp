use std::borrow::Cow;

use crate::commands::CommandFn;
use crate::commands::brigadier::{CommandContext, CommandSyntaxException, Dynamic2CommandExceptionType, Dynamic3CommandExceptionType, DynamicCommandExceptionType, tr};
use crate::commands::source::CommandSourceStack;
use crate::network::chat::{Arg, Component};
use crate::server::Server;
use crate::world::entity::EntityId;
use crate::world::entity::attributes::{Attribute, AttributeInstance, AttributeModifier, Operation};

static ERROR_NOT_LIVING_ENTITY: DynamicCommandExceptionType = DynamicCommandExceptionType(|target| tr("commands.attribute.failed.entity", vec![target]));
static ERROR_NO_SUCH_ATTRIBUTE: Dynamic2CommandExceptionType =
    Dynamic2CommandExceptionType(|target, attribute| tr("commands.attribute.failed.no_attribute", vec![target, attribute]));
static ERROR_NO_SUCH_MODIFIER: Dynamic3CommandExceptionType =
    Dynamic3CommandExceptionType(|target, attribute, modifier| tr("commands.attribute.failed.no_modifier", vec![attribute, target, modifier]));
static ERROR_MODIFIER_ALREADY_PRESENT: Dynamic3CommandExceptionType =
    Dynamic3CommandExceptionType(|target, attribute, modifier| tr("commands.attribute.failed.modifier_already_present", vec![modifier, attribute, target]));

pub fn executors() -> Vec<(String, CommandFn)> {
    let executors: [(&str, CommandFn); 12] = [
        ("get", get_value),
        ("get scale", get_value_scaled),
        ("base set value", set_base),
        ("base get", get_base),
        ("base get scale", get_base_scaled),
        ("base reset", reset_base),
        ("modifier add id value add_value", add_value),
        ("modifier add id value add_multiplied_base", add_multiplied_base),
        ("modifier add id value add_multiplied_total", add_multiplied_total),
        ("modifier remove id", remove_modifier),
        ("modifier value get id", get_modifier),
        ("modifier value get id scale", get_modifier_scaled),
    ];
    executors.into_iter().map(|(path, f)| (format!("attribute target attribute {path}"), f)).collect()
}

fn get_attribute(context: &CommandContext) -> Attribute {
    Attribute::by_name(context.get_identifier("attribute")).expect("parsed attribute")
}

fn description(attribute: Attribute) -> Component {
    Component::translatable(attribute.description_id())
}

fn living_entity(server: &Server, target: EntityId) -> Result<EntityId, CommandSyntaxException> {
    if server.level.is_player(target) { Ok(target) } else { Err(ERROR_NOT_LIVING_ENTITY.create(server.entity_display_name(target))) }
}

fn entity_with_attribute(server: &Server, target: EntityId, attribute: Attribute) -> Result<EntityId, CommandSyntaxException> {
    let entity = living_entity(server, target)?;
    if !server.level.player(entity).living.attributes.has_attribute(attribute) {
        return Err(ERROR_NO_SUCH_ATTRIBUTE.create(server.entity_display_name(target), description(attribute)));
    }
    Ok(entity)
}

fn attribute_instance(server: &mut Server, target: EntityId, attribute: Attribute) -> Result<&mut AttributeInstance, CommandSyntaxException> {
    let entity = living_entity(server, target)?;
    if !server.level.player(entity).living.attributes.has_attribute(attribute) {
        return Err(ERROR_NO_SUCH_ATTRIBUTE.create(server.entity_display_name(target), description(attribute)));
    }
    Ok(server.level.player_mut(entity).living.attributes.instance(attribute).expect("an attribute the entity has"))
}

fn scaled(result: f64, scale: f64) -> i32 {
    (result * scale) as i32
}

fn get_value(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    get_attribute_value(server, context, source, 1.0)
}

fn get_value_scaled(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    get_attribute_value(server, context, source, context.get_double("scale"))
}

fn get_attribute_value(server: &mut Server, context: &CommandContext, source: &CommandSourceStack, scale: f64) -> Result<i32, CommandSyntaxException> {
    let target = context.get_entity(server, source, "target")?;
    let attribute = get_attribute(context);
    let entity = entity_with_attribute(server, target, attribute)?;
    let result = server.level.player(entity).living.attributes.value(attribute);
    let name = server.entity_display_name(target);
    source.send_success(
        server,
        || Component::translatable_with("commands.attribute.value.get.success", vec![description(attribute).into(), name.into(), Arg::Double(result)]),
        false,
    );
    Ok(scaled(result, scale))
}

fn get_base(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    get_attribute_base(server, context, source, 1.0)
}

fn get_base_scaled(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    get_attribute_base(server, context, source, context.get_double("scale"))
}

fn get_attribute_base(server: &mut Server, context: &CommandContext, source: &CommandSourceStack, scale: f64) -> Result<i32, CommandSyntaxException> {
    let target = context.get_entity(server, source, "target")?;
    let attribute = get_attribute(context);
    let entity = entity_with_attribute(server, target, attribute)?;
    let result = server.level.player(entity).living.attributes.base_value(attribute);
    let name = server.entity_display_name(target);
    source.send_success(
        server,
        || Component::translatable_with("commands.attribute.base_value.get.success", vec![description(attribute).into(), name.into(), Arg::Double(result)]),
        false,
    );
    Ok(scaled(result, scale))
}

fn get_modifier(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    get_attribute_modifier(server, context, source, 1.0)
}

fn get_modifier_scaled(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    get_attribute_modifier(server, context, source, context.get_double("scale"))
}

fn get_attribute_modifier(server: &mut Server, context: &CommandContext, source: &CommandSourceStack, scale: f64) -> Result<i32, CommandSyntaxException> {
    let target = context.get_entity(server, source, "target")?;
    let attribute = get_attribute(context);
    let id = context.get_identifier("id");
    let entity = entity_with_attribute(server, target, attribute)?;
    let attributes = &server.level.player(entity).living.attributes;
    let name = server.entity_display_name(target);
    let Some(modifier) = attributes.get(attribute).and_then(|i| i.modifier(id)) else {
        return Err(ERROR_NO_SUCH_MODIFIER.create(name, description(attribute), id));
    };
    let result = modifier.amount;
    source.send_success(
        server,
        || {
            Component::translatable_with(
                "commands.attribute.modifier.value.get.success",
                vec![Component::literal(id).into(), description(attribute).into(), name.into(), Arg::Double(result)],
            )
        },
        false,
    );
    Ok(scaled(result, scale))
}

fn set_base(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let target = context.get_entity(server, source, "target")?;
    let attribute = get_attribute(context);
    let value = context.get_double("value");
    attribute_instance(server, target, attribute)?.set_base_value(value);
    let name = server.entity_display_name(target);
    source.send_success(
        server,
        || Component::translatable_with("commands.attribute.base_value.set.success", vec![description(attribute).into(), name.into(), Arg::Double(value)]),
        false,
    );
    Ok(1)
}

fn reset_base(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let target = context.get_entity(server, source, "target")?;
    let attribute = get_attribute(context);
    let entity = living_entity(server, target)?;
    let name = server.entity_display_name(target);
    let attributes = &mut server.level.player_mut(entity).living.attributes;
    if !attributes.reset_base_value(attribute) {
        return Err(ERROR_NO_SUCH_ATTRIBUTE.create(name, description(attribute)));
    }
    let value = attributes.base_value(attribute);
    source.send_success(
        server,
        || Component::translatable_with("commands.attribute.base_value.reset.success", vec![description(attribute).into(), name.into(), Arg::Double(value)]),
        false,
    );
    Ok(1)
}

fn add_value(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    add_modifier(server, context, source, Operation::AddValue)
}

fn add_multiplied_base(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    add_modifier(server, context, source, Operation::AddMultipliedBase)
}

fn add_multiplied_total(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    add_modifier(server, context, source, Operation::AddMultipliedTotal)
}

fn add_modifier(server: &mut Server, context: &CommandContext, source: &CommandSourceStack, operation: Operation) -> Result<i32, CommandSyntaxException> {
    let target = context.get_entity(server, source, "target")?;
    let attribute = get_attribute(context);
    let id = context.get_identifier("id");
    let amount = context.get_double("value");
    let instance = attribute_instance(server, target, attribute)?;
    if instance.has_modifier(id) {
        return Err(ERROR_MODIFIER_ALREADY_PRESENT.create(server.entity_display_name(target), description(attribute), id));
    }
    instance.add_permanent_modifier(AttributeModifier { id: Cow::Owned(id.to_owned()), amount, operation });
    let name = server.entity_display_name(target);
    source.send_success(
        server,
        || Component::translatable_with("commands.attribute.modifier.add.success", vec![Component::literal(id).into(), description(attribute).into(), name.into()]),
        false,
    );
    Ok(1)
}

fn remove_modifier(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let target = context.get_entity(server, source, "target")?;
    let attribute = get_attribute(context);
    let id = context.get_identifier("id");
    if !attribute_instance(server, target, attribute)?.remove_modifier(id) {
        return Err(ERROR_NO_SUCH_MODIFIER.create(server.entity_display_name(target), description(attribute), id));
    }
    let name = server.entity_display_name(target);
    source.send_success(
        server,
        || Component::translatable_with("commands.attribute.modifier.remove.success", vec![Component::literal(id).into(), description(attribute).into(), name.into()]),
        false,
    );
    Ok(1)
}

pub fn suggest_modifiers(
    server: &Server,
    context: &CommandContext,
    source: &CommandSourceStack,
    mut builder: crate::commands::suggestion::SuggestionsBuilder,
) -> Result<crate::commands::suggestion::Suggestions, CommandSyntaxException> {
    let entity = living_entity(server, context.get_entity(server, source, "target")?)?;
    let attribute = get_attribute(context);
    let ids: Vec<&str> = server.level.player(entity).living.attributes.get(attribute).into_iter().flat_map(|i| i.modifiers().map(|m| m.id.as_ref())).collect();
    crate::commands::shared_suggestion_provider::suggest_resource(ids, &mut builder);
    Ok(builder.build())
}
