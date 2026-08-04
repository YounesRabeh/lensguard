use std::env;
use std::ffi::OsStr;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use crate::DesktopEntryError;

/// Safe subset of a freedesktop desktop entry used for application identity.
///
/// Icon fields are deliberately not retained or returned.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DesktopEntry {
    pub id: String,
    pub name: String,
    pub executable_name: Option<String>,
    pub flatpak_app_id: Option<String>,
}

/// Deterministically ordered desktop-entry lookup index.
#[derive(Clone, Debug, Default)]
pub struct DesktopEntryIndex {
    entries: Vec<DesktopEntry>,
}

impl DesktopEntryIndex {
    /// Builds an index from explicit `applications` directories.
    ///
    /// Missing directories are ignored. Other filesystem failures are typed and returned.
    ///
    /// # Errors
    ///
    /// Returns [`DesktopEntryError`] when a directory or desktop file cannot be read.
    pub fn from_paths(paths: impl IntoIterator<Item = PathBuf>) -> Result<Self, DesktopEntryError> {
        let mut files = Vec::new();
        for path in paths {
            let directory = match fs::read_dir(&path) {
                Ok(directory) => directory,
                Err(error) if error.kind() == ErrorKind::NotFound => continue,
                Err(source) => {
                    return Err(DesktopEntryError::ReadDirectory { path, source });
                }
            };
            for item in directory {
                let item = item.map_err(|source| DesktopEntryError::InspectPath {
                    path: path.clone(),
                    source,
                })?;
                let file_path = item.path();
                if file_path.extension() == Some(OsStr::new("desktop")) {
                    files.push(file_path);
                }
            }
        }
        files.sort();

        let mut entries = Vec::new();
        for path in files {
            let contents =
                fs::read_to_string(&path).map_err(|source| DesktopEntryError::ReadFile {
                    path: path.clone(),
                    source,
                })?;
            if let Some(entry) = parse_desktop_entry(&path, &contents) {
                entries.push(entry);
            }
        }
        entries.sort_by(|left, right| left.id.cmp(&right.id));
        Ok(Self { entries })
    }

    /// Builds the standard user/system index, ignoring inaccessible search directories so
    /// identity resolution can degrade gracefully.
    #[must_use]
    pub fn from_standard_paths() -> Self {
        let entries = standard_application_paths()
            .into_iter()
            .filter_map(|path| Self::from_paths([path]).ok())
            .flat_map(|index| index.entries)
            .collect::<Vec<_>>();
        let mut index = Self { entries };
        index.entries.sort_by(|left, right| left.id.cmp(&right.id));
        index.entries.dedup_by(|left, right| left.id == right.id);
        index
    }

    /// Builds an index from already parsed records.
    #[must_use]
    pub fn from_entries(mut entries: Vec<DesktopEntry>) -> Self {
        entries.sort_by(|left, right| left.id.cmp(&right.id));
        Self { entries }
    }

    pub(crate) fn find(
        &self,
        app_id: Option<&str>,
        executable_name: Option<&str>,
    ) -> Option<&DesktopEntry> {
        app_id
            .and_then(|app_id| {
                let normalized = app_id.strip_suffix(".desktop").unwrap_or(app_id);
                self.entries.iter().find(|entry| {
                    entry.id == normalized || entry.flatpak_app_id.as_deref() == Some(normalized)
                })
            })
            .or_else(|| {
                executable_name.and_then(|executable_name| {
                    self.entries
                        .iter()
                        .find(|entry| entry.executable_name.as_deref() == Some(executable_name))
                })
            })
    }
}

fn standard_application_paths() -> Vec<PathBuf> {
    let data_home = env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/share")));
    let mut paths = data_home
        .into_iter()
        .map(|path| path.join("applications"))
        .collect::<Vec<_>>();
    let data_dirs = env::var_os("XDG_DATA_DIRS").map_or_else(
        || {
            vec![
                PathBuf::from("/usr/local/share"),
                PathBuf::from("/usr/share"),
            ]
        },
        |value| env::split_paths(&value).collect(),
    );
    paths.extend(data_dirs.into_iter().map(|path| path.join("applications")));
    paths
}

fn parse_desktop_entry(path: &Path, contents: &str) -> Option<DesktopEntry> {
    let mut in_desktop_entry = false;
    let mut name = None;
    let mut exec = None;
    let mut flatpak_app_id = None;
    let mut hidden = false;

    for raw_line in contents.lines() {
        let line = raw_line.trim();
        if line.starts_with('[') && line.ends_with(']') {
            in_desktop_entry = line == "[Desktop Entry]";
            continue;
        }
        if !in_desktop_entry || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        match key {
            "Name" => name = nonempty(value),
            "Exec" => exec = executable_from_exec(value),
            "X-Flatpak" => flatpak_app_id = nonempty(value),
            "Hidden" => hidden = value.eq_ignore_ascii_case("true"),
            _ => {}
        }
    }
    if hidden {
        return None;
    }
    let id = path.file_stem()?.to_str()?.to_owned();
    Some(DesktopEntry {
        id,
        name: name?,
        executable_name: exec,
        flatpak_app_id,
    })
}

fn executable_from_exec(value: &str) -> Option<String> {
    let mut words = value.split_whitespace();
    let first = words.next()?;
    let executable = if first == "env" {
        words.find(|word| !word.contains('='))?
    } else {
        first
    };
    Path::new(executable)
        .file_name()
        .and_then(OsStr::to_str)
        .and_then(nonempty)
}

fn nonempty(value: &str) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_owned())
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{executable_from_exec, parse_desktop_entry};

    #[test]
    fn parses_safe_desktop_entry_fields_without_icons() {
        let entry = parse_desktop_entry(
            Path::new("org.example.Camera.desktop"),
            "[Desktop Entry]\nName=Example Camera\nExec=env FOO=bar /usr/bin/example-camera %U\nIcon=/tmp/untrusted.png\nX-Flatpak=org.example.Camera\n",
        )
        .unwrap();
        assert_eq!(entry.id, "org.example.Camera");
        assert_eq!(entry.name, "Example Camera");
        assert_eq!(entry.executable_name.as_deref(), Some("example-camera"));
        assert_eq!(entry.flatpak_app_id.as_deref(), Some("org.example.Camera"));
    }

    #[test]
    fn extracts_executable_basename() {
        assert_eq!(
            executable_from_exec("/usr/bin/camera-app --preview").as_deref(),
            Some("camera-app")
        );
    }

    #[test]
    fn parses_flatpak_style_id_from_fixture() {
        let entry = parse_desktop_entry(
            Path::new("org.example.FlatCamera.desktop"),
            include_str!("../tests/fixtures/desktop-files/org.example.FlatCamera.desktop"),
        )
        .unwrap();
        assert_eq!(
            entry.flatpak_app_id.as_deref(),
            Some("org.example.FlatCamera")
        );
        assert_eq!(entry.name, "Flat Camera");
    }
}
