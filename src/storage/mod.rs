//! Local persistence: paths + JSON fiscal-year files.

pub mod json_store;
pub mod paths;

pub use json_store::{load_year, load_year_from_dir, save_year, save_year_to_dir, StorageError};
pub use paths::{exports_dir, ledgers_dir, project_data_dir};
