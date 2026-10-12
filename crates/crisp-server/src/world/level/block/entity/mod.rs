pub mod abstract_furnace_block_entity;
mod barrel_block_entity;
pub mod base_container_block_entity;
pub mod beehive_block_entity;
pub mod brushable_block_entity;
pub mod chest_block_entity;
pub mod comparator_block_entity;
pub mod container_openers_counter;
mod dispenser_block_entity;
pub mod hopper_block_entity;
#[cfg(test)]
mod oracle_data;
#[cfg(test)]
mod oracle_tests;
pub mod shulker_box_block_entity;
pub mod spawner_block_entity;
#[cfg(test)]
mod tests;

use std::sync::atomic::{AtomicU64, Ordering};

use crisp_core::{BlockPos, ChunkPos};
use crisp_data::{BlockEntityType, BlockState, block_entity_types as types};
use crisp_protocol::nbt::Nbt;

pub use abstract_furnace_block_entity::AbstractFurnaceBlockEntity;
pub use barrel_block_entity::BarrelBlockEntity;
pub use base_container_block_entity::BaseContainerBlockEntity;
pub use beehive_block_entity::BeehiveBlockEntity;
pub use brushable_block_entity::BrushableBlockEntity;
pub use chest_block_entity::ChestBlockEntity;
pub use dispenser_block_entity::DispenserBlockEntity;
pub use hopper_block_entity::HopperBlockEntity;
pub use shulker_box_block_entity::{AnimationStatus, ShulkerBoxBlockEntity};

use crate::world::item::component::{self as dc, DataComponentMap, DataComponentPatch, DataComponentType, PatchedDataComponentMap};
use crate::world::item::ItemStack;
use crate::world::item::crafting::CookingRecipeType;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BlockEntityTicker {
    Furnace,
    Hopper,
    ShulkerBox,
}

enum UpdateTag {
    None,
    CustomOnly,
    WithoutMetadata,
}

#[derive(Clone)]
pub enum BlockEntityKind {
    Chest(ChestBlockEntity),
    Barrel(BarrelBlockEntity),
    ShulkerBox(ShulkerBoxBlockEntity),
    Furnace(AbstractFurnaceBlockEntity),
    Hopper(HopperBlockEntity),
    Dispenser(DispenserBlockEntity),
    Beehive(BeehiveBlockEntity),
    Spawner(spawner_block_entity::SpawnerBlockEntity),
    Brushable(BrushableBlockEntity),
    Comparator(comparator_block_entity::ComparatorBlockEntity),
    Other(Vec<(String, Nbt)>),
}

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Clone)]
pub struct BlockEntity {
    ty: BlockEntityType,
    pos: BlockPos,
    block_state: BlockState,
    remove: bool,
    components: DataComponentMap,
    id: u64,
    pub kind: BlockEntityKind,
}

impl BlockEntity {
    pub fn new(ty: BlockEntityType, pos: BlockPos, state: BlockState) -> Option<BlockEntity> {
        if !ty.is_valid(state) {
            return None;
        }
        let kind = match ty {
            types::CHEST | types::TRAPPED_CHEST => BlockEntityKind::Chest(ChestBlockEntity::default()),
            types::BARREL => BlockEntityKind::Barrel(BarrelBlockEntity::default()),
            types::SHULKER_BOX => BlockEntityKind::ShulkerBox(ShulkerBoxBlockEntity::default()),
            types::FURNACE => BlockEntityKind::Furnace(AbstractFurnaceBlockEntity::new(CookingRecipeType::Smelting)),
            types::BLAST_FURNACE => BlockEntityKind::Furnace(AbstractFurnaceBlockEntity::new(CookingRecipeType::Blasting)),
            types::SMOKER => BlockEntityKind::Furnace(AbstractFurnaceBlockEntity::new(CookingRecipeType::Smoking)),
            types::HOPPER => BlockEntityKind::Hopper(HopperBlockEntity::new(state)),
            types::DISPENSER | types::DROPPER => BlockEntityKind::Dispenser(DispenserBlockEntity::default()),
            types::BEEHIVE => BlockEntityKind::Beehive(BeehiveBlockEntity::default()),
            types::MOB_SPAWNER => BlockEntityKind::Spawner(spawner_block_entity::SpawnerBlockEntity::default()),
            types::BRUSHABLE_BLOCK => BlockEntityKind::Brushable(BrushableBlockEntity::default()),
            types::COMPARATOR => BlockEntityKind::Comparator(comparator_block_entity::ComparatorBlockEntity::default()),
            _ => BlockEntityKind::Other(Vec::new()),
        };
        Some(BlockEntity { ty, pos, block_state: state, remove: false, components: DataComponentMap::EMPTY, id: NEXT_ID.fetch_add(1, Ordering::Relaxed), kind })
    }

    pub fn new_for(pos: BlockPos, state: BlockState) -> Option<BlockEntity> {
        BlockEntity::new(state.block().block_entity_type()?, pos, state)
    }

    pub fn ty(&self) -> BlockEntityType {
        self.ty
    }

    pub fn pos(&self) -> BlockPos {
        self.pos
    }

    pub fn block_state(&self) -> BlockState {
        self.block_state
    }

    pub fn set_block_state(&mut self, state: BlockState) {
        assert!(self.ty.is_valid(state), "Invalid block entity {:?} state at {:?}, got {state:?}", self.ty, self.pos);
        self.block_state = state;
        if let BlockEntityKind::Hopper(hopper) = &mut self.kind {
            hopper.set_block_state(state);
        }
    }

    pub fn id(&self) -> u64 {
        self.id
    }

    pub fn is_removed(&self) -> bool {
        self.remove
    }

    pub fn set_removed(&mut self) {
        self.remove = true;
    }

    pub fn clear_removed(&mut self) {
        self.remove = false;
    }

    pub fn is_valid_block_state(&self, state: BlockState) -> bool {
        self.ty.is_valid(state)
    }

    pub fn get_pos_from_tag(base: ChunkPos, tag: &Nbt) -> BlockPos {
        let int = |key: &str| tag.get(key).and_then(int_value).unwrap_or(0);
        let (mut x, y, mut z) = (int("x"), int("y"), int("z"));
        if x >> 4 != base.x || z >> 4 != base.z {
            warn!("Block entity {} found in a wrong chunk, expected position from chunk {base:?}", crate::nbt::to_string(tag));
            x = base.min_block_x() + (x & 15);
            z = base.min_block_z() + (z & 15);
        }
        BlockPos::new(x, y, z)
    }

    pub fn load_static(pos: BlockPos, state: BlockState, tag: &Nbt) -> Option<BlockEntity> {
        let Some(ty) = tag.get("id").and_then(Nbt::as_str).and_then(crate::commands::arguments::parse_identifier).and_then(|id| BlockEntityType::by_name(&id)) else {
            error!("Skipping block entity with invalid type: {}", tag.get("id").map_or("null".to_string(), crate::nbt::to_string));
            return None;
        };
        let Some(mut entity) = BlockEntity::new(ty, pos, state) else {
            error!("Failed to create block entity {} for block {state:?} at position {pos:?} ", ty.name());
            return None;
        };
        entity.load_with_components(tag);
        Some(entity)
    }

    fn load_additional(&mut self, input: &Nbt) {
        let state = self.block_state;
        match &mut self.kind {
            BlockEntityKind::Chest(chest) => chest.load_additional(input),
            BlockEntityKind::Barrel(barrel) => barrel.load_additional(input),
            BlockEntityKind::ShulkerBox(shulker) => shulker.load_additional(input),
            BlockEntityKind::Furnace(furnace) => furnace.load_additional(input),
            BlockEntityKind::Hopper(hopper) => hopper.load_additional(input),
            BlockEntityKind::Dispenser(dispenser) => dispenser.load_additional(input),
            BlockEntityKind::Beehive(beehive) => beehive.load_additional(input),
            BlockEntityKind::Spawner(spawner) => spawner.load_additional(input),
            BlockEntityKind::Brushable(brushable) => brushable.load_additional(input),
            BlockEntityKind::Comparator(comparator) => comparator.load_additional(input),
            BlockEntityKind::Other(data) => {
                let _ = state;
                *data = match input {
                    Nbt::Compound(fields) => {
                        fields.iter().filter(|(k, _)| !matches!(k.as_str(), "id" | "x" | "y" | "z" | "components" | "keepPacked")).cloned().collect()
                    }
                    _ => Vec::new(),
                };
            }
        }
    }

    fn save_additional(&self, output: &mut Vec<(String, Nbt)>) {
        match &self.kind {
            BlockEntityKind::Chest(chest) => chest.save_additional(output),
            BlockEntityKind::Barrel(barrel) => barrel.save_additional(output),
            BlockEntityKind::ShulkerBox(shulker) => shulker.save_additional(output),
            BlockEntityKind::Furnace(furnace) => furnace.save_additional(output),
            BlockEntityKind::Hopper(hopper) => hopper.save_additional(output),
            BlockEntityKind::Dispenser(dispenser) => dispenser.save_additional(output),
            BlockEntityKind::Beehive(beehive) => beehive.save_additional(output),
            BlockEntityKind::Spawner(spawner) => spawner.save_additional(output),
            BlockEntityKind::Brushable(brushable) => brushable.save_additional(output),
            BlockEntityKind::Comparator(comparator) => comparator.save_additional(output),
            BlockEntityKind::Other(data) => output.extend(data.iter().cloned()),
        }
    }

    pub fn load_with_components(&mut self, input: &Nbt) {
        self.load_additional(input);
        self.components = input.get("components").and_then(|c| decode_component_map(c).ok()).unwrap_or(DataComponentMap::EMPTY);
    }

    pub fn load_custom_only(&mut self, input: &Nbt) {
        self.load_additional(input);
    }

    pub fn save_with_full_metadata(&self) -> Nbt {
        let mut output = self.save_without_metadata_fields();
        self.save_metadata(&mut output);
        Nbt::Compound(output)
    }

    pub fn save_with_id(&self) -> Nbt {
        let mut output = self.save_without_metadata_fields();
        output.push(("id".into(), Nbt::String(self.ty.name().into())));
        Nbt::Compound(output)
    }

    pub fn save_without_metadata(&self) -> Nbt {
        Nbt::Compound(self.save_without_metadata_fields())
    }

    fn save_without_metadata_fields(&self) -> Vec<(String, Nbt)> {
        let mut output = Vec::new();
        self.save_additional(&mut output);
        output.push(("components".into(), encode_component_map(&self.components)));
        output
    }

    pub fn save_custom_only(&self) -> Nbt {
        let mut output = Vec::new();
        self.save_additional(&mut output);
        Nbt::Compound(output)
    }

    fn save_metadata(&self, output: &mut Vec<(String, Nbt)>) {
        output.push(("id".into(), Nbt::String(self.ty.name().into())));
        output.push(("x".into(), Nbt::Int(self.pos.x)));
        output.push(("y".into(), Nbt::Int(self.pos.y)));
        output.push(("z".into(), Nbt::Int(self.pos.z)));
    }

    pub fn get_update_tag(&self) -> Nbt {
        if let BlockEntityKind::Spawner(_) = self.kind {
            let mut tag = self.save_custom_only();
            if let Nbt::Compound(fields) = &mut tag {
                fields.retain(|(key, _)| key != "SpawnPotentials");
            }
            return tag;
        }
        if let BlockEntityKind::Brushable(brushable) = &self.kind {
            return brushable.get_update_tag();
        }
        match self.update_tag_kind() {
            UpdateTag::CustomOnly => self.save_custom_only(),
            UpdateTag::WithoutMetadata => self.save_without_metadata(),
            UpdateTag::None => Nbt::Compound(Vec::new()),
        }
    }

    pub fn has_update_packet(&self) -> bool {
        matches!(self.kind, BlockEntityKind::Spawner(_) | BlockEntityKind::Brushable(_)) || !matches!(self.update_tag_kind(), UpdateTag::None)
    }

    fn update_tag_kind(&self) -> UpdateTag {
        if !matches!(self.kind, BlockEntityKind::Other(_)) {
            return UpdateTag::None;
        }
        match self.ty {
            types::SIGN
            | types::HANGING_SIGN
            | types::BEACON
            | types::CONDUIT
            | types::CREAKING_HEART
            | types::DECORATED_POT
            | types::JIGSAW
            | types::TEST_INSTANCE_BLOCK
            | types::END_GATEWAY => UpdateTag::CustomOnly,
            types::BANNER => UpdateTag::WithoutMetadata,
            _ => UpdateTag::None,
        }
    }

    pub fn apply_components_from_item_stack(&mut self, stack: &ItemStack) {
        self.apply_components(stack.components(), stack.components_patch());
    }

    pub fn apply_components(&mut self, full: &PatchedDataComponentMap, patch: &DataComponentPatch) {
        let mut implicit = vec![dc::BLOCK_ENTITY_DATA, dc::BLOCK_STATE];
        let mut get = |ty: DataComponentType| {
            implicit.push(ty);
            full.get(ty)
        };
        if let Some(base) = self.container_base_mut() {
            base.apply_implicit_components(&mut get);
        }
        let added: Vec<(DataComponentType, Nbt)> =
            patch.iter().filter(|(ty, _)| !implicit.contains(ty)).filter_map(|(ty, value)| value.map(|v| (ty, v.clone()))).collect();
        self.components = DataComponentMap::new(added);
    }

    pub fn collect_components(&self) -> Vec<(DataComponentType, Option<Nbt>)> {
        let mut result: Vec<(DataComponentType, Option<Nbt>)> = self.components.iter().map(|(t, v)| (t, Some(v.clone()))).collect();
        if let Some(base) = self.container_base() {
            base.collect_implicit_components(&mut result, self.is_randomizable());
        }
        result
    }

    pub fn components(&self) -> &DataComponentMap {
        &self.components
    }

    pub fn set_components(&mut self, components: DataComponentMap) {
        self.components = components;
    }

    pub fn remove_components_from_tag(&self, output: &mut Vec<(String, Nbt)>) {
        if self.container_base().is_some() {
            output.retain(|(k, _)| !matches!(k.as_str(), "CustomName" | "lock" | "Items"));
            if self.is_randomizable() {
                output.retain(|(k, _)| !matches!(k.as_str(), "LootTable" | "LootTableSeed"));
            }
        }
    }

    pub fn container_base(&self) -> Option<&BaseContainerBlockEntity> {
        Some(match &self.kind {
            BlockEntityKind::Chest(c) => &c.base,
            BlockEntityKind::Barrel(b) => &b.base,
            BlockEntityKind::ShulkerBox(s) => &s.base,
            BlockEntityKind::Furnace(f) => &f.base,
            BlockEntityKind::Hopper(h) => &h.base,
            BlockEntityKind::Dispenser(d) => &d.base,
            _ => return None,
        })
    }

    pub fn container_base_mut(&mut self) -> Option<&mut BaseContainerBlockEntity> {
        Some(match &mut self.kind {
            BlockEntityKind::Chest(c) => &mut c.base,
            BlockEntityKind::Barrel(b) => &mut b.base,
            BlockEntityKind::ShulkerBox(s) => &mut s.base,
            BlockEntityKind::Furnace(f) => &mut f.base,
            BlockEntityKind::Hopper(h) => &mut h.base,
            BlockEntityKind::Dispenser(d) => &mut d.base,
            _ => return None,
        })
    }

    pub fn is_randomizable(&self) -> bool {
        !matches!(
            self.kind,
            BlockEntityKind::Furnace(_) | BlockEntityKind::Beehive(_) | BlockEntityKind::Spawner(_) | BlockEntityKind::Brushable(_) | BlockEntityKind::Comparator(_) | BlockEntityKind::Other(_)
        )
    }

    pub fn default_name(&self) -> &'static str {
        match self.ty {
            types::CHEST | types::TRAPPED_CHEST => "container.chest",
            types::BARREL => "container.barrel",
            types::SHULKER_BOX => "container.shulkerBox",
            types::FURNACE => "container.furnace",
            types::BLAST_FURNACE => "container.blast_furnace",
            types::SMOKER => "container.smoker",
            types::HOPPER => "container.hopper",
            types::DISPENSER => "container.dispenser",
            types::DROPPER => "container.dropper",
            _ => "",
        }
    }

    pub fn name(&self) -> crate::network::chat::Component {
        match self.container_base().and_then(|b| b.name.as_ref()) {
            Some(name) => crate::world::item::component::values::text(name),
            None => crate::network::chat::Component::translatable(self.default_name()),
        }
    }

    pub fn has_custom_name(&self) -> bool {
        self.container_base().is_some_and(|b| b.name.is_some())
    }

    pub fn ticker(&self, state: BlockState) -> Option<BlockEntityTicker> {
        let block = state.block();
        match self.ty {
            types::FURNACE | types::BLAST_FURNACE | types::SMOKER if block.is_instance_of("AbstractFurnaceBlock") => Some(BlockEntityTicker::Furnace),
            types::HOPPER if block.is_instance_of("HopperBlock") => Some(BlockEntityTicker::Hopper),
            types::SHULKER_BOX if block.is_instance_of("ShulkerBoxBlock") => Some(BlockEntityTicker::ShulkerBox),
            _ => None,
        }
    }

    pub fn trigger_event(&mut self, b0: i32, b1: i32) -> bool {
        match &mut self.kind {
            BlockEntityKind::Chest(chest) => chest.trigger_event(b0, b1),
            BlockEntityKind::ShulkerBox(shulker) => shulker.trigger_event(b0, b1),
            _ => false,
        }
    }
}

pub fn int_value(tag: &Nbt) -> Option<i32> {
    Some(match *tag {
        Nbt::Byte(v) => v as i32,
        Nbt::Short(v) => v as i32,
        Nbt::Int(v) => v,
        Nbt::Long(v) => v as i32,
        Nbt::Float(v) => v as i32,
        Nbt::Double(v) => v as i32,
        _ => return None,
    })
}

pub fn float_value(tag: &Nbt) -> Option<f32> {
    Some(match *tag {
        Nbt::Float(v) => v,
        Nbt::Double(v) => v as f32,
        _ => tag.as_i64()? as f32,
    })
}

pub fn long_value(tag: &Nbt) -> Option<i64> {
    Some(match *tag {
        Nbt::Float(v) => v as i64,
        Nbt::Double(v) => v as i64,
        _ => tag.as_i64()?,
    })
}

pub fn int_or(input: &Nbt, key: &str, default: i32) -> i32 {
    input.get(key).and_then(int_value).unwrap_or(default)
}

pub fn decode_component_map(tag: &Nbt) -> Result<DataComponentMap, String> {
    let Nbt::Compound(fields) = tag else { return Err("Not a map".into()) };
    let mut entries = Vec::new();
    for (key, value) in fields {
        let id = crate::commands::arguments::parse_identifier(key).ok_or_else(|| format!("Not a valid resource location: {key}"))?;
        let ty = DataComponentType::by_name(&id).ok_or_else(|| format!("No component with type: {id}"))?;
        if ty.is_transient() {
            return Err(format!("Encountered transient component {id}"));
        }
        entries.push((ty, ty.decode(value)?));
    }
    Ok(DataComponentMap::new(entries))
}

pub fn encode_component_map(map: &DataComponentMap) -> Nbt {
    Nbt::Compound(map.iter().filter(|(ty, _)| !ty.is_transient()).map(|(ty, value)| (ty.name().to_string(), value.clone())).collect())
}
