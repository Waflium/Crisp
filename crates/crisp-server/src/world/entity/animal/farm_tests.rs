use crisp_core::Vec3;
use crisp_data::items;
use crisp_protocol::nbt::Nbt;

use super::cow::cow::{self, Cow};
use super::cow::mushroom_cow::{self, MushroomCow};
use crate::server::level::{ServerLevel, testing};
use crate::world::entity::mob::{Mob, MobKind};
use crate::world::entity::{EntityId, LevelEntity};
use crate::world::item::ItemStack;
use crate::world::item::component as dc;
use crate::world::{InteractionHand, InteractionResult};

fn setup(mut mob: Mob, held: ItemStack) -> (ServerLevel, EntityId, EntityId) {
    let mut level = testing::level();
    let player = testing::player(&mut level, Vec3::new(0.5, -60.0, 0.5), 0.0, 0.0);
    level.player_mut(player).set_item_in_hand(InteractionHand::MainHand, Some(held));
    mob.entity.set_pos(Vec3::new(0.5, -60.0, 2.0));
    let id = mob.entity.id;
    level.add_fresh_entity(LevelEntity::Mob(Box::new(mob)));
    (level, player, id)
}

fn interact(level: &mut ServerLevel, player: EntityId, target: EntityId) -> InteractionResult {
    let location = level.entity(target).position();
    level.interact_on(player, target, InteractionHand::MainHand, location)
}

fn held(level: &ServerLevel, player: EntityId) -> Option<ItemStack> {
    level.player(player).item_in_hand(InteractionHand::MainHand)
}

fn items_on_ground(level: &ServerLevel) -> Vec<(crisp_data::Item, i32)> {
    level.entities.values().filter_map(|e| e.as_item()).map(|item| (item.item.get_item(), item.item.count)).collect()
}

#[test]
fn milking_a_cow_fills_the_bucket() {
    let (mut level, player, cow) = setup(Cow::create(), ItemStack::new(items::BUCKET, 1));
    assert_eq!(interact(&mut level, player, cow), InteractionResult::SUCCESS);
    assert!(held(&level, player).is_some_and(|s| s.is(items::MILK_BUCKET) && s.count == 1));
}

#[test]
fn milking_from_a_stack_keeps_the_rest() {
    let (mut level, player, cow) = setup(Cow::create(), ItemStack::new(items::BUCKET, 3));
    assert_eq!(interact(&mut level, player, cow), InteractionResult::SUCCESS);
    assert!(held(&level, player).is_some_and(|s| s.is(items::BUCKET) && s.count == 2));
    assert!(level.player(player).inventory.slots.iter().flatten().any(|s| s.is(items::MILK_BUCKET)));
}

#[test]
fn baby_cows_give_no_milk() {
    let mut mob = Cow::create();
    mob.set_baby(true);
    let (mut level, player, cow) = setup(mob, ItemStack::new(items::BUCKET, 1));
    assert_eq!(interact(&mut level, player, cow), InteractionResult::Pass);
    assert!(held(&level, player).is_some_and(|s| s.is(items::BUCKET)));
}

#[test]
fn mooshrooms_fill_bowls_with_stew() {
    let (mut level, player, mooshroom) = setup(MushroomCow::create(), ItemStack::new(items::BOWL, 1));
    assert_eq!(interact(&mut level, player, mooshroom), InteractionResult::SUCCESS);
    assert!(held(&level, player).is_some_and(|s| s.is(items::MUSHROOM_STEW)));
}

#[test]
fn brown_mooshrooms_fed_a_flower_give_suspicious_stew() {
    let mut mob = MushroomCow::create();
    mushroom_cow::set_variant(&mut mob.entity, mushroom_cow::Variant::Brown);
    let (mut level, player, mooshroom) = setup(mob, ItemStack::new(items::CORNFLOWER, 2));
    assert_eq!(interact(&mut level, player, mooshroom), InteractionResult::SUCCESS);
    assert!(held(&level, player).is_some_and(|s| s.count == 1));
    assert_eq!(interact(&mut level, player, mooshroom), InteractionResult::SUCCESS);
    assert!(held(&level, player).is_some_and(|s| s.count == 1));
    level.player_mut(player).set_item_in_hand(InteractionHand::MainHand, Some(ItemStack::new(items::BOWL, 1)));
    assert_eq!(interact(&mut level, player, mooshroom), InteractionResult::SUCCESS);
    let stew = held(&level, player).unwrap();
    assert!(stew.is(items::SUSPICIOUS_STEW));
    let effects = stew.get(dc::SUSPICIOUS_STEW_EFFECTS).unwrap().as_list().to_vec();
    assert_eq!(effects.len(), 1);
    assert_eq!(effects[0].get("id").and_then(Nbt::as_str), Some("minecraft:jump_boost"));
    assert_eq!(effects[0].get("duration").and_then(Nbt::as_i64), Some(100));
}

#[test]
fn shearing_a_mooshroom_turns_it_into_a_cow() {
    let (mut level, player, mooshroom) = setup(MushroomCow::create(), ItemStack::new(items::SHEARS, 1));
    level.entity_mut(mooshroom).add_tag("keep");
    assert_eq!(interact(&mut level, player, mooshroom), InteractionResult::SUCCESS);
    assert!(level.entity(mooshroom).is_removed());
    let cow = level.entities.keys().copied().find(|&id| matches!(level.mob(id).map(|m| &m.kind), Some(MobKind::Cow(_)))).unwrap();
    assert_eq!(level.entity(cow).position(), level.entity(mooshroom).position());
    assert!(level.entity(cow).tags.contains(&"keep".to_string()));
    assert_eq!(items_on_ground(&level), vec![(items::RED_MUSHROOM, 1); 5]);
    assert_eq!(held(&level, player).unwrap().damage_value(), 1);
}

#[test]
fn lightning_swaps_a_mooshrooms_variant_once_per_bolt() {
    let (mut level, _player, mooshroom) = setup(MushroomCow::create(), ItemStack::empty());
    let bolt = crate::world::entity::lightning_bolt::LightningBolt::new(Vec3::new(0.5, -60.0, 2.0));
    let bolt_id = bolt.entity.id;
    level.add_fresh_entity(LevelEntity::LightningBolt(bolt));
    level.mushroom_cow_thunder_hit(mooshroom, bolt_id);
    level.mushroom_cow_thunder_hit(mooshroom, bolt_id);
    assert_eq!(mushroom_cow::get_variant(level.entity(mooshroom)), mushroom_cow::Variant::Brown);
    assert_eq!(level.health(mooshroom), 10.0);
}

#[test]
fn tempted_cows_walk_vanillas_path() {
    use crate::world::level::update_flags::UPDATE_CLIENTS;
    let expected: Vec<Vec3> = include_str!("cow_tempt_oracle.txt")
        .lines()
        .map(|line| {
            let v: Vec<f64> = line.split(' ').map(|n| n.parse().unwrap()).collect();
            Vec3::new(v[0], v[1], v[2])
        })
        .collect();
    let mut level = testing::level();
    let fill = |level: &mut ServerLevel, from: (i32, i32, i32), to: (i32, i32, i32), block: crisp_data::Block| {
        for x in from.0..=to.0 {
            for y in from.1..=to.1 {
                for z in from.2..=to.2 {
                    level.set_block(crisp_core::BlockPos::new(x, y, z), block.default_state(), UPDATE_CLIENTS);
                }
            }
        }
    };
    fill(&mut level, (-2, 149, -6), (16, 149, 6), crisp_data::blocks::STONE);
    fill(&mut level, (7, 150, -2), (7, 151, 2), crisp_data::blocks::STONE);
    fill(&mut level, (9, 150, -6), (10, 150, 6), crisp_data::blocks::STONE);
    fill(&mut level, (5, 150, -4), (5, 150, -1), crisp_data::blocks::OAK_FENCE);
    let player = testing::player(&mut level, Vec3::new(12.5, 150.0, 0.5), 0.0, 0.0);
    level.player_mut(player).set_item_in_hand(InteractionHand::MainHand, Some(ItemStack::new(items::WHEAT, 1)));
    let mut mob = Cow::create();
    mob.entity.snap_to(2.5, 150.0, 0.5, 0.0, 0.0);
    mob.entity.y_head_rot = 0.0;
    mob.living.y_body_rot = 0.0;
    let cow = mob.entity.id;
    level.add_fresh_entity(LevelEntity::Mob(Box::new(mob)));
    let mut actual = Vec::new();
    for _ in 0..expected.len() {
        level.tick();
        actual.push(level.entity(cow).position());
    }
    let n = expected.len() - 4;
    let aligned = (0..4).any(|skip| actual[skip..skip + n] == expected[..n] || actual[..n] == expected[skip..skip + n]);
    assert!(aligned, "the cow left vanilla's path: {:?}", &actual[..12]);
}

#[test]
fn cows_save_vanillas_fields() {
    let mut mob = MushroomCow::create();
    mushroom_cow::set_variant(&mut mob.entity, mushroom_cow::Variant::Brown);
    let mut saved = Vec::new();
    mob.save(&mut saved);
    assert!(saved.iter().any(|(k, v)| k == "Type" && v.as_str() == Some("brown")));
    let mut loaded = MushroomCow::create();
    loaded.load(&Nbt::Compound(saved));
    assert_eq!(mushroom_cow::get_variant(&loaded.entity), mushroom_cow::Variant::Brown);
    let mut mob = Cow::create();
    let warm = crate::world::entity::variant::variant_utils::read_variant(cow::VARIANT_REGISTRY, "warm").unwrap();
    cow::set_variant(&mut mob.entity, warm);
    let mut saved = Vec::new();
    mob.save(&mut saved);
    assert!(saved.iter().any(|(k, v)| k == "variant" && v.as_str() == Some("minecraft:warm")));
}
