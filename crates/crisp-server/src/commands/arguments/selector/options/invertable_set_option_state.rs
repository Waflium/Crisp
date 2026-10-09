#[derive(Clone, Copy, PartialEq, Eq, Default)]
enum Limitation {
    #[default]
    None,
    Single,
    Multiple,
}

#[derive(Default)]
pub struct InvertableSetOptionState {
    state: Limitation,
    tags: Vec<String>,
}

impl InvertableSetOptionState {
    fn can_limit_to_single(&self) -> bool {
        self.state == Limitation::None
    }

    fn can_limit_to_multiple(&self) -> bool {
        self.state != Limitation::Single
    }

    pub fn can_parse_positive_element(&self) -> bool {
        self.can_limit_to_single()
    }

    pub fn can_parse_negative_element(&self) -> bool {
        self.can_limit_to_multiple()
    }

    pub fn can_parse_any_tag(&self) -> bool {
        self.can_limit_to_multiple()
    }

    pub fn can_parse_tag(&self, tag: &str) -> bool {
        self.can_parse_any_tag() && !self.tags.iter().any(|t| t == tag)
    }

    pub fn can_parse_any(&self) -> bool {
        self.state != Limitation::Single
    }

    pub fn can_parse_element(&self, inverted: bool) -> bool {
        if inverted { self.can_parse_negative_element() } else { self.can_parse_positive_element() }
    }

    pub fn mark_parsed_tag(&mut self, tag: String) {
        self.state = Limitation::Multiple;
        self.tags.push(tag);
    }

    pub fn mark_parsed_element(&mut self, inverted: bool) {
        self.state = if inverted { Limitation::Multiple } else { Limitation::Single };
    }
}
