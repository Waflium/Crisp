use crisp_core::{BlockPos, Vec3, mth};

use crate::commands::brigadier::{CommandContext, CommandSyntaxException, SimpleCommandExceptionType, StringReader};
use crate::commands::source::{Anchor, CommandSourceStack};
use crate::server::Server;

pub static ERROR_NOT_COMPLETE: SimpleCommandExceptionType = SimpleCommandExceptionType("argument.pos3d.incomplete");
pub static ERROR_MIXED_TYPE: SimpleCommandExceptionType = SimpleCommandExceptionType("argument.pos.mixed");
pub static ERROR_EXPECTED_DOUBLE: SimpleCommandExceptionType = SimpleCommandExceptionType("argument.pos.missing.double");
pub static ERROR_EXPECTED_INT: SimpleCommandExceptionType = SimpleCommandExceptionType("argument.pos.missing.int");
pub static ERROR_2D_NOT_COMPLETE: SimpleCommandExceptionType = SimpleCommandExceptionType("argument.pos2d.incomplete");
pub static ERROR_ROTATION_NOT_COMPLETE: SimpleCommandExceptionType = SimpleCommandExceptionType("argument.rotation.incomplete");
pub static ERROR_SWIZZLE_INVALID: SimpleCommandExceptionType = SimpleCommandExceptionType("arguments.swizzle.invalid");
pub static ERROR_NOT_LOADED: SimpleCommandExceptionType = SimpleCommandExceptionType("argument.pos.unloaded");
pub static ERROR_OUT_OF_WORLD: SimpleCommandExceptionType = SimpleCommandExceptionType("argument.pos.outofworld");
pub static ERROR_OUT_OF_BOUNDS: SimpleCommandExceptionType = SimpleCommandExceptionType("argument.pos.outofbounds");

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct WorldCoordinate {
    pub relative: bool,
    pub value: f64,
}

impl WorldCoordinate {
    pub fn get(self, original: f64) -> f64 {
        if self.relative { self.value + original } else { self.value }
    }

    const RELATIVE_ZERO: WorldCoordinate = WorldCoordinate { relative: true, value: 0.0 };

    fn is_relative(reader: &mut StringReader) -> bool {
        if reader.peek() == b'~' as u16 {
            reader.skip();
            true
        } else {
            false
        }
    }

    pub fn parse_double(reader: &mut StringReader, center: bool) -> Result<WorldCoordinate, CommandSyntaxException> {
        if reader.at('^') {
            return Err(ERROR_MIXED_TYPE.create_with_context(reader));
        }
        if !reader.can_read() {
            return Err(ERROR_EXPECTED_DOUBLE.create_with_context(reader));
        }
        let relative = Self::is_relative(reader);
        let start = reader.cursor;
        let mut value = if reader.can_read() && reader.peek() != b' ' as u16 { reader.read_double()? } else { 0.0 };
        let number = reader.substring(start, reader.cursor);
        if relative && number.is_empty() {
            return Ok(Self::RELATIVE_ZERO);
        }
        if !number.contains('.') && !relative && center {
            value += 0.5;
        }
        Ok(WorldCoordinate { relative, value })
    }

    pub fn parse_int(reader: &mut StringReader) -> Result<WorldCoordinate, CommandSyntaxException> {
        if reader.at('^') {
            return Err(ERROR_MIXED_TYPE.create_with_context(reader));
        }
        if !reader.can_read() {
            return Err(ERROR_EXPECTED_INT.create_with_context(reader));
        }
        let relative = Self::is_relative(reader);
        let value = if reader.can_read() && reader.peek() != b' ' as u16 {
            if relative { reader.read_double()? } else { reader.read_int()? as f64 }
        } else {
            0.0
        };
        Ok(WorldCoordinate { relative, value })
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Coordinates {
    World { x: WorldCoordinate, y: WorldCoordinate, z: WorldCoordinate },
    Local { left: f64, up: f64, forwards: f64 },
}

impl Coordinates {
    pub fn position(&self, server: &Server, source: &CommandSourceStack) -> Vec3 {
        match *self {
            Coordinates::World { x, y, z } => {
                let pos = source.position;
                Vec3::new(x.get(pos.x), y.get(pos.y), z.get(pos.z))
            }
            Coordinates::Local { left, up, forwards } => {
                let origin = anchor_position(server, source.anchor, source);
                apply_local(origin, source.rotation, left, up, forwards)
            }
        }
    }

    pub fn rotation(&self, source: &CommandSourceStack) -> (f32, f32) {
        match *self {
            Coordinates::World { x, y, .. } => (x.get(source.rotation.0 as f64) as f32, y.get(source.rotation.1 as f64) as f32),
            Coordinates::Local { .. } => (0.0, 0.0),
        }
    }

    pub fn block_pos(&self, server: &Server, source: &CommandSourceStack) -> BlockPos {
        BlockPos::containing_vec(self.position(server, source))
    }

    pub fn is_x_relative(&self) -> bool {
        match self {
            Coordinates::World { x, .. } => x.relative,
            Coordinates::Local { .. } => true,
        }
    }

    pub fn is_y_relative(&self) -> bool {
        match self {
            Coordinates::World { y, .. } => y.relative,
            Coordinates::Local { .. } => true,
        }
    }

    pub fn is_z_relative(&self) -> bool {
        match self {
            Coordinates::World { z, .. } => z.relative,
            Coordinates::Local { .. } => true,
        }
    }
}

pub fn anchor_position(server: &Server, anchor: Anchor, source: &CommandSourceStack) -> Vec3 {
    let pos = source.position;
    match (anchor, source.entity.and_then(|e| server.level.entity_ref(e))) {
        (Anchor::Eyes, Some(entity)) => Vec3::new(pos.x, pos.y + entity.dimensions.eye_height as f64, pos.z),
        _ => pos,
    }
}

pub fn apply_local(source: Vec3, rotation: (f32, f32), left: f64, up: f64, forwards: f64) -> Vec3 {
    const RAD: f32 = (std::f64::consts::PI / 180.0) as f32;
    let y_cos = mth::cos(((rotation.1 + 90.0) * RAD) as f64);
    let y_sin = mth::sin(((rotation.1 + 90.0) * RAD) as f64);
    let x_cos = mth::cos((-rotation.0 * RAD) as f64);
    let x_sin = mth::sin((-rotation.0 * RAD) as f64);
    let x_cos_up = mth::cos(((-rotation.0 + 90.0) * RAD) as f64);
    let x_sin_up = mth::sin(((-rotation.0 + 90.0) * RAD) as f64);
    let fwd = Vec3::new((y_cos * x_cos) as f64, x_sin as f64, (y_sin * x_cos) as f64);
    let upv = Vec3::new((y_cos * x_cos_up) as f64, x_sin_up as f64, (y_sin * x_cos_up) as f64);
    let lft = fwd.cross(upv).scale(-1.0);
    source.add(
        fwd.x * forwards + upv.x * up + lft.x * left,
        fwd.y * forwards + upv.y * up + lft.y * left,
        fwd.z * forwards + upv.z * up + lft.z * left,
    )
}

fn local_double(reader: &mut StringReader, start: usize) -> Result<f64, CommandSyntaxException> {
    if !reader.can_read() {
        return Err(ERROR_EXPECTED_DOUBLE.create_with_context(reader));
    }
    if reader.peek() != b'^' as u16 {
        reader.cursor = start;
        return Err(ERROR_MIXED_TYPE.create_with_context(reader));
    }
    reader.skip();
    if reader.can_read() && reader.peek() != b' ' as u16 { reader.read_double() } else { Ok(0.0) }
}

fn parse_local(reader: &mut StringReader) -> Result<Coordinates, CommandSyntaxException> {
    let start = reader.cursor;
    let left = local_double(reader, start)?;
    if !reader.at(' ') {
        reader.cursor = start;
        return Err(ERROR_NOT_COMPLETE.create_with_context(reader));
    }
    reader.skip();
    let up = local_double(reader, start)?;
    if !reader.at(' ') {
        reader.cursor = start;
        return Err(ERROR_NOT_COMPLETE.create_with_context(reader));
    }
    reader.skip();
    let forwards = local_double(reader, start)?;
    Ok(Coordinates::Local { left, up, forwards })
}

fn parse_world(reader: &mut StringReader, int: bool, center: bool) -> Result<Coordinates, CommandSyntaxException> {
    let start = reader.cursor;
    let read = |reader: &mut StringReader, center: bool| if int { WorldCoordinate::parse_int(reader) } else { WorldCoordinate::parse_double(reader, center) };
    let x = read(reader, center)?;
    if !reader.at(' ') {
        reader.cursor = start;
        return Err(ERROR_NOT_COMPLETE.create_with_context(reader));
    }
    reader.skip();
    let y = read(reader, false)?;
    if !reader.at(' ') {
        reader.cursor = start;
        return Err(ERROR_NOT_COMPLETE.create_with_context(reader));
    }
    reader.skip();
    let z = read(reader, center)?;
    Ok(Coordinates::World { x, y, z })
}

pub fn parse_block_pos(reader: &mut StringReader) -> Result<Coordinates, CommandSyntaxException> {
    if reader.at('^') { parse_local(reader) } else { parse_world(reader, true, false) }
}

pub fn parse_vec3(reader: &mut StringReader, center_correct: bool) -> Result<Coordinates, CommandSyntaxException> {
    if reader.at('^') { parse_local(reader) } else { parse_world(reader, false, center_correct) }
}

fn parse_pair(
    reader: &mut StringReader,
    incomplete: &SimpleCommandExceptionType,
    mut read: impl FnMut(&mut StringReader) -> Result<WorldCoordinate, CommandSyntaxException>,
) -> Result<(WorldCoordinate, WorldCoordinate), CommandSyntaxException> {
    let start = reader.cursor;
    if !reader.can_read() {
        return Err(incomplete.create_with_context(reader));
    }
    let first = read(reader)?;
    if !reader.at(' ') {
        reader.cursor = start;
        return Err(incomplete.create_with_context(reader));
    }
    reader.skip();
    let second = read(reader)?;
    Ok((first, second))
}

pub fn parse_vec2(reader: &mut StringReader, center_correct: bool) -> Result<Coordinates, CommandSyntaxException> {
    let (x, z) = parse_pair(reader, &ERROR_2D_NOT_COMPLETE, |r| WorldCoordinate::parse_double(r, center_correct))?;
    Ok(Coordinates::World { x, y: WorldCoordinate::RELATIVE_ZERO, z })
}

pub fn parse_column_pos(reader: &mut StringReader) -> Result<Coordinates, CommandSyntaxException> {
    let (x, z) = parse_pair(reader, &ERROR_2D_NOT_COMPLETE, WorldCoordinate::parse_int)?;
    Ok(Coordinates::World { x, y: WorldCoordinate::RELATIVE_ZERO, z })
}

pub fn parse_rotation(reader: &mut StringReader) -> Result<Coordinates, CommandSyntaxException> {
    let (y, x) = parse_pair(reader, &ERROR_ROTATION_NOT_COMPLETE, |r| WorldCoordinate::parse_double(r, false))?;
    Ok(Coordinates::World { x, y, z: WorldCoordinate::RELATIVE_ZERO })
}

pub fn parse_swizzle(reader: &mut StringReader) -> Result<u8, CommandSyntaxException> {
    let mut result = 0u8;
    while reader.can_read() && reader.peek() != b' ' as u16 {
        let bit = match reader.read() {
            0x78 => 1,
            0x79 => 2,
            0x7A => 4,
            _ => return Err(ERROR_SWIZZLE_INVALID.create_with_context(reader)),
        };
        if result & bit != 0 {
            return Err(ERROR_SWIZZLE_INVALID.create_with_context(reader));
        }
        result |= bit;
    }
    Ok(result)
}

pub fn get_loaded_block_pos(server: &Server, context: &CommandContext, source: &CommandSourceStack, name: &str) -> Result<BlockPos, CommandSyntaxException> {
    let pos = context.get_coordinates(name).block_pos(server, source);
    if !server.level.has_chunk_at(pos) {
        Err(ERROR_NOT_LOADED.create())
    } else if !server.level.is_in_valid_bounds(pos) {
        Err(ERROR_OUT_OF_WORLD.create())
    } else {
        Ok(pos)
    }
}
