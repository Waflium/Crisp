use crisp_core::random::RandomSource;
use crisp_core::{Axis, BlockPos, Direction};
use crisp_data::packets::play::clientbound;
use crisp_data::properties::{NOTE, NOTEBLOCK_INSTRUMENT, NoteBlockInstrument, POWERED};
use crisp_data::{Block, BlockState, item_tags, registry, registry_id};

use super::{BlockBehaviour, BlockHitResult};
use crate::network::connection::encode;
use crate::server::level::{ServerLevel, SoundSource};
use crate::world::entity::EntityId;
use crate::world::item::ItemStack;
use crate::world::item::context::BlockPlaceContext;
use crate::world::level::block::entity::BlockEntityKind;
use crate::world::level::{BlockGetter, LevelAccessor, LevelReader};
use crate::world::{InteractionHand, InteractionResult};

const NOTE_VOLUME: f32 = 3.0;

pub struct NoteBlock {
    pub block: Block,
}

fn is_tunable(instrument: NoteBlockInstrument) -> bool {
    !works_above_note_block(instrument)
}

fn has_custom_sound(instrument: NoteBlockInstrument) -> bool {
    instrument == NoteBlockInstrument::CustomHead
}

fn works_above_note_block(instrument: NoteBlockInstrument) -> bool {
    use NoteBlockInstrument::*;
    matches!(instrument, Zombie | Skeleton | Creeper | Dragon | WitherSkeleton | Piglin | CustomHead)
}

fn sound_event(instrument: NoteBlockInstrument) -> &'static str {
    use NoteBlockInstrument::*;
    match instrument {
        Harp => "minecraft:block.note_block.harp",
        Basedrum => "minecraft:block.note_block.basedrum",
        Snare => "minecraft:block.note_block.snare",
        Hat => "minecraft:block.note_block.hat",
        Bass => "minecraft:block.note_block.bass",
        Flute => "minecraft:block.note_block.flute",
        Bell => "minecraft:block.note_block.bell",
        Guitar => "minecraft:block.note_block.guitar",
        Chime => "minecraft:block.note_block.chime",
        Xylophone => "minecraft:block.note_block.xylophone",
        IronXylophone => "minecraft:block.note_block.iron_xylophone",
        CowBell => "minecraft:block.note_block.cow_bell",
        Didgeridoo => "minecraft:block.note_block.didgeridoo",
        Bit => "minecraft:block.note_block.bit",
        Banjo => "minecraft:block.note_block.banjo",
        Pling => "minecraft:block.note_block.pling",
        Trumpet => "minecraft:block.note_block.trumpet",
        TrumpetExposed => "minecraft:block.note_block.trumpet_exposed",
        TrumpetOxidized => "minecraft:block.note_block.trumpet_oxidized",
        TrumpetWeathered => "minecraft:block.note_block.trumpet_weathered",
        Zombie => "minecraft:block.note_block.imitate.zombie",
        Skeleton => "minecraft:block.note_block.imitate.skeleton",
        Creeper => "minecraft:block.note_block.imitate.creeper",
        Dragon => "minecraft:block.note_block.imitate.ender_dragon",
        WitherSkeleton => "minecraft:block.note_block.imitate.wither_skeleton",
        Piglin => "minecraft:block.note_block.imitate.piglin",
        CustomHead => "minecraft:ui.button.click",
    }
}

pub fn get_pitch_from_note(two_octave_range_note: i32) -> f32 {
    2.0f64.powf((two_octave_range_note - 12) as f64 / 12.0) as f32
}

impl NoteBlock {
    fn set_instrument(&self, level: &mut dyn LevelReader, position: BlockPos, state: BlockState) -> BlockState {
        let instrument_above = level.block_state(position.above()).instrument();
        if works_above_note_block(instrument_above) {
            return state.set(NOTEBLOCK_INSTRUMENT, instrument_above);
        }
        let instrument_below = level.block_state(position.below()).instrument();
        let new_below = if works_above_note_block(instrument_below) { NoteBlockInstrument::Harp } else { instrument_below };
        state.set(NOTEBLOCK_INSTRUMENT, new_below)
    }

    fn play_note(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos) {
        if works_above_note_block(state.get(NOTEBLOCK_INSTRUMENT)) || level.block_state(pos.above()).is_air() {
            level.block_event(pos, self.block, 0, 0);
        }
    }
}

impl BlockBehaviour for NoteBlock {
    fn get_state_for_placement(&self, block: Block, ctx: &BlockPlaceContext, level: &mut ServerLevel) -> Option<BlockState> {
        Some(self.set_instrument(level, ctx.clicked_pos(), block.default_state()))
    }

    fn update_shape(
        &self,
        state: BlockState,
        level: &mut dyn LevelAccessor,
        pos: BlockPos,
        direction: Direction,
        _neighbour_pos: BlockPos,
        _neighbour_state: BlockState,
    ) -> BlockState {
        if direction.axis() == Axis::Y { self.set_instrument(level, pos, state) } else { state }
    }

    fn neighbor_changed(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, _block: Block, _moved_by_piston: bool) {
        let signal = level.has_neighbor_signal(pos);
        if signal != state.get(POWERED) {
            if signal {
                self.play_note(state, level, pos);
            }
            level.set_block_simple(pos, state.set(POWERED, signal));
        }
    }

    fn use_item_on(
        &self,
        item: ItemStack,
        _state: BlockState,
        _level: &mut ServerLevel,
        _pos: BlockPos,
        _player: EntityId,
        _hand: InteractionHand,
        hit: &BlockHitResult,
    ) -> InteractionResult {
        if item.item.is(&item_tags::NOTEBLOCK_TOP_INSTRUMENTS) && hit.direction == Direction::Up {
            InteractionResult::Pass
        } else {
            InteractionResult::TryWithEmptyHand
        }
    }

    fn use_without_item(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, _player: EntityId, _hit: &BlockHitResult) -> InteractionResult {
        let state = state.cycle(NOTE);
        level.set_block_simple(pos, state);
        self.play_note(state, level, pos);
        InteractionResult::SUCCESS
    }

    fn attack(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, _player: EntityId) {
        self.play_note(state, level, pos);
    }

    fn trigger_event(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, _b0: i32, _b1: i32) -> bool {
        let instrument = state.get(NOTEBLOCK_INSTRUMENT);
        let pitch = if is_tunable(instrument) { get_pitch_from_note(state.get(NOTE)) } else { 1.0 };
        if has_custom_sound(instrument) {
            let Some(sound_id) = self.get_custom_sound_id(level, pos) else { return false };
            let seed = level.random.next_long();
            level.play_seeded_direct_sound(None, pos, &sound_id, SoundSource::Records, NOTE_VOLUME, pitch, seed);
            return true;
        }
        let sound = registry_id(registry::SOUND_EVENT, sound_event(instrument));
        let seed = level.random.next_long();
        level.play_seeded_sound(None, pos, sound, SoundSource::Records, NOTE_VOLUME, pitch, seed);
        true
    }
}

impl NoteBlock {
    fn get_custom_sound_id(&self, level: &mut ServerLevel, pos: BlockPos) -> Option<String> {
        match &level.get_block_entity(pos.above())?.kind {
            BlockEntityKind::Skull(head) => head.get_note_block_sound().map(str::to_string),
            _ => None,
        }
    }
}

impl ServerLevel {
    pub fn play_seeded_direct_sound(&mut self, except: Option<EntityId>, pos: BlockPos, sound_id: &str, source: SoundSource, volume: f32, pitch: f32, seed: i64) {
        let (x, y, z) = (pos.x as f64 + 0.5, pos.y as f64 + 0.5, pos.z as f64 + 0.5);
        let packet = encode(clientbound::SOUND, |p| {
            p.varint(0);
            p.string(sound_id);
            p.bool(false);
            p.varint(source as i32);
            p.i32((x * 8.0) as i32);
            p.i32((y * 8.0) as i32);
            p.i32((z * 8.0) as i32);
            p.f32(volume);
            p.f32(pitch);
            p.i64(seed);
        });
        let range = if volume > 1.0 { 16.0 * volume } else { 16.0 } as f64;
        self.broadcast_near(except, x, y, z, range, &packet);
    }

    pub fn play_seeded_sound(&mut self, except: Option<EntityId>, pos: BlockPos, sound: i32, source: SoundSource, volume: f32, pitch: f32, seed: i64) {
        let (x, y, z) = (pos.x as f64 + 0.5, pos.y as f64 + 0.5, pos.z as f64 + 0.5);
        let packet = encode(clientbound::SOUND, |p| {
            p.varint(sound + 1);
            p.varint(source as i32);
            p.i32((x * 8.0) as i32);
            p.i32((y * 8.0) as i32);
            p.i32((z * 8.0) as i32);
            p.f32(volume);
            p.f32(pitch);
            p.i64(seed);
        });
        let range = if volume > 1.0 { 16.0 * volume } else { 16.0 } as f64;
        self.broadcast_near(except, x, y, z, range, &packet);
    }
}
