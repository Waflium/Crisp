use crisp_protocol::nbt::Nbt;

use crate::world::item::component::{self as dc, DataComponentType};

const TAG_PATTERNS: &str = "patterns";

fn is_empty_patterns(patterns: &Nbt) -> bool {
    matches!(patterns, Nbt::List(list) if list.is_empty())
}

#[derive(Clone)]
pub struct BannerBlockEntity {
    name: Option<Nbt>,
    patterns: Nbt,
}

impl Default for BannerBlockEntity {
    fn default() -> BannerBlockEntity {
        BannerBlockEntity { name: None, patterns: Nbt::List(Vec::new()) }
    }
}

impl BannerBlockEntity {
    pub fn get_custom_name(&self) -> Option<&Nbt> {
        self.name.as_ref()
    }

    pub fn save_additional(&self, output: &mut Vec<(String, Nbt)>) {
        if !is_empty_patterns(&self.patterns) {
            output.push((TAG_PATTERNS.into(), self.patterns.clone()));
        }
        if let Some(name) = &self.name {
            output.push(("CustomName".into(), name.clone()));
        }
    }

    pub fn load_additional(&mut self, input: &Nbt) {
        self.name = input.get("CustomName").and_then(|name| dc::CUSTOM_NAME.decode(name).ok());
        self.patterns = input.get(TAG_PATTERNS).and_then(|patterns| dc::BANNER_PATTERNS.decode(patterns).ok()).unwrap_or(Nbt::List(Vec::new()));
    }

    pub fn get_patterns(&self) -> &Nbt {
        &self.patterns
    }

    pub fn apply_implicit_components<'a>(&mut self, get: &mut impl FnMut(DataComponentType) -> Option<&'a Nbt>) {
        self.patterns = get(dc::BANNER_PATTERNS).cloned().unwrap_or(Nbt::List(Vec::new()));
        self.name = get(dc::CUSTOM_NAME).cloned();
    }

    pub fn collect_implicit_components(&self, components: &mut Vec<(DataComponentType, Option<Nbt>)>) {
        let mut set = |ty: DataComponentType, value: Option<Nbt>| {
            components.retain(|(t, _)| *t != ty);
            components.push((ty, value));
        };
        set(dc::BANNER_PATTERNS, Some(self.patterns.clone()));
        set(dc::CUSTOM_NAME, self.name.clone());
    }
}
