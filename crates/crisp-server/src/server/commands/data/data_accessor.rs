use crisp_core::BlockPos;
use crisp_data::commands::CommandNode;
use crisp_protocol::nbt::Nbt;

use crate::commands::arguments::coordinates::get_loaded_block_pos;
use crate::commands::arguments::nbt_path_argument::NbtPath;
use crate::commands::brigadier::{CommandContext, CommandSyntaxException, SimpleCommandExceptionType};
use crate::commands::source::CommandSourceStack;
use crate::nbt::text_component_tag_visitor::to_pretty_component;
use crate::network::chat::{Arg, Component};
use crate::server::Server;
use crate::util::java_format_fixed;
use crate::world::entity::EntityId;

static ERROR_NOT_A_BLOCK_ENTITY: SimpleCommandExceptionType = SimpleCommandExceptionType("commands.data.block.invalid");

static ERROR_NO_PLAYERS: SimpleCommandExceptionType = SimpleCommandExceptionType("commands.data.entity.invalid");

pub enum DataAccessor {
    Entity(EntityId),
    Block(usize, BlockPos),
    Storage(String),
}

pub const KINDS: &[&str] = &["entity", "block", "storage"];

impl DataAccessor {
    pub fn access(server: &mut Server, context: &CommandContext, source: &CommandSourceStack, literal: usize, arg: &str) -> Result<DataAccessor, CommandSyntaxException> {
        Ok(match CommandNode::get(context.nodes[literal].node).name {
            "entity" => DataAccessor::Entity(context.get_entity(server, source, arg)?),
            "storage" => DataAccessor::Storage(context.get_identifier(arg).to_string()),
            "block" => {
                let pos = get_loaded_block_pos(server, context, source, arg)?;
                if server.levels[source.level].get_block_entity(pos).is_none() {
                    return Err(ERROR_NOT_A_BLOCK_ENTITY.create());
                }
                DataAccessor::Block(source.level, pos)
            }
            other => unreachable!("no {other} data accessor"),
        })
    }

    pub fn get_data(&self, server: &mut Server) -> Result<Nbt, CommandSyntaxException> {
        Ok(match self {
            DataAccessor::Entity(entity) => server.entity_level(*entity).entity_tag_to_compare(*entity),
            DataAccessor::Storage(id) => Nbt::Compound(server.command_storage.borrow_mut().get(id)),
            DataAccessor::Block(level, pos) => server.levels[*level].block_entity_data(*pos).ok_or_else(|| ERROR_NOT_A_BLOCK_ENTITY.create())?,
        })
    }

    pub fn set_data(&self, server: &mut Server, tag: Nbt) -> Result<(), CommandSyntaxException> {
        match self {
            DataAccessor::Entity(id) => {
                let level = server.entity_level_mut(*id);
                let Some(entity) = level.entities.get_mut(id) else { return Err(ERROR_NO_PLAYERS.create()) };
                let uuid = entity.entity().uuid;
                if let Err(e) = entity.load(&tag) {
                    error!("{e}");
                }
                entity.entity_mut().uuid = uuid;
                level.entity_moved(*id);
            }
            DataAccessor::Storage(id) => server.command_storage.borrow_mut().set(id, into_fields(tag)),
            DataAccessor::Block(level, pos) => {
                let level = &mut server.levels[*level];
                let Some(entity) = level.get_block_entity(*pos) else { return Err(ERROR_NOT_A_BLOCK_ENTITY.create()) };
                entity.load_with_components(&tag);
                level.block_entity_set_changed(*pos);
                let state = crate::world::level::BlockGetter::block_state(level, *pos);
                level.send_block_updated(*pos, state, state);
            }
        }
        Ok(())
    }

    pub fn write_back(&self, server: &mut Server, tag: &Nbt) {
        match self {
            DataAccessor::Storage(id) => server.command_storage.borrow_mut().write_back(id, into_fields(tag.clone())),
            DataAccessor::Entity(_) | DataAccessor::Block(..) => {}
        }
    }

    fn position_args(pos: BlockPos) -> Vec<Arg> {
        vec![Arg::Int(pos.x), Arg::Int(pos.y), Arg::Int(pos.z)]
    }

    pub fn modified_success(&self, server: &Server) -> Component {
        match self {
            DataAccessor::Entity(entity) => Component::translatable_with("commands.data.entity.modified", vec![server.entity_display_name(*entity).into()]),
            DataAccessor::Storage(id) => Component::translatable_with("commands.data.storage.modified", vec![Arg::Component(Component::literal(id))]),
            DataAccessor::Block(_, pos) => Component::translatable_with("commands.data.block.modified", Self::position_args(*pos)),
        }
    }

    pub fn print_success(&self, server: &Server, data: &Nbt) -> Component {
        match self {
            DataAccessor::Entity(entity) => Component::translatable_with(
                "commands.data.entity.query",
                vec![server.entity_display_name(*entity).into(), Arg::Component(to_pretty_component(data))],
            ),
            DataAccessor::Storage(id) => {
                Component::translatable_with("commands.data.storage.query", vec![Arg::Component(Component::literal(id)), Arg::Component(to_pretty_component(data))])
            }
            DataAccessor::Block(_, pos) => {
                let mut args = Self::position_args(*pos);
                args.push(Arg::Component(to_pretty_component(data)));
                Component::translatable_with("commands.data.block.query", args)
            }
        }
    }

    pub fn print_scaled_success(&self, server: &Server, path: &NbtPath, scale: f64, value: i32) -> Component {
        match self {
            DataAccessor::Entity(entity) => Component::translatable_with(
                "commands.data.entity.get",
                vec![path.original.as_str().into(), server.entity_display_name(*entity).into(), java_format_fixed(scale, 2).into(), value.into()],
            ),
            DataAccessor::Storage(id) => Component::translatable_with(
                "commands.data.storage.get",
                vec![path.original.as_str().into(), Arg::Component(Component::literal(id)), java_format_fixed(scale, 2).into(), value.into()],
            ),
            DataAccessor::Block(_, pos) => {
                let mut args = vec![path.original.as_str().into()];
                args.extend(Self::position_args(*pos));
                args.push(java_format_fixed(scale, 2).into());
                args.push(value.into());
                Component::translatable_with("commands.data.block.get", args)
            }
        }
    }
}

fn into_fields(tag: Nbt) -> Vec<(String, Nbt)> {
    match tag {
        Nbt::Compound(fields) => fields,
        _ => unreachable!("data roots are compounds"),
    }
}

pub fn suggest_storage(
    server: &Server,
    _context: &CommandContext,
    _source: &CommandSourceStack,
    mut builder: crate::commands::suggestion::SuggestionsBuilder,
) -> Result<crate::commands::suggestion::Suggestions, CommandSyntaxException> {
    crate::commands::shared_suggestion_provider::suggest_resource(server.command_storage.borrow_mut().keys(), &mut builder);
    Ok(builder.build())
}
