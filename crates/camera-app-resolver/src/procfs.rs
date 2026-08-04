use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use crate::{ProcfsError, flatpak};

/// Process metadata permitted for identity resolution.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ProcessSnapshot {
    pub process_name: Option<String>,
    pub executable: Option<PathBuf>,
    pub flatpak_app_id: Option<String>,
}

/// Testable process-inspection boundary.
pub trait ProcessSource: Send + Sync {
    /// Reads safe process metadata without reading the process command line.
    ///
    /// # Errors
    ///
    /// Returns a typed procfs error when the process disappeared, access is denied, or its primary
    /// process-name field cannot be read.
    fn read_process(&self, pid: u32) -> Result<ProcessSnapshot, ProcfsError>;
}

/// Linux procfs implementation of [`ProcessSource`].
#[derive(Clone, Debug)]
pub struct ProcfsReader {
    root: PathBuf,
}

impl ProcfsReader {
    /// Uses the system `/proc` mount.
    #[must_use]
    pub fn system() -> Self {
        Self {
            root: PathBuf::from("/proc"),
        }
    }

    /// Uses an explicit procfs root, primarily for integration tests.
    #[must_use]
    pub fn with_root(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }
}

impl Default for ProcfsReader {
    fn default() -> Self {
        Self::system()
    }
}

impl ProcessSource for ProcfsReader {
    fn read_process(&self, pid: u32) -> Result<ProcessSnapshot, ProcfsError> {
        let process_dir = self.root.join(pid.to_string());
        fs::metadata(&process_dir).map_err(|error| map_error(pid, &process_dir, error))?;

        let comm_path = process_dir.join("comm");
        let process_name = fs::read_to_string(&comm_path)
            .map_err(|error| map_error(pid, &comm_path, error))
            .map(|value| nonempty(value.trim()))?;
        let executable = read_link_optional(&process_dir.join("exe"));
        let flatpak_info = read_string_optional(&process_dir.join("root/.flatpak-info"));
        let cgroup = read_string_optional(&process_dir.join("cgroup"));
        let flatpak_app_id = flatpak_info
            .as_deref()
            .and_then(flatpak::app_id_from_flatpak_info)
            .or_else(|| cgroup.as_deref().and_then(flatpak::app_id_from_cgroup));

        Ok(ProcessSnapshot {
            process_name,
            executable,
            flatpak_app_id,
        })
    }
}

fn nonempty(value: &str) -> Option<String> {
    (!value.is_empty()).then(|| value.to_owned())
}

fn read_link_optional(path: &Path) -> Option<PathBuf> {
    fs::read_link(path).ok()
}

fn read_string_optional(path: &Path) -> Option<String> {
    fs::read_to_string(path).ok()
}

fn map_error(pid: u32, path: &Path, source: std::io::Error) -> ProcfsError {
    match source.kind() {
        ErrorKind::NotFound => ProcfsError::MissingProcess {
            pid,
            path: path.to_owned(),
        },
        ErrorKind::PermissionDenied => ProcfsError::PermissionDenied {
            pid,
            path: path.to_owned(),
        },
        _ => ProcfsError::Read {
            pid,
            path: path.to_owned(),
            source,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::{ProcessSource, ProcfsReader};
    use crate::ProcfsError;

    #[test]
    fn missing_process_returns_typed_error() {
        let reader = ProcfsReader::with_root("/definitely/not/a/procfs/mount");
        assert!(matches!(
            reader.read_process(42),
            Err(ProcfsError::MissingProcess { pid: 42, .. })
        ));
    }
}
