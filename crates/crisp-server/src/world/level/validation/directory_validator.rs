use std::io;
use std::path::{Path, PathBuf};

use super::ForbiddenSymlinkInfo;
use super::path_allow_list::PathAllowList;
use crate::server::dedicated::settings::java_io_exception;
use crate::util::basic_file_attributes::BasicFileAttributes;

#[derive(Debug)]
pub enum SymlinkTargetAllowList {
    Nothing,
    Everything,
    PathAllowList(PathAllowList),
}

#[derive(Debug)]
pub struct DirectoryValidator {
    symlink_target_allow_list: SymlinkTargetAllowList,
}

impl DirectoryValidator {
    pub fn new(symlink_target_allow_list: SymlinkTargetAllowList) -> DirectoryValidator {
        DirectoryValidator { symlink_target_allow_list }
    }

    fn matches(&self, target: &Path) -> bool {
        match &self.symlink_target_allow_list {
            SymlinkTargetAllowList::Nothing => false,
            SymlinkTargetAllowList::Everything => true,
            SymlinkTargetAllowList::PathAllowList(list) => list.matches(target),
        }
    }

    pub fn validate_symlink(&self, path: &Path, issues: &mut Vec<ForbiddenSymlinkInfo>) -> Result<(), String> {
        let target = read_symbolic_link(path)?;
        if !self.matches(&target) {
            issues.push(ForbiddenSymlinkInfo { link: path.to_path_buf(), target });
        }
        Ok(())
    }

    pub fn validate_directory(&self, directory: &Path, allow_top_symlink: bool) -> Result<Vec<ForbiddenSymlinkInfo>, String> {
        let mut issues = Vec::new();
        let attributes = match BasicFileAttributes::read(directory) {
            Ok(attributes) => attributes,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(issues),
            Err(e) => return Err(java_io_exception(&e, directory)),
        };
        if attributes.regular_file {
            return Err(format!("java.io.IOException: Path {} is not a directory", directory.display()));
        }
        let mut directory = directory.to_path_buf();
        if attributes.symbolic_link {
            if !allow_top_symlink {
                self.validate_symlink(&directory, &mut issues)?;
                return Ok(issues);
            }
            directory = read_symbolic_link(&directory)?;
        }
        self.validate_known_directory(&directory, &mut issues)?;
        Ok(issues)
    }

    pub fn validate_known_directory(&self, directory: &Path, issues: &mut Vec<ForbiddenSymlinkInfo>) -> Result<(), String> {
        let attributes = BasicFileAttributes::read(directory).map_err(|e| java_io_exception(&e, directory))?;
        if attributes.symbolic_link {
            return self.validate_symlink(directory, issues);
        }
        if !attributes.directory {
            return Ok(());
        }
        let entries = std::fs::read_dir(directory).map_err(|e| java_io_exception(&e, directory))?;
        for entry in entries {
            let path: PathBuf = entry.map_err(|e| java_io_exception(&e, directory))?.path();
            self.validate_known_directory(&path, issues)?;
        }
        Ok(())
    }
}

fn read_symbolic_link(path: &Path) -> Result<PathBuf, String> {
    std::fs::read_link(path).map_err(|e| java_io_exception(&e, path))
}
