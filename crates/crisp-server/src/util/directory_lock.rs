use std::fs::{File, OpenOptions, TryLockError};
use std::io::{self, Write};
use std::path::Path;

pub const LOCK_FILE: &str = "session.lock";

pub struct DirectoryLock {
    _lock_file: File,
}

impl DirectoryLock {
    pub fn create(dir: &Path) -> io::Result<DirectoryLock> {
        let lock_path = dir.join(LOCK_FILE);
        std::fs::create_dir_all(dir)?;
        let mut lock_file = OpenOptions::new().create(true).write(true).truncate(false).open(&lock_path)?;
        lock_file.write_all("\u{2603}".as_bytes()).and_then(|()| lock_file.sync_all()).map_err(java_io_exception)?;
        match lock_file.try_lock() {
            Ok(()) => Ok(DirectoryLock { _lock_file: lock_file }),
            Err(TryLockError::WouldBlock) => {
                let absolute = std::path::absolute(&lock_path).unwrap_or(lock_path);
                Err(io::Error::other(format!(
                    "net.minecraft.util.DirectoryLock$LockException: {}: already locked (possibly by other Minecraft instance?)",
                    absolute.display()
                )))
            }
            Err(TryLockError::Error(e)) => Err(java_io_exception(e)),
        }
    }
}

fn java_io_exception(error: io::Error) -> io::Error {
    let message = error.to_string();
    let message = message.split(" (os error").next().unwrap_or(&message).trim_end_matches('.');
    io::Error::other(format!("java.io.IOException: {message}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_second_lock_fails() {
        let dir = std::env::temp_dir().join(format!("crisp-lock-{}", std::process::id()));
        let first = DirectoryLock::create(&dir).unwrap();
        let second = DirectoryLock::create(&dir).err().map(|e| e.to_string()).unwrap_or_default();
        assert!(
            second.ends_with("session.lock: already locked (possibly by other Minecraft instance?)") || second.starts_with("java.io.IOException: "),
            "{second}"
        );
        drop(first);
        assert!(DirectoryLock::create(&dir).is_ok());
        assert_eq!(std::fs::read(dir.join(LOCK_FILE)).unwrap(), "\u{2603}".as_bytes());
        std::fs::remove_dir_all(dir).unwrap();
    }
}
