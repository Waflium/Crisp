
use serde_json::Value;

use crate::network::chat::Component;
use crate::network::chat::component_serialization;
use crate::world::flag::FeatureFlagSet;
use crate::world::item::component::json_to_nbt;

const LAST_PRE_MINOR_VERSION: i32 = 81;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct PackFormat {
    pub major: i32,
    pub minor: i32,
}

impl PackFormat {
    pub const CURRENT: PackFormat = PackFormat { major: 121, minor: 0 };

    pub const fn of(major: i32, minor: i32) -> PackFormat {
        PackFormat { major, minor }
    }

    pub fn minor_range(self) -> FormatRange {
        FormatRange { min: self, max: PackFormat::of(self.major, i32::MAX) }
    }
}

impl std::fmt::Display for PackFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        if self.minor == i32::MAX { write!(f, "{}.*", self.major) } else { write!(f, "{}.{}", self.major, self.minor) }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct FormatRange {
    pub min: PackFormat,
    pub max: PackFormat,
}

impl FormatRange {
    pub fn contains(&self, format: PackFormat) -> bool {
        format >= self.min && format <= self.max
    }
}

#[derive(Clone, Debug)]
pub struct PackMetadataSection {
    pub description: Component,
    pub supported_formats: FormatRange,
}

fn int(value: &Value) -> Result<i32, String> {
    match value {
        Value::Number(n) => Ok(n.as_i64().map(|v| v as i32).or_else(|| n.as_f64().map(|v| v as i32)).unwrap_or(0)),
        Value::Bool(b) => Ok(*b as i32),
        other => Err(format!("Not a number: {other}")),
    }
}

fn non_negative_int(value: &Value) -> Result<i32, String> {
    let n = int(value)?;
    if n < 0 { Err(format!("Value must be non-negative: {n}")) } else { Ok(n) }
}

fn pack_format(value: &Value, default_minor: i32) -> Result<PackFormat, String> {
    match value {
        Value::Array(list) => {
            if list.is_empty() || list.len() > 256 {
                return Err(format!("List must have between 1 and 256 elements: {}", list.len()));
            }
            let major = non_negative_int(&list[0])?;
            let minor = match list.get(1) {
                Some(minor) => non_negative_int(minor)?,
                None => default_minor,
            };
            Ok(PackFormat::of(major, minor))
        }
        other => Ok(PackFormat::of(non_negative_int(other)?, default_minor)),
    }
}

fn int_range(value: &Value) -> Result<(i32, i32), String> {
    let (min, max) = match value {
        Value::Array(list) => {
            if list.len() != 2 {
                return Err(format!("Input is not a list of 2 elements: {value}"));
            }
            (int(&list[0])?, int(&list[1])?)
        }
        Value::Object(object) => {
            let field = |name: &str| object.get(name).ok_or_else(|| format!("No key {name} in {value}")).and_then(int);
            (field("min_inclusive")?, field("max_inclusive")?)
        }
        other => {
            let v = int(other)?;
            (v, v)
        }
    };
    if min > max {
        return Err("min_inclusive must be less than or equal to max_inclusive".into());
    }
    Ok((min, max))
}

#[derive(Debug, Default)]
struct IntermediaryFormat {
    min: Option<PackFormat>,
    max: Option<PackFormat>,
    format: Option<i32>,
    supported: Option<(i32, i32)>,
}

impl IntermediaryFormat {
    fn pack(object: &serde_json::Map<String, Value>) -> Result<IntermediaryFormat, String> {
        Ok(IntermediaryFormat {
            min: object.get("min_format").map(|v| pack_format(v, 0)).transpose()?,
            max: object.get("max_format").map(|v| pack_format(v, i32::MAX)).transpose()?,
            format: object.get("pack_format").map(int).transpose()?,
            supported: object.get("supported_formats").map(int_range).transpose()?,
        })
    }

    fn overlay(object: &serde_json::Map<String, Value>) -> Result<IntermediaryFormat, String> {
        let min = object.get("min_format").map(|v| pack_format(v, 0)).transpose()?;
        Ok(IntermediaryFormat {
            min,
            max: object.get("max_format").map(|v| pack_format(v, i32::MAX)).transpose()?,
            format: min.map(|m| m.major),
            supported: object.get("formats").map(int_range).transpose()?,
        })
    }

    fn effective_min_major_version(&self) -> i32 {
        match (self.min, self.supported) {
            (Some(min), Some(supported)) => min.major.min(supported.0),
            (Some(min), None) => min.major,
            (None, Some(supported)) => supported.0,
            (None, None) => i32::MAX,
        }
    }

    fn validate(&self, has_pack_format_field: bool, require_old_field: bool, context: &str, old_field_name: &str) -> Result<FormatRange, String> {
        let last = LAST_PRE_MINOR_VERSION;
        if self.min.is_some() != self.max.is_some() {
            return Err(format!("{context} missing field, must declare both min_format and max_format"));
        }
        if require_old_field && self.supported.is_none() {
            return Err(format!(
                "{context} missing required field {old_field_name}, must be present in all overlays for any overlays to work across game versions"
            ));
        }
        if let (Some(min), Some(max)) = (self.min, self.max) {
            return self.validate_new_format(min, max, has_pack_format_field, require_old_field, context, old_field_name);
        }
        if let Some((min, max)) = self.supported {
            if max > last {
                return Err(format!("{context} declares support for version newer than {last}, but is missing mandatory fields min_format and max_format"));
            }
            if has_pack_format_field {
                let Some(format) = self.format else {
                    return Err(format!(
                        "{context} declares support for formats up to {last}, but game versions supporting formats 15 to {last} require a pack_format field. Add \"pack_format\": {min} or require a version greater or equal to {}.0.",
                        last + 1
                    ));
                };
                validate_pack_format_for_range(format, min, max)?;
            }
            return Ok(FormatRange { min: PackFormat::of(min, 0), max: PackFormat::of(max, 0) });
        }
        if has_pack_format_field && let Some(format) = self.format {
            if format > last {
                return Err(format!("{context} declares support for version newer than {last}, but is missing mandatory fields min_format and max_format"));
            }
            return Ok(FormatRange { min: PackFormat::of(format, 0), max: PackFormat::of(format, 0) });
        }
        Err(format!("{context} could not be parsed, missing format version information"))
    }

    fn validate_new_format(
        &self,
        min: PackFormat,
        max: PackFormat,
        has_pack_format_field: bool,
        require_old_field: bool,
        context: &str,
        old_field_name: &str,
    ) -> Result<FormatRange, String> {
        let last = LAST_PRE_MINOR_VERSION;
        let (major_min, major_max) = (min.major, max.major);
        if min > max {
            return Err(format!("{context} min_format ({min}) is greater than max_format ({max})"));
        }
        if major_min > last && !require_old_field {
            if self.supported.is_some() {
                return Err(format!(
                    "{context} key {old_field_name} is deprecated starting from pack format {}. Remove {old_field_name} from your pack.mcmeta.",
                    last + 1
                ));
            }
            if has_pack_format_field && let Some(format) = self.format {
                validate_pack_format_for_range(format, major_min, major_max)?;
            }
        } else {
            let Some(supported) = self.supported else {
                return Err(format!(
                    "{context} declares support for format {major_min}, but game versions supporting formats 15 to {last} require a {old_field_name} field. Add \"{old_field_name}\": [{major_min}, {last}] or require a version greater or equal to {}.0.",
                    last + 1
                ));
            };
            if supported.0 != major_min {
                return Err(format!("{context} version declaration mismatch between {old_field_name} (from {}) and min_format ({min})", supported.0));
            }
            if supported.1 != major_max && supported.1 != last {
                return Err(format!("{context} version declaration mismatch between {old_field_name} (up to {}) and max_format ({max})", supported.1));
            }
            if has_pack_format_field {
                let Some(format) = self.format else {
                    return Err(format!(
                        "{context} declares support for formats up to {last}, but game versions supporting formats 15 to {last} require a pack_format field. Add \"pack_format\": {major_min} or require a version greater or equal to {}.0.",
                        last + 1
                    ));
                };
                validate_pack_format_for_range(format, major_min, major_max)?;
            }
        }
        Ok(FormatRange { min, max })
    }
}

fn validate_pack_format_for_range(format: i32, min: i32, max: i32) -> Result<(), String> {
    if format < min || format > max {
        return Err(format!("Pack declared support for versions {min} to {max} but declared main format is {format}"));
    }
    if format < 15 {
        return Err("Multi-version packs cannot support minimum version of less than 15, since this will leave versions in range unable to load pack.".into());
    }
    Ok(())
}

fn object<'a>(value: &'a Value, what: &str) -> Result<&'a serde_json::Map<String, Value>, String> {
    value.as_object().ok_or_else(|| format!("Not a JSON object: {what}"))
}

fn description(value: Option<&Value>) -> Result<Component, String> {
    let value = value.ok_or("No key description in MapLike")?;
    component_serialization::decode(&json_to_nbt(value))
}

pub fn pack_section(value: &Value) -> Result<PackMetadataSection, String> {
    let object = object(value, "pack")?;
    let description = description(object.get("description"))?;
    let supported_formats = IntermediaryFormat::pack(object)?.validate(true, false, "Pack", "supported_formats")?;
    Ok(PackMetadataSection { description, supported_formats })
}

pub fn fallback_pack_section(value: &Value) -> Result<PackMetadataSection, String> {
    let description = description(object(value, "pack")?.get("description"))?;
    let unknown = PackFormat::of(i32::MAX, 0);
    Ok(PackMetadataSection { description, supported_formats: FormatRange { min: unknown, max: unknown } })
}

pub fn features_section(value: &Value) -> Result<FeatureFlagSet, String> {
    let enabled = object(value, "features")?.get("enabled").ok_or("No key enabled in MapLike")?;
    let names = enabled.as_array().ok_or_else(|| format!("Not a list: {enabled}"))?;
    let names: Vec<&str> = names.iter().map(|n| n.as_str().ok_or_else(|| format!("Not a string: {n}"))).collect::<Result<_, _>>()?;
    FeatureFlagSet::from_names(names)
}

pub fn overlays_section(value: &Value) -> Result<Vec<String>, String> {
    let entries = object(value, "overlays")?.get("entries").ok_or("No key entries in MapLike")?;
    let entries = entries.as_array().ok_or_else(|| format!("Not a list: {entries}"))?;
    let mut parsed = Vec::with_capacity(entries.len());
    for entry in entries {
        let entry = object(entry, "overlay")?;
        let format = IntermediaryFormat::overlay(entry)?;
        let directory = entry.get("directory").and_then(Value::as_str).ok_or("No key directory in MapLike")?;
        if directory.is_empty() || !directory.bytes().all(|c| c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_' | b'.')) {
            return Err(format!("{directory} is not accepted directory name"));
        }
        parsed.push((format, directory.to_string()));
    }
    let min_version = parsed.iter().map(|(format, _)| format.effective_min_major_version()).min().unwrap_or(i32::MAX);
    let mut overlays = Vec::new();
    for (format, directory) in parsed {
        if format.min.is_none() && format.max.is_none() && format.supported.is_none() {
            warn!("Unknown or broken overlay entry {directory}");
            continue;
        }
        let range = format.validate(false, min_version <= LAST_PRE_MINOR_VERSION, &format!("Overlay \"{directory}\""), "formats")?;
        if range.contains(PackFormat::CURRENT) {
            overlays.push(directory);
        }
    }
    Ok(overlays)
}
