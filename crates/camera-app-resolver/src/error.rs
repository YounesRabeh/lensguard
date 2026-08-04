use std::path::PathBuf;

use thiserror::Error;

/// Failures while inspecting a process through procfs.
#[derive(Debug, Error)]
pub enum ProcfsError {
    #[error("process {pid} no longer exists at {path}")]
    MissingProcess { pid: u32, path: PathBuf },
    #[error("permission denied while reading process {pid} metadata at {path}")]
    PermissionDenied { pid: u32, path: PathBuf },
    #[error("failed to read process {pid} metadata at {path}: {source}")]
    Read {
        pid: u32,
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

/// Failures while loading explicit desktop-entry search paths.
#[derive(Debug, Error)]
pub enum DesktopEntryError {
    #[error("failed to read desktop-entry directory {path}: {source}")]
    ReadDirectory {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to inspect desktop-entry path {path}: {source}")]
    InspectPath {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to read desktop entry {path}: {source}")]
    ReadFile {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

/// Resolver construction failures.
#[derive(Debug, Error)]
pub enum ResolverError {
    #[error("application resolver cache capacity must be greater than zero")]
    ZeroCacheCapacity,
    #[error(transparent)]
    DesktopEntries(#[from] DesktopEntryError),
}
