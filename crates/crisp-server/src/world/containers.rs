use crisp_core::random::RandomSource;
use crisp_core::{BlockPos, Vec3};

use crate::server::level::ServerLevel;
use crate::world::container::ContainerRef;
use crate::world::entity::{ItemEntity, LevelEntity};
use crate::world::item::ItemStack;

pub fn drop_contents(level: &mut ServerLevel, pos: BlockPos, container: ContainerRef) {
    for slot in 0..level.container_size(container) {
        let stack = level.get_item(container, slot);
        level.put_item(container, slot, stack.copy_with_count(0));
        drop_item_stack(level, pos.x as f64, pos.y as f64, pos.z as f64, stack);
    }
}

pub fn drop_item_stack(level: &mut ServerLevel, x: f64, y: f64, z: f64, mut stack: ItemStack) {
    let size = ItemEntity::DIMENSIONS.width as f64;
    let center_range = 1.0 - size;
    let half_size = size / 2.0;
    let random = &mut level.random;
    let xo = x.floor() + random.next_double() * center_range + half_size;
    let yo = y.floor() + random.next_double() * center_range;
    let zo = z.floor() + random.next_double() * center_range + half_size;
    while !stack.is_empty() {
        let random = &mut level.random;
        let piece = stack.split(random.next_int_bounded(21) + 10);
        random.next_double();
        random.next_double();
        let delta = Vec3::new(random.triangle(0.0, 0.11485000171139836), random.triangle(0.2, 0.11485000171139836), random.triangle(0.0, 0.11485000171139836));
        let item = ItemEntity::with_movement(Vec3::new(xo, yo, zo), piece, delta);
        level.add_fresh_entity(LevelEntity::Item(item));
    }
}

pub fn update_neighbours_after_destroy(level: &mut ServerLevel, state: crisp_data::BlockState, pos: BlockPos) {
    level.update_neighbour_for_output_signal(pos, state.block());
}
