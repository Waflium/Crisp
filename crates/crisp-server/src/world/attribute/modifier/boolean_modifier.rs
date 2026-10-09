#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum BooleanModifier {
    And,
    Nand,
    Or,
    Nor,
    Xor,
    Xnor,
}

impl BooleanModifier {
    pub fn apply(self, subject: bool, argument: bool) -> bool {
        match self {
            BooleanModifier::And => argument && subject,
            BooleanModifier::Nand => !argument || !subject,
            BooleanModifier::Or => argument || subject,
            BooleanModifier::Nor => !argument && !subject,
            BooleanModifier::Xor => argument ^ subject,
            BooleanModifier::Xnor => argument == subject,
        }
    }
}
