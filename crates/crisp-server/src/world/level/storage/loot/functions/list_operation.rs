use crisp_protocol::nbt::Nbt;

use crate::util::codec::{self, DataResult};

#[derive(Clone, Copy, Debug)]
pub enum ListOperation {
    ReplaceAll,
    ReplaceSection { offset: i32, size: Option<i32> },
    Insert { offset: i32 },
    Append,
}

fn non_negative(tag: &Nbt) -> DataResult<i32> {
    let value = codec::int(tag)?;
    if value < 0 {
        return Err(format!("Value must be non-negative: {value}"));
    }
    Ok(value)
}

impl ListOperation {
    pub fn decode(tag: &Nbt, max_size: i32) -> DataResult<ListOperation> {
        let mode = codec::decode_field(tag, "mode", |t| {
            codec::string_enum(t, &[("replace_all", 0), ("replace_section", 1), ("insert", 2), ("append", 3)])
        })?;
        let operation = match mode {
            0 => ListOperation::ReplaceAll,
            1 => ListOperation::ReplaceSection { offset: codec::decode_or(tag, "offset", 0, non_negative)?, size: codec::decode_optional(tag, "size", non_negative)? },
            2 => ListOperation::Insert { offset: codec::decode_or(tag, "offset", 0, non_negative)? },
            _ => ListOperation::Append,
        };
        if let ListOperation::ReplaceSection { size: Some(size), .. } = operation
            && size > max_size
        {
            return Err(format!("Size value too large: {size}, max size is {max_size}"));
        }
        Ok(operation)
    }

    pub fn apply<T: Clone>(self, original: &[T], replacement: &[T], max_size: usize) -> Vec<T> {
        match self {
            ListOperation::ReplaceAll => replacement.to_vec(),
            ListOperation::Append => {
                if original.len() + replacement.len() > max_size {
                    error!("Contents overflow in section append");
                    return original.to_vec();
                }
                original.iter().chain(replacement).cloned().collect()
            }
            ListOperation::Insert { offset } => {
                let offset = offset as usize;
                if offset > original.len() {
                    error!("Cannot insert when offset is out of bounds");
                    return original.to_vec();
                }
                if original.len() + replacement.len() > max_size {
                    error!("Contents overflow in section insertion");
                    return original.to_vec();
                }
                original[..offset].iter().chain(replacement).chain(&original[offset..]).cloned().collect()
            }
            ListOperation::ReplaceSection { offset, size } => {
                let offset = offset as usize;
                if offset > original.len() {
                    error!("Cannot replace when offset is out of bounds");
                    return original.to_vec();
                }
                let mut new_list: Vec<T> = original[..offset].to_vec();
                new_list.extend_from_slice(replacement);
                let resume_index = offset + size.map_or(replacement.len(), |s| s as usize);
                if resume_index < original.len() {
                    new_list.extend_from_slice(&original[resume_index..]);
                }
                if new_list.len() > max_size {
                    error!("Contents overflow in section replacement");
                    return original.to_vec();
                }
                new_list
            }
        }
    }
}

pub struct StandAlone<T> {
    pub value: Vec<T>,
    pub operation: ListOperation,
}

impl<T> StandAlone<T> {
    pub fn decode(tag: &Nbt, value: impl Fn(&Nbt) -> DataResult<T>, max_size: i32) -> DataResult<StandAlone<T>> {
        let values = codec::decode_field(tag, "values", |t| codec::list_of(t, &value))?;
        if values.len() > max_size as usize {
            return Err(format!("Too long: {}, expected range [0-{max_size}]", values.len()));
        }
        Ok(StandAlone { value: values, operation: ListOperation::decode(tag, max_size)? })
    }
}

impl<T: Clone> StandAlone<T> {
    pub fn apply(&self, input: &[T]) -> Vec<T> {
        self.operation.apply(input, &self.value, i32::MAX as usize)
    }
}
