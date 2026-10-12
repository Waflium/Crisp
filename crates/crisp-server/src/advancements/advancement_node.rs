use super::advancement::Advancement;
use super::advancement_holder::AdvancementHolder;
use crate::util::java_collections::{JavaHashCode, identifier_hash};

pub struct AdvancementNode {
    pub holder: &'static AdvancementHolder,
    pub parent: Option<usize>,
    pub children: Vec<usize>,
    pub x: f32,
    pub y: f32,
}

impl AdvancementNode {
    pub fn new(holder: &'static AdvancementHolder, parent: Option<usize>) -> AdvancementNode {
        AdvancementNode { holder, parent, children: Vec::new(), x: 0.0, y: 0.0 }
    }

    pub fn advancement(&self) -> &'static Advancement {
        &self.holder.value
    }

    pub fn is_task(&self) -> bool {
        self.parent.is_some()
    }

    pub fn is_root(&self) -> bool {
        self.parent.is_none()
    }

    pub fn add_child(&mut self, child: usize) {
        if !self.children.contains(&child) {
            self.children.push(child);
        }
    }

    pub fn set_location(&mut self, x: f32, y: f32) {
        self.x = x;
        self.y = y;
    }
}

impl PartialEq for AdvancementNode {
    fn eq(&self, other: &AdvancementNode) -> bool {
        std::ptr::eq(self, other) || self.holder == other.holder
    }
}

impl JavaHashCode for &AdvancementNode {
    fn java_hash_code(&self) -> i32 {
        identifier_hash(&self.holder.id)
    }
}
