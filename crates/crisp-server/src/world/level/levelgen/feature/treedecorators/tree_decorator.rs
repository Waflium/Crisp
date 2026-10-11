use crisp_core::BlockPos;
use crisp_core::random::RandomSource;
use crisp_data::properties::BooleanProperty;
use crisp_data::{BlockState, blocks};
use serde_json::Value;

use super::alter_ground_decorator::AlterGroundDecorator;
use super::attached_to_leaves_decorator::AttachedToLeavesDecorator;
use super::attached_to_logs_decorator::AttachedToLogsDecorator;
use super::pale_moss_decorator::PaleMossDecorator;
use super::place_on_ground_decorator::PlaceOnGroundDecorator;
use super::{beehive_decorator, cocoa_decorator, creaking_heart_decorator, leave_vine_decorator, shelf_mushroom_decorator, trunk_vine_decorator};
use crate::server::level::world_gen_region::WorldGenRegion;
use crate::util::java_collections::JavaObjectHashSet;
use crate::util::valueproviders::float;
use crate::world::level::levelgen::feature::tree_feature::TreeSetter;

pub enum TreeDecorator {
    TrunkVine,
    LeaveVine(f32),
    PaleMoss(PaleMossDecorator),
    CreakingHeart(f32),
    Cocoa(f32),
    ShelfMushroom(f32),
    Beehive(f32),
    AlterGround(AlterGroundDecorator),
    AttachedToLeaves(AttachedToLeavesDecorator),
    PlaceOnGround(PlaceOnGroundDecorator),
    AttachedToLogs(AttachedToLogsDecorator),
}

impl TreeDecorator {
    pub fn parse(json: &Value) -> Result<TreeDecorator, String> {
        let ty = json.get("type").and_then(Value::as_str).ok_or("tree decorator without type")?;
        let probability = || float(json.get("probability").ok_or("missing probability")?);
        Ok(match ty.strip_prefix("minecraft:").unwrap_or(ty) {
            "trunk_vine" => TreeDecorator::TrunkVine,
            "leave_vine" => TreeDecorator::LeaveVine(probability()?),
            "pale_moss" => TreeDecorator::PaleMoss(PaleMossDecorator::parse(json)?),
            "creaking_heart" => TreeDecorator::CreakingHeart(probability()?),
            "cocoa" => TreeDecorator::Cocoa(probability()?),
            "shelf_mushroom" => TreeDecorator::ShelfMushroom(probability()?),
            "beehive" => TreeDecorator::Beehive(probability()?),
            "alter_ground" => TreeDecorator::AlterGround(AlterGroundDecorator::parse(json)?),
            "attached_to_leaves" => TreeDecorator::AttachedToLeaves(AttachedToLeavesDecorator::parse(json)?),
            "place_on_ground" => TreeDecorator::PlaceOnGround(PlaceOnGroundDecorator::parse(json)?),
            "attached_to_logs" => TreeDecorator::AttachedToLogs(AttachedToLogsDecorator::parse(json)?),
            other => return Err(format!("unknown tree decorator type {other}")),
        })
    }

    pub fn place<R: RandomSource>(&self, context: &mut Context<R>) {
        match self {
            TreeDecorator::TrunkVine => trunk_vine_decorator::place(context),
            TreeDecorator::LeaveVine(probability) => leave_vine_decorator::place(*probability, context),
            TreeDecorator::PaleMoss(decorator) => decorator.place(context),
            TreeDecorator::CreakingHeart(probability) => creaking_heart_decorator::place(*probability, context),
            TreeDecorator::Cocoa(probability) => cocoa_decorator::place(*probability, context),
            TreeDecorator::ShelfMushroom(probability) => shelf_mushroom_decorator::place(*probability, context),
            TreeDecorator::Beehive(probability) => beehive_decorator::place(*probability, context),
            TreeDecorator::AlterGround(decorator) => decorator.place(context),
            TreeDecorator::AttachedToLeaves(decorator) => decorator.place(context),
            TreeDecorator::PlaceOnGround(decorator) => decorator.place(context),
            TreeDecorator::AttachedToLogs(decorator) => decorator.place(context),
        }
    }
}

pub struct Context<'a, 'l, R> {
    pub level: &'a mut WorldGenRegion<'l>,
    decoration_setter: TreeSetter<'a>,
    pub random: &'a mut R,
    pub logs: Vec<BlockPos>,
    pub leaves: Vec<BlockPos>,
    pub roots: Vec<BlockPos>,
}

impl<'a, 'l, R: RandomSource> Context<'a, 'l, R> {
    pub fn new(
        level: &'a mut WorldGenRegion<'l>,
        decoration_setter: TreeSetter<'a>,
        random: &'a mut R,
        trunk_set: &JavaObjectHashSet<BlockPos>,
        foliage_set: &JavaObjectHashSet<BlockPos>,
        root_set: &JavaObjectHashSet<BlockPos>,
    ) -> Context<'a, 'l, R> {
        let mut roots: Vec<BlockPos> = root_set.iter().collect();
        let mut logs: Vec<BlockPos> = trunk_set.iter().collect();
        let mut leaves: Vec<BlockPos> = foliage_set.iter().collect();
        logs.sort_by_key(|pos| pos.y);
        leaves.sort_by_key(|pos| pos.y);
        roots.sort_by_key(|pos| pos.y);
        Context { level, decoration_setter, random, logs, leaves, roots }
    }

    pub fn place_vine(&mut self, pos: BlockPos, direction: BooleanProperty) {
        self.set_block(pos, blocks::VINE.default_state().set(direction, true));
    }

    pub fn set_block(&mut self, pos: BlockPos, state: BlockState) {
        self.decoration_setter.accept(self.level, pos, state);
    }

    pub fn is_air(&mut self, pos: BlockPos) -> bool {
        self.level.get_block_state(pos).is_air()
    }

    pub fn check_block(&mut self, pos: BlockPos, predicate: impl Fn(BlockState) -> bool) -> bool {
        predicate(self.level.get_block_state(pos))
    }

    pub fn is_replaceable(&mut self, pos: BlockPos) -> bool {
        self.check_block(pos, BlockState::can_be_replaced)
    }

    pub fn is_water_or_water_nearby(&mut self, pos: BlockPos) -> bool {
        let water = |state: BlockState| state.is_block(blocks::WATER);
        self.check_block(pos, water)
            || self.check_block(pos.east(), water)
            || self.check_block(pos.west(), water)
            || self.check_block(pos.north(), water)
            || self.check_block(pos.south(), water)
    }
}
