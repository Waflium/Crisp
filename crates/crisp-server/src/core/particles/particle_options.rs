use crisp_protocol::Writer;
use crisp_protocol::nbt::Nbt;

use super::particle_types;

#[derive(Clone, Debug, PartialEq)]
pub struct ParticleOptions {
    pub particle_type: &'static str,
    pub options: Nbt,
}

impl ParticleOptions {
    pub fn get_type(&self) -> &'static str {
        self.particle_type
    }

    pub fn write(&self, w: &mut Writer) {
        w.varint(crisp_data::registry_id(crisp_data::registry::PARTICLE_TYPE, self.particle_type));
        let (codec, stream) = particle_types::codec(self.particle_type);
        crate::world::item::component::codec::write(codec, stream, &self.options, w).expect("decoded particle options");
    }
}
