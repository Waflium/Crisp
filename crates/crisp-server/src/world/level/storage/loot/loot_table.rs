use crisp_core::mth;
use crisp_core::random::RandomSource;
use crisp_protocol::nbt::Nbt;

use super::functions::loot_item_function::{self, LootItemFunction};
use super::loot_context::{LootContext, LootContextBuilder, LootDataType, VisitedEntry};
use super::loot_params::LootParams;
use super::loot_pool::LootPool;
use crate::core::holder::{Holder, RegistryElement};
use crate::server::reloadable_server_registries;
use crate::server::level::ServerLevel;
use crate::util::codec::{self, DataResult};
use crate::world::container::ContainerRef;
use crate::world::item::ItemStack;

pub struct LootTable {
    random_sequence: Option<String>,
    pools: Vec<LootPool>,
    modifier: Option<Holder<LootItemFunction>>,
}

pub static EMPTY: LootTable = LootTable { random_sequence: None, pools: Vec::new(), modifier: None };

impl RegistryElement for LootTable {
    const REGISTRY: &'static str = "minecraft:loot_table";

    fn contains_key(key: &str) -> bool {
        reloadable_server_registries::contains_key(Self::REGISTRY, key)
    }

    fn lookup(key: &str) -> Option<&'static LootTable> {
        reloadable_server_registries::get().loot_tables.get(key)
    }
}

pub fn create_stack_splitter<'a>(output: &mut dyn FnMut(&mut LootContext<'a>, ItemStack)) -> impl FnMut(&mut LootContext<'a>, ItemStack) + '_ {
    move |context, result| {
        if result.count < result.max_stack_size() {
            output(context, result);
        } else {
            let mut count = result.count;
            while count > 0 {
                let copy = result.copy_with_count(result.max_stack_size().min(count));
                count -= copy.count;
                output(context, copy);
            }
        }
    }
}

impl LootTable {
    pub fn decode(tag: &Nbt) -> DataResult<LootTable> {
        codec::map(tag)?;
        Ok(LootTable {
            random_sequence: codec::decode_optional(tag, "random_sequence", codec::identifier)?,
            pools: codec::decode_or(tag, "pools", Vec::new(), |t| codec::list_of(t, LootPool::decode))?,
            modifier: codec::decode_optional(tag, "modifier", LootItemFunction::decode_holder)?,
        })
    }

    pub fn decode_holder(tag: &Nbt) -> DataResult<Holder<LootTable>> {
        Holder::decode(tag, LootTable::decode)
    }

    pub fn decode_list(tag: &Nbt) -> DataResult<Vec<Holder<LootTable>>> {
        codec::compact_list_of(tag, LootTable::decode_holder)
    }

    pub fn get(key: &str) -> &'static LootTable {
        LootTable::lookup(key).unwrap_or(&EMPTY)
    }

    pub fn get_random_items_raw<'a>(&self, context: &mut LootContext<'a>, output: &mut dyn FnMut(&mut LootContext<'a>, ItemStack)) {
        let breadcrumb = VisitedEntry::new(LootDataType::Table, self);
        if context.push_visited_element(breadcrumb) {
            let mut decorated_output = |context: &mut LootContext<'a>, item_stack: ItemStack| loot_item_function::decorate(&self.modifier, item_stack, context, output);
            for pool in &self.pools {
                pool.add_random_items(&mut decorated_output, context);
            }
            context.pop_visited_element(breadcrumb);
        } else {
            warn!("Detected infinite loop in loot tables");
        }
    }

    pub fn get_random_items_with_seed(&self, params: LootParams, optional_loot_table_seed: i64, output: &mut dyn FnMut(&mut ServerLevel, ItemStack)) {
        let mut context = LootContextBuilder::new(params).with_optional_random_seed(optional_loot_table_seed).create(self.random_sequence.as_deref());
        let mut output = |context: &mut LootContext, item_stack: ItemStack| output(context.level(), item_stack);
        self.get_random_items_raw(&mut context, &mut create_stack_splitter(&mut output));
    }

    pub fn get_random_items(&self, params: LootParams) -> Vec<ItemStack> {
        let mut context = LootContextBuilder::new(params).create(self.random_sequence.as_deref());
        self.get_random_items_in(&mut context)
    }

    pub fn get_random_items_with_level_random(&self, params: LootParams) -> Vec<ItemStack> {
        let mut context = LootContextBuilder::new(params).with_level_random_source().create(self.random_sequence.as_deref());
        self.get_random_items_in(&mut context)
    }

    fn get_random_items_in(&self, context: &mut LootContext) -> Vec<ItemStack> {
        let mut result = Vec::new();
        self.get_random_items_raw(context, &mut create_stack_splitter(&mut |_, item_stack| result.push(item_stack)));
        result
    }

    pub fn fill(&self, container: ContainerRef, params: LootParams, optional_random_seed: i64) {
        let mut context = LootContextBuilder::new(params).with_optional_random_seed(optional_random_seed).create(self.random_sequence.as_deref());
        let mut item_stacks = self.get_random_items_in(&mut context);
        let mut available_slots = get_available_slots(&mut context, container);
        shuffle_and_split_items(&mut item_stacks, available_slots.len(), &mut context.random());
        for item_stack in item_stacks {
            let Some(slot) = available_slots.pop() else {
                warn!("Tried to over-fill a container");
                return;
            };
            if item_stack.is_empty() {
                context.level().set_item(container, slot, ItemStack::empty());
            } else {
                context.level().set_item(container, slot, item_stack);
            }
        }
    }
}

fn shuffle_and_split_items(result: &mut Vec<ItemStack>, available_slots: usize, random: &mut impl RandomSource) {
    let mut splittable_items = Vec::new();
    result.retain(|item_stack| {
        if item_stack.is_empty() {
            false
        } else if item_stack.count > 1 {
            splittable_items.push(item_stack.clone());
            false
        } else {
            true
        }
    });
    while available_slots as i64 - result.len() as i64 - splittable_items.len() as i64 > 0 && !splittable_items.is_empty() {
        let mut item_stack: ItemStack = splittable_items.remove(mth::next_int(random, 0, splittable_items.len() as i32 - 1) as usize);
        let remove = mth::next_int(random, 1, item_stack.count / 2);
        let copy = item_stack.split(remove);
        if item_stack.count > 1 && random.next_boolean() {
            splittable_items.push(item_stack);
        } else {
            result.push(item_stack);
        }
        if copy.count > 1 && random.next_boolean() {
            splittable_items.push(copy);
        } else {
            result.push(copy);
        }
    }
    result.append(&mut splittable_items);
    crate::util::shuffle(result, random);
}

fn get_available_slots(context: &mut LootContext, container: ContainerRef) -> Vec<usize> {
    let level = context.level();
    let mut slots: Vec<usize> = (0..level.container_size(container)).filter(|&i| level.get_item(container, i).is_empty()).collect();
    crate::util::shuffle(&mut slots, &mut context.random());
    slots
}

#[cfg(test)]
mod tests {
    use crisp_core::BlockPos;
    use crisp_data::properties::{SLAB_TYPE, SlabType};
    use crisp_protocol::nbt::Nbt;
    use crisp_data::{BlockState, EquipmentSlot, blocks, damage_types, items};

    use super::LootTable;
    use crate::server::level::{ServerLevel, testing};
    use crate::server::reloadable_server_registries;
    use crate::world::item::crafting::tests::vanilla_data_files;
    use crate::world::damagesource::DamageSource;
    use crate::world::entity::animal::chicken::{self, Chicken};
    use crate::world::entity::data::DataValue;
    use crate::world::entity::{EntityId, LevelEntity};
    use crate::world::item::ItemStack;
    use crate::world::item::component as dc;
    use crate::world::level::block;
    use crate::world::level::storage::loot::loot_params::LootParams;

    #[test]
    fn every_vanilla_loot_table_decodes() {
        assert_eq!(reloadable_server_registries::get().loot_tables.len(), vanilla_data_files("loot_table"));
    }

    #[test]
    fn every_vanilla_predicate_decodes() {
        assert_eq!(reloadable_server_registries::get().predicates.len(), vanilla_data_files("predicate"));
    }

    fn drops(level: &mut ServerLevel, state: BlockState, tool: &ItemStack) -> Vec<ItemStack> {
        block::get_drops(state, level, BlockPos::ZERO, None, None, tool)
    }

    fn enchanted(item: crisp_data::Item, enchantments: &str) -> ItemStack {
        let mut stack = ItemStack::new(item, 1);
        let value = dc::ENCHANTMENTS.decode(&crate::nbt::parse_fully(enchantments).unwrap()).unwrap();
        stack.set(dc::ENCHANTMENTS, Some(value));
        stack
    }

    #[test]
    fn grass_block_drops_dirt_by_hand() {
        let mut level = testing::level();
        assert_eq!(drops(&mut level, blocks::GRASS_BLOCK.default_state(), &ItemStack::empty()), [ItemStack::new(items::DIRT, 1)]);
    }

    #[test]
    fn double_slab_drops_two() {
        let mut level = testing::level();
        let double = blocks::OAK_SLAB.default_state().set(SLAB_TYPE, SlabType::Double);
        assert_eq!(drops(&mut level, double, &ItemStack::empty()), [ItemStack::new(items::OAK_SLAB, 2)]);
        assert_eq!(drops(&mut level, blocks::OAK_SLAB.default_state(), &ItemStack::empty()), [ItemStack::new(items::OAK_SLAB, 1)]);
    }

    #[test]
    fn silk_touch_keeps_glass() {
        let mut level = testing::level();
        let state = blocks::GLASS.default_state();
        assert_eq!(drops(&mut level, state, &enchanted(items::DIAMOND_PICKAXE, "{silk_touch:1}")), [ItemStack::new(items::GLASS, 1)]);
        assert_eq!(drops(&mut level, state, &enchanted(items::DIAMOND_PICKAXE, "{efficiency:5}")), []);
    }

    #[test]
    fn fortune_multiplies_ore_drops() {
        let mut level = testing::level();
        let pickaxe = enchanted(items::DIAMOND_PICKAXE, "{fortune:3}");
        let counts: Vec<i32> = (0..200).map(|_| drops(&mut level, blocks::DIAMOND_ORE.default_state(), &pickaxe)[0].count).collect();
        assert!(counts.iter().all(|c| (1..=4).contains(c)));
        assert!(counts.iter().any(|&c| c > 1));
    }

    #[test]
    fn silk_touched_beehive_keeps_its_honey_level() {
        let mut level = testing::level();
        let drops = drops(&mut level, blocks::BEEHIVE.default_state(), &enchanted(items::DIAMOND_PICKAXE, "{silk_touch:1}"));
        assert_eq!(drops.len(), 1);
        assert_eq!(drops[0].get(dc::BLOCK_STATE), Some(&crate::nbt::parse_fully("{honey_level:\"0\"}").unwrap()));
    }

    #[test]
    fn glowstone_drops_two_to_four_dust() {
        let mut level = testing::level();
        for _ in 0..50 {
            let d = drops(&mut level, blocks::GLOWSTONE.default_state(), &ItemStack::empty());
            assert_eq!(d.len(), 1);
            assert_eq!(d[0].item, items::GLOWSTONE_DUST);
            assert!((2..=4).contains(&d[0].count));
        }
    }

    fn chicken(level: &mut ServerLevel, variant: &str) -> EntityId {
        let mut mob = Chicken::create();
        mob.entity.data.set(chicken::DATA_VARIANT, DataValue::ChickenVariant(chicken::variant_id(variant).unwrap()));
        let id = mob.entity.id;
        assert!(level.add_fresh_entity(LevelEntity::Mob(Box::new(mob))));
        id
    }

    fn entity_drops(level: &mut ServerLevel, id: EntityId, attacker: Option<EntityId>) -> Vec<ItemStack> {
        let position = level.entity(id).position();
        let source = DamageSource { causing_entity: attacker, direct_entity: attacker, ..DamageSource::new(damage_types::GENERIC) };
        let mut params = LootParams::new(level);
        params.this_entity = Some(id);
        params.origin = Some(position);
        params.damage_source = Some(source);
        params.attacking_entity = attacker;
        params.direct_attacking_entity = attacker;
        LootTable::get("minecraft:entities/chicken").get_random_items(params)
    }

    #[test]
    fn chickens_lay_the_egg_of_their_variant() {
        let mut level = testing::level();
        for (variant, egg) in [("minecraft:temperate", items::EGG), ("minecraft:warm", items::BROWN_EGG), ("minecraft:cold", items::BLUE_EGG)] {
            let id = chicken(&mut level, variant);
            let mut laid = Vec::new();
            assert!(level.drop_from_gift_loot_table(id, "minecraft:gameplay/chicken_lay", |_, item_stack| laid.push(item_stack)));
            assert_eq!(laid, [ItemStack::new(egg, 1)]);
        }
    }

    #[test]
    fn burning_chickens_drop_cooked_chicken() {
        let mut level = testing::level();
        let id = chicken(&mut level, "minecraft:temperate");
        assert_eq!(entity_drops(&mut level, id, None).last(), Some(&ItemStack::new(items::CHICKEN, 1)));
        level.entity_mut(id).remaining_fire_ticks = 100;
        assert_eq!(entity_drops(&mut level, id, None).last(), Some(&ItemStack::new(items::COOKED_CHICKEN, 1)));
    }

    fn sorted(tag: &Nbt) -> Nbt {
        match tag {
            Nbt::Compound(fields) => {
                let mut fields: Vec<(String, Nbt)> = fields.iter().map(|(k, v)| (k.clone(), sorted(v))).collect();
                fields.sort_by(|a, b| a.0.cmp(&b.0));
                Nbt::Compound(fields)
            }
            Nbt::List(values) => Nbt::List(values.iter().map(sorted).collect()),
            other => other.clone(),
        }
    }

    #[test]
    fn matches_vanilla_loot_command() {
        let mut level = testing::level();
        level.seed = 12345;
        let (mut checked, mut failures) = (0, Vec::new());
        for line in include_str!("loot_oracle.txt").lines() {
            let (label, data) = line.split_once('\t').unwrap();
            let expected = crate::nbt::parse_fully(data).unwrap();
            let words: Vec<&str> = label.split(' ').collect();
            let drops = match words[0] {
                "TABLE" => {
                    let mut params = LootParams::new(&mut level);
                    params.origin = Some(crisp_core::Vec3::new(0.0, 0.0, 0.0));
                    LootTable::get(&format!("minecraft:{}", words[1])).get_random_items(params)
                }
                _ => {
                    let state = crisp_data::Block::by_name(&format!("minecraft:{}", words[1])).unwrap().default_state();
                    let tool = match words[2] {
                        "air" => ItemStack::empty(),
                        tool => {
                            let item = crisp_data::Item::by_name(&format!("minecraft:{}", tool.split('[').next().unwrap())).unwrap();
                            if tool.contains("fortune:3") { enchanted(item, "{fortune:3}") } else { ItemStack::new(item, 1) }
                        }
                    };
                    block::get_drops(state, &mut level, BlockPos::new(1, -60, 0), None, None, &tool)
                }
            };
            let actual = Nbt::List(
                drops
                    .iter()
                    .enumerate()
                    .filter(|(_, stack)| !stack.is_empty())
                    .map(|(slot, stack)| {
                        let Nbt::Compound(mut fields) = stack.to_nbt() else { unreachable!() };
                        fields.push(("Slot".into(), Nbt::Byte(slot as i8)));
                        Nbt::Compound(fields)
                    })
                    .collect(),
            );
            checked += 1;
            if sorted(&actual) != sorted(&expected) {
                failures.push(format!("{label}\n  vanilla: {expected:?}\n  crisp:   {actual:?}"));
            }
        }
        assert!(checked > 0);
        assert!(failures.is_empty(), "{checked} checked, {} failed:\n{}", failures.len(), failures.join("\n"));
    }

    #[test]
    fn looting_adds_to_chicken_drops() {
        let mut level = testing::level();
        let id = chicken(&mut level, "minecraft:temperate");
        let killer = chicken(&mut level, "minecraft:temperate");
        level.mob_mut(killer).unwrap().equipment[EquipmentSlot::Mainhand as usize] = Some(enchanted(items::DIAMOND_SWORD, "{looting:3}"));
        let counts: Vec<i32> = (0..200).map(|_| entity_drops(&mut level, id, Some(killer)).last().unwrap().count).collect();
        assert!(counts.iter().all(|c| (1..=4).contains(c)));
        assert!(counts.iter().any(|&c| c > 1));
    }
}
