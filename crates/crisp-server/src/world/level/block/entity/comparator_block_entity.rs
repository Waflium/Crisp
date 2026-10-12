use crisp_protocol::nbt::Nbt;

#[derive(Clone, Default)]
pub struct ComparatorBlockEntity {
    output: i32,
}

impl ComparatorBlockEntity {
    pub fn save_additional(&self, output: &mut Vec<(String, Nbt)>) {
        output.push(("OutputSignal".into(), Nbt::Int(self.output)));
    }

    pub fn load_additional(&mut self, input: &Nbt) {
        self.output = super::int_or(input, "OutputSignal", 0);
    }

    pub fn get_output_signal(&self) -> i32 {
        self.output
    }

    pub fn set_output_signal(&mut self, value: i32) {
        self.output = value;
    }
}
