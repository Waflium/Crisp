use crisp_protocol::Writer;
use crisp_protocol::nbt::Nbt;

use crate::world::inventory::recipe_book_type::RecipeBookType;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct TypeSettings {
    pub open: bool,
    pub filtering: bool,
}

const FIELD_NAMES: [(&str, &str); 4] = [
    ("isGuiOpen", "isFilteringCraftable"),
    ("isFurnaceGuiOpen", "isFurnaceFilteringCraftable"),
    ("isBlastingFurnaceGuiOpen", "isBlastingFurnaceFilteringCraftable"),
    ("isSmokerGuiOpen", "isSmokerFilteringCraftable"),
];

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct RecipeBookSettings {
    settings: [TypeSettings; 4],
}

impl RecipeBookSettings {
    pub fn get_settings(&self, ty: RecipeBookType) -> TypeSettings {
        self.settings[ty as usize]
    }

    pub fn is_open(&self, ty: RecipeBookType) -> bool {
        self.get_settings(ty).open
    }

    pub fn set_open(&mut self, ty: RecipeBookType, open: bool) {
        self.settings[ty as usize].open = open;
    }

    pub fn is_filtering(&self, ty: RecipeBookType) -> bool {
        self.get_settings(ty).filtering
    }

    pub fn set_filtering(&mut self, ty: RecipeBookType, filtering: bool) {
        self.settings[ty as usize].filtering = filtering;
    }

    pub fn write(&self, w: &mut Writer) {
        for settings in &self.settings {
            w.bool(settings.open);
            w.bool(settings.filtering);
        }
    }

    pub fn decode(tag: &Nbt) -> RecipeBookSettings {
        let flag = |name: &str| tag.get(name).and_then(Nbt::as_i64).is_some_and(|value| value != 0);
        let mut settings = RecipeBookSettings::default();
        for (i, (open, filtering)) in FIELD_NAMES.iter().enumerate() {
            settings.settings[i] = TypeSettings { open: flag(open), filtering: flag(filtering) };
        }
        settings
    }

    pub fn encode(&self, output: &mut Vec<(String, Nbt)>) {
        for (settings, (open, filtering)) in self.settings.iter().zip(FIELD_NAMES) {
            if settings.open {
                output.push((open.into(), Nbt::Byte(1)));
            }
            if settings.filtering {
                output.push((filtering.into(), Nbt::Byte(1)));
            }
        }
    }
}
