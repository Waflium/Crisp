use crisp_core::hash_map::java_hash_map_order;
use crisp_core::random::{RandomSource, java_string_hash};
use crisp_core::{AABB, Axis, BlockPos, Vec3, mth};
use crisp_protocol::nbt::Nbt;

use super::data::{DataValue, ids};
use super::item::ItemEntity;
use super::primed_tnt::PrimedTnt;
use super::{Entity, EntityId, LevelEntity, MAX_ENTITY_TAG_COUNT, RemovalReason};
use crate::server::level::ServerLevel;
use crate::world::item::ItemStack;
use crate::world::level::block::entity::int_value;
use crate::world::level::chunk::serialization::{state_from_nbt, state_to_nbt};
use crate::world::level::collision::{Collider, shapes_collide};
use crate::world::level::storage::uuid_ints;

pub fn uuid_from_nbt(nbt: &Nbt) -> Option<u128> {
    match nbt {
        Nbt::IntArray(v) if v.len() == 4 => Some(v.iter().fold(0u128, |uuid, &i| uuid << 32 | i as u32 as u128)),
        _ => None,
    }
}

fn number(nbt: &Nbt, key: &str) -> Option<f64> {
    nbt.get(key).and_then(Nbt::as_f64)
}

fn flag(nbt: &Nbt, key: &str) -> bool {
    nbt.get(key).and_then(Nbt::as_i64).is_some_and(|v| v != 0)
}

fn vec3(nbt: &Nbt, key: &str) -> Option<Vec3> {
    match nbt.get(key)?.as_list() {
        [x, y, z] => Some(Vec3::new(x.as_f64()?, y.as_f64()?, z.as_f64()?)),
        _ => None,
    }
}

impl Entity {
    pub fn save_without_id(&self, out: &mut Vec<(String, Nbt)>) {
        let pos = self.position();
        let d = self.delta_movement;
        out.push(("Pos".into(), Nbt::List(vec![Nbt::Double(pos.x), Nbt::Double(pos.y), Nbt::Double(pos.z)])));
        out.push(("Motion".into(), Nbt::List(vec![Nbt::Double(d.x), Nbt::Double(d.y), Nbt::Double(d.z)])));
        out.push(("Rotation".into(), Nbt::List(vec![Nbt::Float(self.y_rot), Nbt::Float(self.x_rot)])));
        out.push(("fall_distance".into(), Nbt::Double(self.fall_distance)));
        out.push(("Fire".into(), Nbt::Short(self.remaining_fire_ticks as i16)));
        out.push(("Air".into(), Nbt::Short(self.air_supply() as i16)));
        out.push(("OnGround".into(), Nbt::Byte(self.on_ground as i8)));
        out.push(("Invulnerable".into(), Nbt::Byte(self.invulnerable as i8)));
        out.push(("PortalCooldown".into(), Nbt::Int(self.portal_cooldown)));
        if self.invulnerable_time > 0 {
            out.push(("invulnerable_time".into(), Nbt::Int(self.invulnerable_time)));
        }
        out.push(("UUID".into(), uuid_ints(self.uuid)));
        if let DataValue::OptionalComponent(Some(name)) = self.data.get(ids::CUSTOM_NAME) {
            out.push(("CustomName".into(), name.clone()));
        }
        if matches!(self.data.get(ids::CUSTOM_NAME_VISIBLE), DataValue::Boolean(true)) {
            out.push(("CustomNameVisible".into(), Nbt::Byte(1)));
        }
        if self.is_silent() {
            out.push(("Silent".into(), Nbt::Byte(1)));
        }
        if self.is_no_gravity() {
            out.push(("NoGravity".into(), Nbt::Byte(1)));
        }
        if self.has_glowing_tag {
            out.push(("Glowing".into(), Nbt::Byte(1)));
        }
        if self.ticks_frozen() > 0 {
            out.push(("TicksFrozen".into(), Nbt::Int(self.ticks_frozen())));
        }
        if self.has_visual_fire {
            out.push(("HasVisualFire".into(), Nbt::Byte(1)));
        }
        if !self.tags.is_empty() {
            let hashes: Vec<i32> = self.tags.iter().map(|t| java_string_hash(t)).collect();
            let tags = java_hash_map_order(&hashes).into_iter().map(|i| Nbt::String(self.tags[i].clone())).collect();
            out.push(("Tags".into(), Nbt::List(tags)));
        }
        if let Some(data) = self.custom_data.as_ref().filter(|d| !matches!(d, Nbt::Compound(f) if f.is_empty())) {
            out.push(("data".into(), data.clone()));
        }
    }

    pub fn load(&mut self, nbt: &Nbt) -> Result<(), String> {
        let pos = vec3(nbt, "Pos").unwrap_or(Vec3::ZERO);
        let motion = vec3(nbt, "Motion").unwrap_or(Vec3::ZERO);
        let (y_rot, x_rot) = match nbt.get("Rotation").map(Nbt::as_list) {
            Some([y, x]) => (y.as_f64().unwrap_or(0.0) as f32, x.as_f64().unwrap_or(0.0) as f32),
            _ => (0.0, 0.0),
        };
        let clamp_motion = |v: f64| if v.abs() > 10.0 { 0.0 } else { v };
        self.delta_movement = Vec3::new(clamp_motion(motion.x), clamp_motion(motion.y), clamp_motion(motion.z));
        self.needs_sync = true;
        let pos = Vec3::new(pos.x.clamp(-3.0000512e7, 3.0000512e7), pos.y.clamp(-2.0e7, 2.0e7), pos.z.clamp(-3.0000512e7, 3.0000512e7));
        if !pos.x.is_finite() || !pos.y.is_finite() || !pos.z.is_finite() {
            return Err("Entity has invalid position".into());
        }
        if !y_rot.is_finite() || !x_rot.is_finite() {
            return Err("Entity has invalid rotation".into());
        }
        self.set_pos(pos);
        self.y_rot = y_rot % 360.0;
        self.x_rot = x_rot.clamp(-90.0, 90.0) % 360.0;
        self.y_head_rot = self.y_rot;
        self.set_old_pos();
        self.fall_distance = number(nbt, "fall_distance").unwrap_or(0.0);
        self.remaining_fire_ticks = number(nbt, "Fire").map_or(0, |v| v as i16 as i32);
        self.set_air_supply(number(nbt, "Air").map_or(super::TOTAL_AIR_SUPPLY, |v| v as i32));
        self.on_ground = flag(nbt, "OnGround");
        self.invulnerable = flag(nbt, "Invulnerable");
        self.invulnerable_time = number(nbt, "invulnerable_time").map_or(0, |v| v as i32);
        self.portal_cooldown = number(nbt, "PortalCooldown").map_or(0, |v| v as i32);
        if let Some(uuid) = nbt.get("UUID").and_then(uuid_from_nbt) {
            self.uuid = uuid;
        }
        self.data.set(ids::CUSTOM_NAME, DataValue::OptionalComponent(nbt.get("CustomName").cloned()));
        self.data.set(ids::CUSTOM_NAME_VISIBLE, DataValue::Boolean(flag(nbt, "CustomNameVisible")));
        self.data.set(ids::SILENT, DataValue::Boolean(flag(nbt, "Silent")));
        self.data.set(ids::NO_GRAVITY, DataValue::Boolean(flag(nbt, "NoGravity")));
        self.has_glowing_tag = flag(nbt, "Glowing");
        self.set_shared_flag(super::flags::GLOWING, self.has_glowing_tag);
        self.set_ticks_frozen(number(nbt, "TicksFrozen").map_or(0, |v| v as i32));
        self.has_visual_fire = flag(nbt, "HasVisualFire");
        self.custom_data = nbt.get("data").cloned();
        self.tags.clear();
        for tag in nbt.get("Tags").map(Nbt::as_list).unwrap_or(&[]).iter().filter_map(Nbt::as_str).take(MAX_ENTITY_TAG_COUNT) {
            if !self.tags.iter().any(|t| t == tag) {
                self.tags.push(tag.to_string());
            }
        }
        Ok(())
    }
}

impl LevelEntity {
    fn encode_id(&self) -> Option<&'static str> {
        match self {
            LevelEntity::Item(_) => Some("minecraft:item"),
            LevelEntity::PrimedTnt(_) => Some("minecraft:tnt"),
            LevelEntity::FallingBlock(_) => Some("minecraft:falling_block"),
            LevelEntity::ExperienceOrb(_) => Some("minecraft:experience_orb"),
            LevelEntity::LightningBolt(_) => None,
            LevelEntity::EyeOfEnder(_) => Some("minecraft:eye_of_ender"),
            LevelEntity::Mob(mob) => Some(crisp_data::registry::ENTITY_TYPE[mob.entity.entity_type as usize]),
        }
    }

    pub fn save(&self) -> Option<Nbt> {
        if self.entity().removal.is_some_and(|r| !r.should_save()) {
            return None;
        }
        let id = self.encode_id()?;
        let mut out = vec![("id".to_string(), Nbt::String(id.into()))];
        self.save_without_id(&mut out);
        Some(Nbt::Compound(out))
    }

    pub fn save_without_id(&self, out: &mut Vec<(String, Nbt)>) {
        self.entity().save_without_id(out);
        match self {
            LevelEntity::Item(item) => {
                out.push(("Health".into(), Nbt::Short(item.health as i16)));
                out.push(("Age".into(), Nbt::Short(item.age as i16)));
                out.push(("PickupDelay".into(), Nbt::Short(item.pickup_delay as i16)));
                if let Some(thrower) = item.thrower {
                    out.push(("Thrower".into(), uuid_ints(thrower)));
                }
                if let Some(owner) = item.target {
                    out.push(("Owner".into(), uuid_ints(owner)));
                }
                if !item.item.is_empty() {
                    out.push(("Item".into(), item.item.to_nbt()));
                }
            }
            LevelEntity::PrimedTnt(tnt) => {
                out.push(("fuse".into(), Nbt::Short(tnt.fuse() as i16)));
                out.push(("block_state".into(), state_to_nbt(tnt.block_state())));
                if tnt.explosion_power != 4.0 {
                    out.push(("explosion_power".into(), Nbt::Float(tnt.explosion_power)));
                }
                if let Some(owner) = tnt.owner {
                    out.push(("owner".into(), uuid_ints(owner)));
                }
            }
            LevelEntity::ExperienceOrb(orb) => {
                out.push(("Health".into(), Nbt::Short(orb.health as i16)));
                out.push(("Age".into(), Nbt::Short(orb.age as i16)));
                out.push(("Value".into(), Nbt::Short(orb.value() as i16)));
                out.push(("Count".into(), Nbt::Int(orb.count)));
            }
            LevelEntity::FallingBlock(falling) => falling.add_additional_save_data(out),
            LevelEntity::LightningBolt(_) => {}
            LevelEntity::EyeOfEnder(eye) => out.push(("Item".into(), eye.get_item().to_nbt())),
            LevelEntity::Mob(mob) => mob.save(out),
        }
    }

    pub fn load(&mut self, nbt: &Nbt) -> Result<(), String> {
        self.entity_mut().load(nbt)?;
        match self {
            LevelEntity::Item(item) => {
                item.health = number(nbt, "Health").map_or(5, |v| v as i16 as i32);
                item.age = number(nbt, "Age").map_or(0, |v| v as i16 as i32);
                item.pickup_delay = number(nbt, "PickupDelay").map_or(0, |v| v as i16 as i32);
                item.target = nbt.get("Owner").and_then(uuid_from_nbt);
                item.thrower = nbt.get("Thrower").and_then(uuid_from_nbt);
                let stack = nbt.get("Item").and_then(|i| ItemStack::from_nbt(i).ok()).unwrap_or(ItemStack::new(crisp_data::items::AIR, 0));
                item.set_item(stack);
                if item.item.is_empty() {
                    item.entity.discard();
                }
            }
            LevelEntity::PrimedTnt(tnt) => {
                tnt.set_fuse(number(nbt, "fuse").map_or(super::primed_tnt::DEFAULT_FUSE_TIME, |v| v as i16 as i32));
                tnt.set_block_state(nbt.get("block_state").and_then(state_from_nbt).unwrap_or(crisp_data::blocks::TNT.default_state()));
                tnt.explosion_power = number(nbt, "explosion_power").map_or(4.0, |v| v as f32).clamp(0.0, 128.0);
                tnt.owner = nbt.get("owner").and_then(uuid_from_nbt);
            }
            LevelEntity::ExperienceOrb(orb) => {
                let short = |key: &str, default: i16| nbt.get(key).and_then(int_value).map_or(default, |v| v as i16) as i32;
                orb.health = short("Health", 5);
                orb.age = short("Age", 0);
                orb.set_value(short("Value", 0));
                orb.count = nbt.get("Count").and_then(int_value).filter(|&c| c > 0).unwrap_or(1);
            }
            LevelEntity::FallingBlock(falling) => falling.read_additional_save_data(nbt),
            LevelEntity::LightningBolt(_) => {}
            LevelEntity::EyeOfEnder(eye) => {
                let item = nbt.get("Item").and_then(|i| ItemStack::from_nbt(i).ok()).unwrap_or_else(ItemStack::empty);
                eye.set_item(&item);
            }
            LevelEntity::Mob(mob) => mob.load(nbt),
        }
        Ok(())
    }
}

fn only_op_can_set_nbt(entity_type: &str) -> bool {
    matches!(entity_type, "minecraft:falling_block" | "minecraft:command_block_minecart" | "minecraft:spawner_minecart")
}

fn update_custom_entity_tag(entity: &mut LevelEntity, entity_data: &Nbt) {
    let Nbt::Compound(fields) = entity_data else { return };
    let entity_type = crisp_data::registry::ENTITY_TYPE[entity.entity().entity_type as usize];
    if fields.iter().find(|(k, _)| k == "id").and_then(|(_, v)| v.as_str()) != Some(entity_type) {
        return;
    }
    if only_op_can_set_nbt(entity_type) {
        return;
    }
    let unsafe_data: Vec<(String, Nbt)> = fields.iter().filter(|(k, _)| k != "id").cloned().collect();
    let mut tag = Vec::new();
    entity.save_without_id(&mut tag);
    let uuid = entity.entity().uuid;
    crate::nbt::merge(&mut tag, &unsafe_data);
    if let Err(e) = entity.load(&Nbt::Compound(tag)) {
        error!("{e}");
    }
    entity.entity_mut().uuid = uuid;
}

impl ServerLevel {
    pub fn create_entity(entity_type: &str) -> Option<LevelEntity> {
        Some(match entity_type {
            "minecraft:item" => LevelEntity::Item(ItemEntity::with_movement(Vec3::ZERO, ItemStack::new(crisp_data::items::AIR, 0), Vec3::ZERO)),
            "minecraft:tnt" => LevelEntity::PrimedTnt(PrimedTnt::of_type()),
            "minecraft:falling_block" => LevelEntity::FallingBlock(super::item::falling_block_entity::FallingBlockEntity::of_type()),
            "minecraft:experience_orb" => LevelEntity::ExperienceOrb(super::experience_orb::ExperienceOrb::of_type()),
            "minecraft:lightning_bolt" => LevelEntity::LightningBolt(super::lightning_bolt::LightningBolt::new(Vec3::ZERO)),
            "minecraft:eye_of_ender" => LevelEntity::EyeOfEnder(super::projectile::eye_of_ender::EyeOfEnder::of_type()),
            "minecraft:chicken" => LevelEntity::Mob(Box::new(super::animal::chicken::Chicken::create())),
            _ => return None,
        })
    }

    fn get_y_offset(&self, spawn_pos: BlockPos, moved_up: bool, entity_box: &AABB) -> f64 {
        let mut aabb = AABB::of_block(spawn_pos);
        if moved_up {
            aabb = aabb.expand_towards(0.0, -1.0, 0.0);
        }
        let shapes: Vec<Collider> = self.block_collisions(&aabb, &crate::world::phys::shapes::collision_context::CollisionContext::EMPTY, false).into_iter().map(|(_, collider)| collider).collect();
        1.0 + shapes_collide(Axis::Y, entity_box, &shapes, if moved_up { -2.0 } else { -1.0 })
    }

    pub fn spawn_entity_type(&mut self, entity_type: &str, spawn_pos: BlockPos) -> Option<EntityId> {
        self.spawn_entity_type_from_stack(entity_type, None, spawn_pos, false, false)
    }

    pub fn spawn_entity_type_from_stack(&mut self, entity_type: &str, item_stack: Option<&ItemStack>, spawn_pos: BlockPos, try_move_down: bool, moved_up: bool) -> Option<EntityId> {
        let mut entity = ServerLevel::create_entity(entity_type)?;
        let y_off = if try_move_down {
            entity.entity_mut().set_pos(Vec3::new(spawn_pos.x as f64 + 0.5, (spawn_pos.y + 1) as f64, spawn_pos.z as f64 + 0.5));
            self.get_y_offset(spawn_pos, moved_up, &entity.entity().bounding_box())
        } else {
            0.0
        };
        let y_rot = mth::wrap_degrees(self.random.next_float() * 360.0);
        entity.entity_mut().snap_to(spawn_pos.x as f64 + 0.5, spawn_pos.y as f64 + y_off, spawn_pos.z as f64 + 0.5, y_rot, 0.0);
        if let LevelEntity::Mob(mob) = &mut entity {
            mob.entity.y_head_rot = mob.entity.y_rot;
            mob.living.y_body_rot = mob.entity.y_rot;
            self.finalize_mob_spawn(mob);
        }
        if let Some(item_stack) = item_stack {
            match &mut entity {
                LevelEntity::Mob(mob) => mob.apply_components_from_item_stack(item_stack),
                other => other.entity_mut().apply_components_from_item_stack(item_stack),
            }
            if let Some(entity_data) = item_stack.get(crate::world::item::component::ENTITY_DATA) {
                update_custom_entity_tag(&mut entity, entity_data);
            }
        }
        let id = entity.entity().id;
        let is_mob = matches!(entity, LevelEntity::Mob(_));
        self.add_fresh_entity(entity);
        if is_mob {
            self.play_ambient_sound(id);
        }
        Some(id)
    }

    pub fn load_entity(&mut self, nbt: &Nbt) -> Option<LevelEntity> {
        let entity_type = nbt.get("id").and_then(Nbt::as_str)?;
        let Some(mut entity) = ServerLevel::create_entity(entity_type) else {
            warn!("Skipping Entity with id {entity_type}");
            return None;
        };
        if let Err(e) = entity.load(nbt) {
            error!("{e}");
            return None;
        }
        Some(entity)
    }

    pub fn entity_tag_to_compare(&self, id: EntityId) -> Nbt {
        if let Some(player) = self.players.get(&id) {
            let Nbt::Compound(mut fields) = player.to_nbt(self.dimension) else { unreachable!() };
            if let Some(selected) = player.inventory.slots[player.inventory.selected_slot()].as_ref().filter(|s| !s.is_empty()) {
                fields.push(("SelectedItem".into(), selected.to_nbt()));
            }
            return Nbt::Compound(fields);
        }
        let mut fields = Vec::new();
        self.entities[&id].save_without_id(&mut fields);
        Nbt::Compound(fields)
    }
}

impl RemovalReason {
    pub fn should_save(self) -> bool {
        matches!(self, RemovalReason::UnloadedToChunk | RemovalReason::UnloadedWithPlayer)
    }

    pub fn should_destroy(self) -> bool {
        matches!(self, RemovalReason::Killed | RemovalReason::Discarded)
    }
}

#[cfg(test)]
mod tests {
    use crisp_core::Vec3;
    use crisp_data::items;

    use super::*;
    use crate::server::level::testing;

    #[test]
    fn items_and_tnt_round_trip() {
        let mut level = testing::level();
        let mut item = ItemEntity::with_movement(Vec3::new(1.5, -60.0, 2.5), ItemStack::new(items::DIAMOND, 3), Vec3::new(0.1, 0.0, 0.0));
        item.pickup_delay = 10;
        item.entity.tags = vec!["b".into(), "a".into()];
        let saved = LevelEntity::Item(item).save().unwrap();
        assert_eq!(saved.get("id").and_then(Nbt::as_str), Some("minecraft:item"));
        let LevelEntity::Item(loaded) = level.load_entity(&saved).unwrap() else { panic!("not an item") };
        assert_eq!((loaded.item.item, loaded.item.count, loaded.pickup_delay), (items::DIAMOND, 3, 10));
        assert_eq!(loaded.entity.position(), Vec3::new(1.5, -60.0, 2.5));
        assert_eq!(LevelEntity::Item(loaded).save(), Some(saved));

        let mut tnt = PrimedTnt::new(&mut level, Vec3::new(0.5, -60.0, 0.5), Some(7));
        tnt.set_fuse(33);
        let saved = LevelEntity::PrimedTnt(tnt).save().unwrap();
        let LevelEntity::PrimedTnt(loaded) = level.load_entity(&saved).unwrap() else { panic!("not TNT") };
        assert_eq!((loaded.fuse(), loaded.owner), (33, Some(7)));
        let bolt = super::super::lightning_bolt::LightningBolt::new(Vec3::ZERO);
        assert_eq!(LevelEntity::LightningBolt(bolt).save(), None);
    }
}
