use super::numbers::NumberFormat;
use super::{Arg, Component, Contents, HoverEvent, Style};
use crate::commands::arguments::entity_argument::ERROR_NOT_SINGLE_ENTITY;
use crate::commands::brigadier::CommandSyntaxException;
use crate::commands::source::CommandSourceStack;
use crate::server::Server;
use crate::world::entity::EntityId;
use crate::server::commands::list_players_command::format_list;
use crate::world::scores::score_holder::ScoreHolder;

const DEPTH_LIMIT: i32 = 100;

pub fn merge_styles(mut component: Component, style: &Style) -> Component {
    if style.is_empty() {
        return component;
    }
    if component.style.is_empty() {
        component.style = style.clone();
    } else if component.style != *style {
        component.style = component.style.apply_to(style);
    }
    component
}

pub fn wrap_in_square_brackets(component: Component) -> Component {
    Component::translatable_with("chat.square_brackets", vec![Arg::Component(component)])
}

pub fn resolve(server: &Server, source: &CommandSourceStack, component: &Component) -> Result<Component, CommandSyntaxException> {
    resolve_depth(server, source, source.entity, component, 0)
}

pub fn update_for_entity(server: &Server, source: &CommandSourceStack, component: &Component, entity: Option<EntityId>) -> Result<Component, CommandSyntaxException> {
    resolve_depth(server, source, entity, component, 0)
}

fn resolve_depth(server: &Server, source: &CommandSourceStack, entity: Option<EntityId>, component: &Component, depth: i32) -> Result<Component, CommandSyntaxException> {
    if depth > DEPTH_LIMIT {
        return Ok(component.clone());
    }
    let mut result = resolve_contents(server, source, entity, &component.contents, depth + 1)?;
    for sibling in &component.siblings {
        result.siblings.push(resolve_depth(server, source, entity, sibling, depth + 1)?);
    }
    let style = resolve_style(server, source, entity, &component.style, depth)?;
    Ok(result.with_style(&style))
}

fn resolve_style(server: &Server, source: &CommandSourceStack, entity: Option<EntityId>, style: &Style, depth: i32) -> Result<Style, CommandSyntaxException> {
    if let Some(HoverEvent::ShowText(text)) = &style.hover_event {
        let resolved = resolve_depth(server, source, entity, text, depth + 1)?;
        return Ok(Style { hover_event: Some(HoverEvent::ShowText(Box::new(resolved))), ..style.clone() });
    }
    Ok(style.clone())
}

fn resolve_contents(server: &Server, source: &CommandSourceStack, entity: Option<EntityId>, contents: &Contents, depth: i32) -> Result<Component, CommandSyntaxException> {
    let bare = |contents: Contents| Component { contents, style: Style::default(), siblings: Vec::new() };
    Ok(match contents {
        Contents::Translatable { key, fallback, args } => {
            let mut resolved = Vec::with_capacity(args.len());
            for arg in args {
                resolved.push(match arg {
                    Arg::Component(c) => Arg::Component(resolve_depth(server, source, entity, c, depth)?),
                    other => other.clone(),
                });
            }
            bare(Contents::Translatable { key: key.clone(), fallback: fallback.clone(), args: resolved })
        }
        Contents::Score { name, selector, objective } => {
            let holder = match selector {
                Some(selector) => {
                    let entities = selector.find_entities(server, source)?;
                    match entities.len() {
                        0 => ScoreHolder::for_name_only(name.clone()),
                        1 => ScoreHolder::for_entity(server.entity_level(entities[0]), entities[0]),
                        _ => return Err(ERROR_NOT_SINGLE_ENTITY.create()),
                    }
                }
                None => ScoreHolder::for_name_only(name.clone()),
            };
            let holder = match entity {
                Some(entity) if holder.is_wildcard() => ScoreHolder::for_entity(server.entity_level(entity), entity),
                _ => holder,
            };
            let scoreboard = server.scoreboard.borrow();
            let score = scoreboard
                .get_objective(objective)
                .and_then(|o| scoreboard.get_player_score_info(holder.scoreboard_name(), o.id()).map(|score| score.format_value(&o.number_format_or_default(NumberFormat::no_style()))));
            score.unwrap_or_else(Component::empty)
        }
        Contents::Selector { selector, separator, .. } => {
            let separator = match separator {
                Some(separator) => Some(resolve_depth(server, source, entity, separator, depth)?),
                None => None,
            };
            let names = selector.find_entities(server, source)?.into_iter().map(|e| server.entity_display_name(e)).collect();
            match separator {
                Some(separator) => format_list_with(names, separator),
                None => format_list(names),
            }
        }
        other => bare(other.clone()),
    })
}

fn format_list_with(values: Vec<Component>, separator: Component) -> Component {
    if values.len() == 1 {
        return values.into_iter().next().unwrap();
    }
    let mut result = Component::empty();
    for (i, value) in values.into_iter().enumerate() {
        if i > 0 {
            result = result.append(separator.clone());
        }
        result = result.append(value);
    }
    result
}
