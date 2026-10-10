use std::path::{Path, PathBuf};

pub mod directory_validator;
pub mod path_allow_list;

#[derive(Debug)]
pub struct ForbiddenSymlinkInfo {
    pub link: PathBuf,
    pub target: PathBuf,
}

pub fn content_validation_message(directory: &Path, entries: &[ForbiddenSymlinkInfo]) -> String {
    let entries: Vec<String> = entries.iter().map(|e| format!("{}->{}", e.link.display(), e.target.display())).collect();
    format!("Failed to validate '{}'. Found forbidden symlinks: {}", directory.display(), entries.join(", "))
}
