use std::sync::OnceLock;

pub const MOB_INVENTORY_SLOT_OFFSET: i32 = 300;
pub const MOB_INVENTORY_SIZE: i32 = 8;

#[derive(Debug)]
pub struct SlotRange {
    pub name: String,
    pub slots: Vec<i32>,
}

fn slots() -> &'static [SlotRange] {
    static SLOTS: OnceLock<Vec<SlotRange>> = OnceLock::new();
    SLOTS.get_or_init(|| {
        let mut values = Vec::new();
        let add_single_slot =|values: &mut Vec<SlotRange>, name: &str, id: i32| values.push(SlotRange { name: name.to_string(), slots: vec![id] });
        let add_slot_range = |values: &mut Vec<SlotRange>, prefix: &str, offset: i32, size: i32| {
            for i in 0..size {
                values.push(SlotRange { name: format!("{prefix}{i}"), slots: vec![offset + i] });
            }
            values.push(SlotRange { name: format!("{prefix}*"), slots: (offset..offset + size).collect() });
        };
        add_single_slot(&mut values, "contents", 0);
        add_slot_range(&mut values, "container.", 0, 54);
        add_slot_range(&mut values, "hotbar.", 0, 9);
        add_slot_range(&mut values, "inventory.", 9, 27);
        add_slot_range(&mut values, "enderchest.", 200, 27);
        add_slot_range(&mut values, "mob.inventory.", MOB_INVENTORY_SLOT_OFFSET, MOB_INVENTORY_SIZE);
        add_slot_range(&mut values, "horse.", 500, 15);
        let (mainhand, offhand) = (98, 99);
        add_single_slot(&mut values, "weapon", mainhand);
        add_single_slot(&mut values, "weapon.mainhand", mainhand);
        add_single_slot(&mut values, "weapon.offhand", offhand);
        values.push(SlotRange { name: "weapon.*".into(), slots: vec![mainhand, offhand] });
        let (head, chest, legs, feet, body) = (103, 102, 101, 100, 105);
        add_single_slot(&mut values, "armor.head", head);
        add_single_slot(&mut values, "armor.chest", chest);
        add_single_slot(&mut values, "armor.legs", legs);
        add_single_slot(&mut values, "armor.feet", feet);
        add_single_slot(&mut values, "armor.body", body);
        values.push(SlotRange { name: "armor.*".into(), slots: vec![head, chest, legs, feet, body] });
        add_single_slot(&mut values, "saddle", 106);
        add_single_slot(&mut values, "horse.chest", 499);
        add_single_slot(&mut values, "player.cursor", 499);
        add_slot_range(&mut values, "player.crafting.", 500, 4);
        values
    })
}

pub fn name_to_ids(name: &str) -> Option<&'static SlotRange> {
    slots().iter().find(|range| range.name == name)
}
