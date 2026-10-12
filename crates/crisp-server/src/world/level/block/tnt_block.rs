use crisp_core::{BlockPos, Vec3};
use crisp_data::properties::UNSTABLE;
use crisp_data::{BlockState, blocks, game_rule, items};

use super::BlockBehaviour;
use crate::server::level::game_mode::GameType;
use crate::server::level::{ServerLevel, SoundSource};
use crate::world::entity::sound_event;
use crate::world::entity::primed_tnt::{PrimedTnt, random_short_fuse};
use crate::world::entity::{EntityId, LevelEntity};
use crate::world::item::ItemStack;
use crate::world::level::BlockGetter;
use crate::world::level::block::BlockHitResult;
use crate::world::level::server_explosion::ServerExplosion;
use crate::world::level::update_flags::UPDATE_ALL_IMMEDIATE;
use crate::world::{InteractionHand, InteractionResult};

pub fn prime(level: &mut ServerLevel, pos: BlockPos, source: Option<EntityId>, item: Option<&ItemStack>) -> bool {
    if !level.game_rules.get_bool(game_rule::TNT_EXPLODES) {
        return false;
    }
    if let Some(player) = source.filter(|&s| level.is_player(s))
        && level.player(player).game_mode.game_type == GameType::Adventure
        && !item.is_some_and(|item| item.can_break_block_in_adventure_mode(level.block_state(pos)))
    {
        return false;
    }
    let owner = source.map(|s| level.entity(s).uuid);
    let tnt = PrimedTnt::new(level, Vec3::new(pos.x as f64 + 0.5, pos.y as f64, pos.z as f64 + 0.5), owner);
    let (x, y, z) = (tnt.entity.x(), tnt.entity.y(), tnt.entity.z());
    level.add_fresh_entity(LevelEntity::PrimedTnt(tnt));
    level.play_sound_at(None, x, y, z, sound_event("minecraft:entity.tnt.primed"), SoundSource::Blocks, 1.0, 1.0);
    true
}

pub struct TntBlock;

impl BlockBehaviour for TntBlock {
    fn on_place(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, old_state: BlockState, _moved_by_piston: bool) {
        if !old_state.is_block(state.block()) && level.has_neighbor_signal(pos) && prime(level, pos, None, None) {
            level.remove_block(pos, false);
        }
    }

    fn neighbor_changed(&self, _state: BlockState, level: &mut ServerLevel, pos: BlockPos, _block: crisp_data::Block, _moved_by_piston: bool) {
        if level.has_neighbor_signal(pos) && prime(level, pos, None, None) {
            level.remove_block(pos, false);
        }
    }

    fn player_will_destroy(&self, level: &mut ServerLevel, pos: BlockPos, state: BlockState, player: EntityId) -> BlockState {
        if !level.player(player).abilities.instabuild && state.get(UNSTABLE) {
            prime(level, pos, None, None);
        }
        super::block_player_will_destroy(level, pos, state, player)
    }

    fn was_exploded(&self, level: &mut ServerLevel, pos: BlockPos, explosion: &ServerExplosion) {
        if !level.game_rules.get_bool(game_rule::TNT_EXPLODES) {
            return;
        }
        let owner = explosion.indirect_source.map(|s| level.entity(s).uuid);
        let mut primed = PrimedTnt::new(level, Vec3::new(pos.x as f64 + 0.5, pos.y as f64, pos.z as f64 + 0.5), owner);
        let fuse = random_short_fuse(primed.fuse(), &mut level.random);
        primed.set_fuse(fuse);
        level.add_fresh_entity(LevelEntity::PrimedTnt(primed));
    }

    fn drop_from_explosion(&self) -> bool {
        false
    }

    fn use_item_on(
        &self,
        item: ItemStack,
        _state: BlockState,
        level: &mut ServerLevel,
        pos: BlockPos,
        player: EntityId,
        hand: InteractionHand,
        _hit: &BlockHitResult,
    ) -> InteractionResult {
        if item.item != items::FLINT_AND_STEEL && item.item != items::FIRE_CHARGE {
            return InteractionResult::TryWithEmptyHand;
        }
        if prime(level, pos, Some(player), Some(&item)) {
            level.set_block(pos, blocks::AIR.default_state(), UPDATE_ALL_IMMEDIATE);
            let slot = level.player(player).hand_slot(hand);
            if item.item == items::FLINT_AND_STEEL {
                let equipment = if hand == InteractionHand::MainHand { crisp_data::EquipmentSlot::Mainhand } else { crisp_data::EquipmentSlot::Offhand };
                level.hurt_and_break(player, equipment, 1);
            } else {
                let p = level.player_mut(player);
                if !p.abilities.instabuild
                    && let Some(held) = &mut p.inventory.slots[slot]
                {
                    held.count -= 1;
                    if held.count <= 0 {
                        p.inventory.slots[slot] = None;
                    }
                }
            }
            level.award_stat(player, crate::stats::stats::ITEM_USED.get(item.item.0), 1);
        } else if !level.game_rules.get_bool(game_rule::TNT_EXPLODES) {
            level.send_overlay_message(player, &crate::network::chat::Component::translatable("block.minecraft.tnt.disabled"));
            return InteractionResult::Pass;
        }
        InteractionResult::SUCCESS
    }
}

#[cfg(test)]
mod tests {
    use crisp_core::{BlockPos, Direction, Vec3};
    use crisp_data::{blocks, items};

    use crate::server::level::testing;
    use crate::world::item::ItemStack;
    use crate::world::level::BlockGetter;
    use crate::world::level::block;
    use crate::world::level::update_flags::UPDATE_ALL;

    #[test]
    fn lit_tnt_blows_up_the_ground_and_sets_off_more() {
        let mut level = testing::level();
        let player = testing::player(&mut level, Vec3::new(0.5, -60.0, 3.5), 180.0, 30.0);
        level.handle_player_loaded(player);
        let tnt = BlockPos::new(0, -60, 0);
        let other = BlockPos::new(0, -60, -2);
        level.set_block(tnt, blocks::TNT.default_state(), UPDATE_ALL);
        level.set_block(other, blocks::TNT.default_state(), UPDATE_ALL);
        level.player_mut(player).inventory.slots[36] = Some(ItemStack::new(items::FLINT_AND_STEEL, 1));
        block::click(&mut level, player, tnt, Direction::Up, Vec3::new(0.5, -59.0, 0.5));
        assert!(level.block_state(tnt).is_air());
        for _ in 0..80 {
            level.tick();
        }
        assert!(level.block_state(tnt.below()).is_air());
        assert!(level.block_state(other).is_air());
        assert!(level.player(player).health < 20.0);
        assert_eq!(level.entities.values().filter(|e| matches!(e, crate::world::entity::LevelEntity::PrimedTnt(_))).count(), 1);
    }
}
