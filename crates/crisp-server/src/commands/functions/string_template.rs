
use super::MAX_COMMAND_LINE_LENGTH;

#[derive(Debug)]
pub struct StringTemplate {
    pub segments: Vec<String>,
    pub variables: Vec<String>,
}

impl StringTemplate {
    pub fn from_string(input: &str) -> Result<StringTemplate, String> {
        let mut segments = Vec::new();
        let mut variables = Vec::new();
        let length = input.len();
        let mut start = 0;
        let mut index = input.find('$');
        while let Some(i) = index {
            if i != length - 1 && input.as_bytes()[i + 1] == b'(' {
                segments.push(input[start..i].to_string());
                let Some(variable_end) = input[i + 1..].find(')').map(|e| e + i + 1) else {
                    return Err("Unterminated macro variable".into());
                };
                let variable = &input[i + 2..variable_end];
                if !is_valid_variable_name(variable) {
                    return Err(format!("Invalid macro variable name '{variable}'"));
                }
                variables.push(variable.to_string());
                start = variable_end + 1;
                index = input[start..].find('$').map(|e| e + start);
            } else {
                index = input[i + 1..].find('$').map(|e| e + i + 1);
            }
        }
        if start == 0 {
            return Err("No variables in macro".into());
        }
        if start != length {
            segments.push(input[start..].to_string());
        }
        Ok(StringTemplate { segments, variables })
    }

    pub fn substitute(&self, arguments: &[&str]) -> Result<String, String> {
        let mut builder = String::new();
        let mut units = 0;
        let check = |units: usize, builder: &String| {
            if units > MAX_COMMAND_LINE_LENGTH {
                let truncated: Vec<u16> = builder.encode_utf16().take(512).collect();
                return Err(format!("Command too long: {units} characters, contents: {}...", String::from_utf16_lossy(&truncated)));
            }
            Ok(())
        };
        for (segment, argument) in self.segments.iter().zip(arguments) {
            builder.push_str(segment);
            builder.push_str(argument);
            units += segment.encode_utf16().count() + argument.encode_utf16().count();
            check(units, &builder)?;
        }
        if self.segments.len() > self.variables.len() {
            let last = self.segments.last().unwrap();
            builder.push_str(last);
            units += last.encode_utf16().count();
        }
        check(units, &builder)?;
        Ok(builder)
    }
}

pub fn is_valid_variable_name(variable: &str) -> bool {
    variable.chars().all(|c| c == '_' || (c <= '\u{FFFF}' && (c.is_alphabetic() || c.is_numeric())))
}
