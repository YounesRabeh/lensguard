use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

use camera_app_resolver::{
    ApplicationResolver, DesktopEntryIndex, ProcessSource, ProcfsReader, ResolutionRequest,
};

static TEMP_SEQUENCE: AtomicUsize = AtomicUsize::new(0);

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new() -> Self {
        let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "lensguard-resolver-test-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn resolves_a_spawned_process() {
    let mut child = Command::new("sleep").arg("30").spawn().unwrap();
    let result = ProcfsReader::system().read_process(child.id());
    let _ = child.kill();
    let _ = child.wait();

    let process = result.unwrap();
    assert_eq!(process.process_name.as_deref(), Some("sleep"));
    assert_eq!(
        process
            .executable
            .as_deref()
            .and_then(Path::file_name)
            .and_then(|name| name.to_str()),
        Some("sleep")
    );
}

#[test]
fn resolves_desktop_and_flatpak_ids_from_a_temporary_fixture_directory() {
    let directory = TestDirectory::new();
    for (name, contents) in [
        (
            "org.gnome.Snapshot.desktop",
            include_str!("fixtures/desktop-files/org.gnome.Snapshot.desktop"),
        ),
        (
            "org.example.FlatCamera.desktop",
            include_str!("fixtures/desktop-files/org.example.FlatCamera.desktop"),
        ),
    ] {
        fs::write(directory.path().join(name), contents).unwrap();
    }

    let entries = DesktopEntryIndex::from_paths([directory.path().to_owned()]).unwrap();
    let mut resolver =
        ApplicationResolver::with_sources(ProcfsReader::system(), entries, 8).unwrap();

    let native = resolver.resolve(ResolutionRequest {
        pid: None,
        app_id: Some(String::from("org.gnome.Snapshot")),
        metadata_display_name: Some(String::from("org.gnome.Snapshot")),
        binary: None,
    });
    assert_eq!(native.display_name, "Snapshot");

    let flatpak = resolver.resolve(ResolutionRequest {
        pid: None,
        app_id: Some(String::from("org.example.FlatCamera")),
        metadata_display_name: None,
        binary: None,
    });
    assert_eq!(flatpak.display_name, "Flat Camera");
    assert_eq!(flatpak.app_id.as_deref(), Some("org.example.FlatCamera"));
}
