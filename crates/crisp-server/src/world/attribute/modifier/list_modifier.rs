use std::sync::Arc;

use crate::world::attribute::Value;

pub fn append(subject: &Value, argument: &Value) -> Value {
    let (Value::AmbientParticles(list), Value::AmbientParticles(extra)) = (subject, argument) else { panic!("not lists: {subject:?}, {argument:?}") };
    if extra.is_empty() {
        subject.clone()
    } else if list.is_empty() {
        argument.clone()
    } else {
        Value::AmbientParticles(Arc::new(list.iter().chain(extra.iter()).cloned().collect()))
    }
}
