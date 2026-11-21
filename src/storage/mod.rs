//! Local persistence: Unified SQLite + GnuCash compatibility + JSON backup.

pub mod json_store;
pub mod migration;
pub mod paths;
pub mod sqlite_store;

pub use json_store::{load_year, load_year_from_dir, save_year, save_year_to_dir, StorageError};
pub use migration::{auto_migrate_if_needed, detect_legacy_json_years, migrate_json_directory_to_sqlite, MigrationReport};
pub use paths::{exports_dir, ledgers_dir, project_data_dir};
pub use sqlite_store::{
    export_gnucash_database, get_sqlite_registered_years, get_sqlite_stats, init_sqlite_schema,
    load_year_from_sqlite, open_sqlite_connection, save_year_to_sqlite, sqlite_db_path,
    SqliteError, SqliteStorageStats,
};
