//! OS-specific application data directories.

use directories::ProjectDirs;
use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PathError {
    #[error("アプリケーションデータディレクトリを解決できませんでした")]
    ProjectDirsUnavailable,
}

/// Base data directory for this app (`AppData` / `Application Support` / XDG).
pub fn project_data_dir() -> Result<PathBuf, PathError> {
    ProjectDirs::from("local", "TaxReturnApplication", "TaxReturnApplication")
        .map(|dirs| dirs.data_dir().to_path_buf())
        .ok_or(PathError::ProjectDirsUnavailable)
}

/// Directory that holds `{year}.json` ledger files.
pub fn ledgers_dir() -> Result<PathBuf, PathError> {
    Ok(project_data_dir()?.join("ledgers"))
}

/// Directory for generated PDF exports.
pub fn exports_dir() -> Result<PathBuf, PathError> {
    Ok(project_data_dir()?.join("exports"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ledgers_dir_ends_with_ledgers() {
        // May fail only if home cannot be resolved — rare in CI/dev.
        if let Ok(dir) = ledgers_dir() {
            assert!(dir.ends_with("ledgers"));
        }
    }
}
