use crisp_data::registry;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Activity(pub u16);

impl Activity {
    pub fn by_name(name: &str) -> Option<Activity> {
        registry::ACTIVITY.iter().position(|&a| a == name).map(|i| Activity(i as u16))
    }

    pub fn name(self) -> &'static str {
        registry::ACTIVITY[self.0 as usize]
    }
}
