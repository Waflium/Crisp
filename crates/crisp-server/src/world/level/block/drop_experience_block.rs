use crisp_core::BlockPos;
use crisp_core::random::RandomSource;
use crisp_data::{Block, BlockState};

use super::BlockBehaviour;
use crate::server::level::ServerLevel;
use crate::world::item::ItemStack;
use crate::world::item::enchantment::enchantment_helper;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum XpRange {
    Constant(i32),
    Uniform { min_inclusive: i32, max_inclusive: i32 },
}

impl XpRange {
    pub fn of(block: Block) -> XpRange {
        let fields: serde_json::Value = serde_json::from_str(block.info().fields).unwrap_or_default();
        let range = &fields["xpRange"];
        let int = |key: &str| range[key].as_i64().map(|v| v as i32);
        match (int("minInclusive"), int("maxInclusive")) {
            (Some(min_inclusive), Some(max_inclusive)) => XpRange::Uniform { min_inclusive, max_inclusive },
            _ => XpRange::Constant(int("value").unwrap_or(0)),
        }
    }

    pub fn sample(self, random: &mut impl RandomSource) -> i32 {
        match self {
            XpRange::Constant(value) => value,
            XpRange::Uniform { min_inclusive, max_inclusive } => crisp_core::mth::random_between_inclusive(random, min_inclusive, max_inclusive),
        }
    }
}

impl ServerLevel {
    pub fn try_drop_experience(&mut self, pos: BlockPos, tool: Option<&ItemStack>, xp_range: XpRange) {
        let amount = xp_range.sample(&mut self.random);
        let experience = enchantment_helper::process_block_experience(&mut self.random, tool, amount);
        if experience > 0 {
            self.pop_experience(pos, experience);
        }
    }
}

pub struct DropExperienceBlock {
    xp_range: XpRange,
}

impl DropExperienceBlock {
    pub fn new(block: Block) -> DropExperienceBlock {
        DropExperienceBlock { xp_range: XpRange::of(block) }
    }
}

impl BlockBehaviour for DropExperienceBlock {
    fn spawn_after_break(&self, _state: BlockState, level: &mut ServerLevel, pos: BlockPos, tool: Option<ItemStack>, drop_experience: bool) {
        if drop_experience {
            level.try_drop_experience(pos, tool.as_ref(), self.xp_range);
        }
    }
}
