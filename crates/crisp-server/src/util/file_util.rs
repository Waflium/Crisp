use std::path::Path;

fn contains_allowed_characters_only(segment: &str) -> bool {
    !segment.is_empty() && segment.bytes().all(|c| matches!(c, b'-' | b'.' | b'_' | b'a'..=b'z' | b'0'..=b'9'))
}

pub fn is_valid_path_segment(segment: &str) -> bool {
    segment != ".." && segment != "." && contains_allowed_characters_only(segment)
}

pub fn create_directories_safe(dir: &Path) -> std::io::Result<()> {
    match dir.exists() {
        true => std::fs::create_dir_all(dir.canonicalize()?),
        false => std::fs::create_dir_all(dir),
    }
}
