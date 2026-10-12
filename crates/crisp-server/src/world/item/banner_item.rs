use crisp_data::Item;

use crate::world::item::dye_color::DyeColor;

pub fn is_banner_item(item: Item) -> bool {
    item.info().classes.contains(&"BannerItem")
}

pub fn get_color(item: Item) -> Option<DyeColor> {
    if !is_banner_item(item) {
        return None;
    }
    let fields: serde_json::Value = serde_json::from_str(item.info().block?.info().fields).unwrap_or_default();
    fields["color"].as_str().and_then(DyeColor::by_name)
}
