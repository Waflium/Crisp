use std::sync::OnceLock;

use crisp_protocol::nbt::Nbt;

use crate::util::codec::{self, DataResult};

pub trait RegistryElement: Sized + Send + Sync + 'static {
    const REGISTRY: &'static str;

    fn contains_key(key: &str) -> bool;

    fn lookup(key: &str) -> Option<&'static Self>;
}

pub enum Holder<T: RegistryElement> {
    Direct(Box<T>),
    Reference(String, OnceLock<&'static T>),
}

impl<T: RegistryElement> Holder<T> {
    pub fn direct(value: T) -> Holder<T> {
        Holder::Direct(Box::new(value))
    }

    pub fn value(&self) -> &T {
        match self {
            Holder::Direct(value) => value,
            Holder::Reference(key, value) => value.get_or_init(|| T::lookup(key).unwrap_or_else(|| panic!("Unbound value in {}: {key}", T::REGISTRY))),
        }
    }

    pub fn decode(tag: &Nbt, direct: impl FnOnce(&Nbt) -> DataResult<T>) -> DataResult<Holder<T>> {
        match tag {
            Nbt::String(_) => {
                let key = codec::identifier(tag)?;
                if T::contains_key(&key) {
                    Ok(Holder::Reference(key, OnceLock::new()))
                } else {
                    Err(format!("Missing element ResourceKey[{} / {key}]", T::REGISTRY))
                }
            }
            _ => Ok(Holder::direct(direct(tag)?)),
        }
    }
}
