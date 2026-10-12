use super::stat::Stat;
use super::stat_type::StatType;

pub const BLOCK_MINED: StatType = StatType::Mined;
pub const ITEM_CRAFTED: StatType = StatType::Crafted;
pub const ITEM_USED: StatType = StatType::Used;
pub const ITEM_BROKEN: StatType = StatType::Broken;
pub const ITEM_PICKED_UP: StatType = StatType::PickedUp;
pub const ITEM_DROPPED: StatType = StatType::Dropped;
pub const ENTITY_KILLED: StatType = StatType::Killed;
pub const ENTITY_KILLED_BY: StatType = StatType::KilledBy;
pub const CUSTOM: StatType = StatType::Custom;
pub const LEAVE_GAME: Stat = CUSTOM.get(0);
pub const PLAY_TIME: Stat = CUSTOM.get(1);
pub const TOTAL_WORLD_TIME: Stat = CUSTOM.get(2);
pub const TIME_SINCE_DEATH: Stat = CUSTOM.get(3);
pub const TIME_SINCE_REST: Stat = CUSTOM.get(4);
pub const CROUCH_TIME: Stat = CUSTOM.get(5);
pub const WALK_ONE_CM: Stat = CUSTOM.get(6);
pub const CROUCH_ONE_CM: Stat = CUSTOM.get(7);
pub const SPRINT_ONE_CM: Stat = CUSTOM.get(8);
pub const WALK_ON_WATER_ONE_CM: Stat = CUSTOM.get(9);
pub const FALL_ONE_CM: Stat = CUSTOM.get(10);
pub const CLIMB_ONE_CM: Stat = CUSTOM.get(11);
pub const FLY_ONE_CM: Stat = CUSTOM.get(12);
pub const WALK_UNDER_WATER_ONE_CM: Stat = CUSTOM.get(13);
pub const MINECART_ONE_CM: Stat = CUSTOM.get(14);
pub const BOAT_ONE_CM: Stat = CUSTOM.get(15);
pub const PIG_ONE_CM: Stat = CUSTOM.get(16);
pub const HAPPY_GHAST_ONE_CM: Stat = CUSTOM.get(17);
pub const HORSE_ONE_CM: Stat = CUSTOM.get(18);
pub const AVIATE_ONE_CM: Stat = CUSTOM.get(19);
pub const SWIM_ONE_CM: Stat = CUSTOM.get(20);
pub const STRIDER_ONE_CM: Stat = CUSTOM.get(21);
pub const NAUTILUS_ONE_CM: Stat = CUSTOM.get(22);
pub const JUMP: Stat = CUSTOM.get(23);
pub const DROP: Stat = CUSTOM.get(24);
pub const DAMAGE_DEALT: Stat = CUSTOM.get(25);
pub const DAMAGE_DEALT_ABSORBED: Stat = CUSTOM.get(26);
pub const DAMAGE_DEALT_RESISTED: Stat = CUSTOM.get(27);
pub const DAMAGE_TAKEN: Stat = CUSTOM.get(28);
pub const DAMAGE_BLOCKED_BY_SHIELD: Stat = CUSTOM.get(29);
pub const DAMAGE_ABSORBED: Stat = CUSTOM.get(30);
pub const DAMAGE_RESISTED: Stat = CUSTOM.get(31);
pub const DEATHS: Stat = CUSTOM.get(32);
pub const MOB_KILLS: Stat = CUSTOM.get(33);
pub const ANIMALS_BRED: Stat = CUSTOM.get(34);
pub const PLAYER_KILLS: Stat = CUSTOM.get(35);
pub const FISH_CAUGHT: Stat = CUSTOM.get(36);
pub const TALKED_TO_VILLAGER: Stat = CUSTOM.get(37);
pub const TRADED_WITH_VILLAGER: Stat = CUSTOM.get(38);
pub const EAT_CAKE_SLICE: Stat = CUSTOM.get(39);
pub const FILL_CAULDRON: Stat = CUSTOM.get(40);
pub const USE_CAULDRON: Stat = CUSTOM.get(41);
pub const CLEAN_ARMOR: Stat = CUSTOM.get(42);
pub const CLEAN_BANNER: Stat = CUSTOM.get(43);
pub const CLEAN_SHULKER_BOX: Stat = CUSTOM.get(44);
pub const INTERACT_WITH_BREWINGSTAND: Stat = CUSTOM.get(45);
pub const INTERACT_WITH_BEACON: Stat = CUSTOM.get(46);
pub const INSPECT_DROPPER: Stat = CUSTOM.get(47);
pub const INSPECT_HOPPER: Stat = CUSTOM.get(48);
pub const INSPECT_DISPENSER: Stat = CUSTOM.get(49);
pub const PLAY_NOTEBLOCK: Stat = CUSTOM.get(50);
pub const TUNE_NOTEBLOCK: Stat = CUSTOM.get(51);
pub const POT_FLOWER: Stat = CUSTOM.get(52);
pub const TRIGGER_TRAPPED_CHEST: Stat = CUSTOM.get(53);
pub const OPEN_ENDERCHEST: Stat = CUSTOM.get(54);
pub const ENCHANT_ITEM: Stat = CUSTOM.get(55);
pub const PLAY_RECORD: Stat = CUSTOM.get(56);
pub const INTERACT_WITH_FURNACE: Stat = CUSTOM.get(57);
pub const INTERACT_WITH_CRAFTING_TABLE: Stat = CUSTOM.get(58);
pub const OPEN_CHEST: Stat = CUSTOM.get(59);
pub const SLEEP_IN_BED: Stat = CUSTOM.get(60);
pub const SLEEP_IN_STRAW_BED: Stat = CUSTOM.get(61);
pub const OPEN_SHULKER_BOX: Stat = CUSTOM.get(62);
pub const OPEN_BARREL: Stat = CUSTOM.get(63);
pub const INTERACT_WITH_BLAST_FURNACE: Stat = CUSTOM.get(64);
pub const INTERACT_WITH_SMOKER: Stat = CUSTOM.get(65);
pub const INTERACT_WITH_LECTERN: Stat = CUSTOM.get(66);
pub const INTERACT_WITH_CAMPFIRE: Stat = CUSTOM.get(67);
pub const INTERACT_WITH_CARTOGRAPHY_TABLE: Stat = CUSTOM.get(68);
pub const INTERACT_WITH_LOOM: Stat = CUSTOM.get(69);
pub const INTERACT_WITH_STONECUTTER: Stat = CUSTOM.get(70);
pub const BELL_RING: Stat = CUSTOM.get(71);
pub const RAID_TRIGGER: Stat = CUSTOM.get(72);
pub const RAID_WIN: Stat = CUSTOM.get(73);
pub const INTERACT_WITH_ANVIL: Stat = CUSTOM.get(74);
pub const INTERACT_WITH_GRINDSTONE: Stat = CUSTOM.get(75);
pub const TARGET_HIT: Stat = CUSTOM.get(76);
pub const INTERACT_WITH_SMITHING_TABLE: Stat = CUSTOM.get(77);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn custom_stats_follow_the_registry() {
        let names = [
            (LEAVE_GAME, "minecraft.custom:minecraft.leave_game"),
            (CROUCH_TIME, "minecraft.custom:minecraft.sneak_time"),
            (NAUTILUS_ONE_CM, "minecraft.custom:minecraft.nautilus_one_cm"),
            (JUMP, "minecraft.custom:minecraft.jump"),
            (INTERACT_WITH_BREWINGSTAND, "minecraft.custom:minecraft.interact_with_brewingstand"),
            (TARGET_HIT, "minecraft.custom:minecraft.target_hit"),
            (INTERACT_WITH_SMITHING_TABLE, "minecraft.custom:minecraft.interact_with_smithing_table"),
        ];
        for (stat, name) in names {
            assert_eq!(stat.get_name(), name);
        }
        assert_eq!(crisp_data::registry::CUSTOM_STAT.len(), INTERACT_WITH_SMITHING_TABLE.value as usize + 1);
    }
}
