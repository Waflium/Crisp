use crisp_core::random::{LegacyRandomSource, RandomSource};
use crisp_core::{BlockPos, mth};

use super::structure_processor::StructureProcessor;
use super::structure_template::Palette;
use crate::world::level::block::mirror::Mirror;
use crate::world::level::block::rotation::Rotation;
use crate::world::level::levelgen::structure::BoundingBox;

pub struct StructurePlaceSettings<'p> {
    pub mirror: Mirror,
    pub rotation: Rotation,
    pub rotation_pivot: BlockPos,
    pub bounding_box: Option<BoundingBox>,
    pub apply_waterlogging: bool,
    pub random: bool,
    pub processors: &'p [StructureProcessor],
    pub known_shape: bool,
}

impl Default for StructurePlaceSettings<'_> {
    fn default() -> Self {
        StructurePlaceSettings {
            mirror: Mirror::None,
            rotation: Rotation::None,
            rotation_pivot: BlockPos::new(0, 0, 0),
            bounding_box: None,
            apply_waterlogging: true,
            random: false,
            processors: &[],
            known_shape: false,
        }
    }
}

pub enum SettingsRandom<'r, R> {
    Given(&'r mut R),
    Positional(LegacyRandomSource),
}

impl<R: RandomSource> RandomSource for SettingsRandom<'_, R> {
    fn set_seed(&mut self, seed: i64) {
        match self {
            SettingsRandom::Given(random) => random.set_seed(seed),
            SettingsRandom::Positional(random) => random.set_seed(seed),
        }
    }

    fn next_int(&mut self) -> i32 {
        match self {
            SettingsRandom::Given(random) => random.next_int(),
            SettingsRandom::Positional(random) => random.next_int(),
        }
    }

    fn next_int_bounded(&mut self, bound: i32) -> i32 {
        match self {
            SettingsRandom::Given(random) => random.next_int_bounded(bound),
            SettingsRandom::Positional(random) => random.next_int_bounded(bound),
        }
    }

    fn next_long(&mut self) -> i64 {
        match self {
            SettingsRandom::Given(random) => random.next_long(),
            SettingsRandom::Positional(random) => random.next_long(),
        }
    }

    fn next_boolean(&mut self) -> bool {
        match self {
            SettingsRandom::Given(random) => random.next_boolean(),
            SettingsRandom::Positional(random) => random.next_boolean(),
        }
    }

    fn next_float(&mut self) -> f32 {
        match self {
            SettingsRandom::Given(random) => random.next_float(),
            SettingsRandom::Positional(random) => random.next_float(),
        }
    }

    fn next_double(&mut self) -> f64 {
        match self {
            SettingsRandom::Given(random) => random.next_double(),
            SettingsRandom::Positional(random) => random.next_double(),
        }
    }

    fn next_gaussian(&mut self) -> f64 {
        match self {
            SettingsRandom::Given(random) => random.next_gaussian(),
            SettingsRandom::Positional(random) => random.next_gaussian(),
        }
    }
}

impl StructurePlaceSettings<'_> {
    pub fn get_random<'r, R: RandomSource>(&self, pos: BlockPos, random: &'r mut R) -> SettingsRandom<'r, R> {
        if self.random {
            SettingsRandom::Given(random)
        } else {
            SettingsRandom::Positional(LegacyRandomSource::new(mth::get_seed(pos.x, pos.y, pos.z)))
        }
    }

    pub fn get_random_palette<'t, R: RandomSource>(&self, palettes: &'t [Palette], pos: BlockPos, random: &mut R) -> &'t Palette {
        &palettes[self.get_random(pos, random).next_int_bounded(palettes.len() as i32) as usize]
    }
}
