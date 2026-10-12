use crisp_core::BlockPos;
use crisp_protocol::nbt::Nbt;

use super::map_decoration_type::MapDecorationTypeHolder;
use super::map_decoration_types::WHITE_BANNER;
use crate::server::level::ServerLevel;
use crate::world::item::dye_color::DyeColor;
use crate::world::level::block::abstract_banner_block;
use crate::world::level::block::entity::BlockEntityKind;

#[derive(Clone, PartialEq, Debug)]
pub struct MapBanner {
    pub pos: BlockPos,
    pub color: DyeColor,
    pub name: Option<Nbt>,
}

impl MapBanner {
    pub fn decode(tag: &Nbt) -> Option<MapBanner> {
        let Some(Nbt::IntArray(pos)) = tag.get("pos") else { return None };
        let [x, y, z] = pos[..] else { return None };
        let color = tag.get("color").and_then(Nbt::as_str).and_then(DyeColor::by_name).unwrap_or(DyeColor::VALUES[0]);
        Some(MapBanner { pos: BlockPos::new(x, y, z), color, name: tag.get("name").cloned() })
    }

    pub fn encode(&self) -> Nbt {
        let mut fields = vec![("pos".into(), Nbt::IntArray(vec![self.pos.x, self.pos.y, self.pos.z]))];
        if self.color != DyeColor::VALUES[0] {
            fields.push(("color".into(), Nbt::String(self.color.name.into())));
        }
        if let Some(name) = &self.name {
            fields.push(("name".into(), name.clone()));
        }
        Nbt::Compound(fields)
    }

    pub fn from_world(level: &ServerLevel, pos: BlockPos) -> Option<MapBanner> {
        let block_entity = level.chunks.block_entity_ref(pos)?;
        let BlockEntityKind::Banner(banner) = &block_entity.kind else { return None };
        let color = abstract_banner_block::get_color(block_entity.block_state().block());
        Some(MapBanner { pos, color, name: banner.get_custom_name().cloned() })
    }

    pub fn get_decoration(&self) -> MapDecorationTypeHolder {
        MapDecorationTypeHolder(WHITE_BANNER.0 + self.color.id as u16)
    }

    pub fn get_id(&self) -> String {
        format!("banner-{},{},{}", self.pos.x, self.pos.y, self.pos.z)
    }
}
