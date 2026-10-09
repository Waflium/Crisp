#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum IntegerModifier {
    Add,
    Subtract,
    Multiply,
    Minimum,
    Maximum,
}

impl IntegerModifier {
    pub fn apply(self, subject: i32, argument: i32) -> i32 {
        match self {
            IntegerModifier::Add => subject.wrapping_add(argument),
            IntegerModifier::Subtract => subject.wrapping_sub(argument),
            IntegerModifier::Multiply => subject.wrapping_mul(argument),
            IntegerModifier::Minimum => subject.min(argument),
            IntegerModifier::Maximum => subject.max(argument),
        }
    }
}
