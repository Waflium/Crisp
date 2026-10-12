use crisp_protocol::nbt::Nbt;

use crate::commands::arguments::nbt_path_argument::NbtPath;
use crate::util::codec::{self, DataResult};
use crate::world::level::storage::loot::loot_context::LootContext;

pub struct StoredNumberAccess {
    storage: String,
    path: NbtPath,
}

impl StoredNumberAccess {
    pub fn decode(tag: &Nbt) -> DataResult<StoredNumberAccess> {
        let path = codec::decode_field(tag, "path", NbtPath::decode)?;
        Ok(StoredNumberAccess { storage: codec::decode_field(tag, "storage", codec::identifier)?, path })
    }

    pub fn get_numeric_tag(&self, context: &mut LootContext) -> Option<Nbt> {
        let value = Nbt::Compound(context.level().command_storage.borrow_mut().get(&self.storage));
        let selected_tags = self.path.get(&value).ok()?;
        match selected_tags.as_slice() {
            [tag @ (Nbt::Byte(_) | Nbt::Short(_) | Nbt::Int(_) | Nbt::Long(_) | Nbt::Float(_) | Nbt::Double(_))] => Some(tag.clone()),
            _ => None,
        }
    }
}
