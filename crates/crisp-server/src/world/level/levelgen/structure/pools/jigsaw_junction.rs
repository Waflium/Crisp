use crisp_protocol::nbt::Nbt;

use super::structure_template_pool::Projection;

#[derive(Clone)]
pub struct JigsawJunction {
    pub source_x: i32,
    pub source_ground_y: i32,
    pub source_z: i32,
    pub delta_y: i32,
    pub dest_projection: Projection,
}

impl JigsawJunction {
    pub fn serialize(&self) -> Nbt {
        Nbt::Compound(vec![
            ("source_x".to_owned(), Nbt::Int(self.source_x)),
            ("source_ground_y".to_owned(), Nbt::Int(self.source_ground_y)),
            ("source_z".to_owned(), Nbt::Int(self.source_z)),
            ("delta_y".to_owned(), Nbt::Int(self.delta_y)),
            ("dest_proj".to_owned(), Nbt::String(self.dest_projection.get_name().to_owned())),
        ])
    }

    pub fn deserialize(tag: &Nbt) -> JigsawJunction {
        let int = |key: &str| match tag.get(key) {
            Some(Nbt::Int(v)) => *v,
            _ => 0,
        };
        JigsawJunction {
            source_x: int("source_x"),
            source_ground_y: int("source_ground_y"),
            source_z: int("source_z"),
            delta_y: int("delta_y"),
            dest_projection: tag.get("dest_proj").and_then(Nbt::as_str).and_then(Projection::by_name).unwrap_or(Projection::Rigid),
        }
    }
}
