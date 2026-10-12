use crisp_protocol::nbt::Nbt;

use super::read_identifier;
use crate::commands::brigadier::{CommandSyntaxException, DynamicCommandExceptionType, StringReader, tr};
use crate::core::particles::particle_options::ParticleOptions;
use crate::core::particles::particle_types;

pub static ERROR_UNKNOWN_PARTICLE: DynamicCommandExceptionType = DynamicCommandExceptionType(|value| tr("particle.notFound", vec![value]));
pub static ERROR_INVALID_OPTIONS: DynamicCommandExceptionType = DynamicCommandExceptionType(|message| tr("particle.invalidOptions", vec![message]));

pub fn read_particle(reader: &mut StringReader) -> Result<ParticleOptions, CommandSyntaxException> {
    let particle_type = read_particle_type(reader)?;
    let extra_data = if reader.can_read() && reader.peek() == b'{' as u16 { crate::nbt::parse_as_argument(reader)? } else { Nbt::Compound(Vec::new()) };
    particle_types::parse(particle_type, &extra_data).map_err(|message| ERROR_INVALID_OPTIONS.create(message))
}

fn read_particle_type(reader: &mut StringReader) -> Result<&'static str, CommandSyntaxException> {
    let id = read_identifier(reader)?;
    crisp_data::registry::PARTICLE_TYPE.iter().find(|&&name| name == id).copied().ok_or_else(|| ERROR_UNKNOWN_PARTICLE.create_with_context(reader, id))
}
