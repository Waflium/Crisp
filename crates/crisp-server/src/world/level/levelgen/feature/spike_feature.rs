use crisp_core::random::RandomSource;
use crisp_core::{BlockPos, mth};
use crisp_data::BlockState;
use serde_json::Value;

use crate::server::level::world_gen_region::WorldGenRegion;
use crate::world::level::levelgen::blockpredicates::block_predicate::BlockPredicate;
use crate::world::level::levelgen::parse_block_state;

pub struct SpikeFeature {
    state: BlockState,
    can_place_on: BlockPredicate,
    can_replace: BlockPredicate,
}

impl SpikeFeature {
    pub fn parse(json: &Value) -> Result<SpikeFeature, String> {
        let field = |key: &str| json.get(key).ok_or_else(|| format!("missing {key}"));
        Ok(SpikeFeature {
            state: parse_block_state(field("state")?)?,
            can_place_on: BlockPredicate::parse(field("can_place_on")?)?,
            can_replace: BlockPredicate::parse(field("can_replace")?)?,
        })
    }

    pub fn place<R: RandomSource>(&self, level: &mut WorldGenRegion, random: &mut R, mut origin: BlockPos) -> bool {
        while level.get_block_state(origin).is_air() && origin.y > level.get_min_y() + 2 {
            origin = origin.below();
        }
        if !self.can_place_on.test(level, origin) {
            return false;
        }
        origin = origin.offset(0, random.next_int_bounded(4), 0);
        let height = random.next_int_bounded(4) + 7;
        let width = height / 4 + random.next_int_bounded(2);
        if width > 1 && random.next_int_bounded(60) == 0 {
            origin = origin.offset(0, 10 + random.next_int_bounded(30), 0);
        }
        for y_off in 0..height {
            let scale = (1.0 - y_off as f32 / height as f32) * width as f32;
            let new_width = mth::ceil(scale as f64);
            for xo in -new_width..=new_width {
                let dx = xo.abs() as f32 - 0.25;
                for zo in -new_width..=new_width {
                    let dz = zo.abs() as f32 - 0.25;
                    if (xo == 0 && zo == 0 || dx * dx + dz * dz <= scale * scale)
                        && (xo != -new_width && xo != new_width && zo != -new_width && zo != new_width || random.next_float() <= 0.75)
                    {
                        let positive_offset = origin.offset(xo, y_off, zo);
                        if level.get_block_state(positive_offset).is_air() || self.can_replace.test(level, positive_offset) {
                            level.set_block_and_update(positive_offset, self.state);
                        }
                        if y_off != 0 && new_width > 1 {
                            let negative_offset = origin.offset(xo, -y_off, zo);
                            if level.get_block_state(negative_offset).is_air() || self.can_replace.test(level, negative_offset) {
                                level.set_block_and_update(negative_offset, self.state);
                            }
                        }
                    }
                }
            }
        }
        let pillar_width = (width - 1).clamp(0, 1);
        for xo in -pillar_width..=pillar_width {
            for zo in -pillar_width..=pillar_width {
                let mut cursor = origin.offset(xo, -1, zo);
                let mut run_length = 50;
                if xo.abs() == 1 && zo.abs() == 1 {
                    run_length = random.next_int_bounded(5);
                }
                while cursor.y > 50 {
                    let state = level.get_block_state(cursor);
                    if !state.is_air() && !self.can_replace.test(level, cursor) && state != self.state {
                        break;
                    }
                    level.set_block_and_update(cursor, self.state);
                    cursor = cursor.below();
                    run_length -= 1;
                    if run_length <= 0 {
                        cursor = cursor.offset(0, -(random.next_int_bounded(5) + 1), 0);
                        run_length = random.next_int_bounded(5);
                    }
                }
            }
        }
        true
    }
}
