//! Exclusive lock for a SQLite database file.
//!
//! Orbit holds `<database>.lock` for the life of a SQLite process, so `setup-token`
//! cannot run beside the server. Postgres uses a shared advisory lock instead and
//! allows the command while the server is up. Haruvo does the same split: this file
//! lock is SQLite-only.

use std::fs::{File, OpenOptions, TryLockError};
use std::path::{Path, PathBuf};

/// Holds an exclusive `flock` on `<database>.lock` until dropped.
#[derive(Debug)]
pub struct Guard {
    _file: File,
}

pub fn lock_sqlite(database_path: &Path) -> Result<Guard, String> {
    if let Some(parent) = database_path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent).map_err(|error| {
            format!(
                "failed to create the database directory {}: {error}",
                parent.display()
            )
        })?;
    }
    let lock_path = lock_path(database_path);
    let file = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(false)
        .open(&lock_path)
        .map_err(|error| {
            format!(
                "failed to open database ownership lock {}: {error}",
                lock_path.display()
            )
        })?;
    match file.try_lock() {
        Ok(()) => Ok(Guard { _file: file }),
        Err(TryLockError::WouldBlock) => Err(format!(
            "database is already owned by another Haruvo process: {}. On SQLite, stop Haruvo before `setup-token rotate` or starting another server",
            database_path.display()
        )),
        Err(TryLockError::Error(error)) => Err(format!(
            "failed to lock {}: {error}",
            database_path.display()
        )),
    }
}

fn lock_path(path: &Path) -> PathBuf {
    let base = std::fs::canonicalize(path).unwrap_or_else(|_| {
        if path.is_absolute() {
            path.to_path_buf()
        } else {
            std::env::current_dir()
                .map(|cwd| cwd.join(path))
                .unwrap_or_else(|_| path.to_path_buf())
        }
    });
    let mut os = base.into_os_string();
    os.push(".lock");
    PathBuf::from(os)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn second_sqlite_lock_fails_until_the_first_is_dropped() {
        let path = std::env::temp_dir().join(format!(
            "haruvo-lock-{}-{}.sqlite",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let first = lock_sqlite(&path).unwrap();
        let conflict = lock_sqlite(&path).unwrap_err();
        assert!(conflict.contains("already owned"), "{conflict}");
        drop(first);
        let second = lock_sqlite(&path).unwrap();
        drop(second);
        let _ = std::fs::remove_file(lock_path(&path));
    }
}
