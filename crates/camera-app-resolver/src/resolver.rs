use std::collections::{BTreeMap, VecDeque};
use std::ffi::OsStr;

use camera_core::ApplicationIdentity;

use crate::{DesktopEntryIndex, ProcessSnapshot, ProcessSource, ProcfsReader, ResolverError};

const UNKNOWN_APPLICATION: &str = "Unknown application";
const MAX_IDENTITY_TEXT_CHARS: usize = 256;

/// Backend and process hints used to resolve a human-readable application identity.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ResolutionRequest {
    pub pid: Option<u32>,
    pub app_id: Option<String>,
    pub metadata_display_name: Option<String>,
    pub binary: Option<String>,
}

impl From<&ApplicationIdentity> for ResolutionRequest {
    fn from(identity: &ApplicationIdentity) -> Self {
        Self {
            pid: identity.pid,
            app_id: identity.app_id.clone(),
            metadata_display_name: (identity.display_name != UNKNOWN_APPLICATION)
                .then(|| identity.display_name.clone()),
            binary: identity.binary.clone(),
        }
    }
}

/// Bounded, explicitly invalidatable application identity resolver.
pub struct ApplicationResolver {
    process_source: Box<dyn ProcessSource>,
    desktop_entries: DesktopEntryIndex,
    cache_capacity: usize,
    cache: BTreeMap<ResolutionRequest, ApplicationIdentity>,
    recency: VecDeque<ResolutionRequest>,
}

impl ApplicationResolver {
    /// Creates a resolver using the system procfs and standard XDG desktop-entry paths.
    ///
    /// # Errors
    ///
    /// Returns [`ResolverError::ZeroCacheCapacity`] when `cache_capacity` is zero.
    pub fn new(cache_capacity: usize) -> Result<Self, ResolverError> {
        Self::with_sources(
            ProcfsReader::system(),
            DesktopEntryIndex::from_standard_paths(),
            cache_capacity,
        )
    }

    /// Creates a resolver from explicit sources for testing or alternate environments.
    ///
    /// # Errors
    ///
    /// Returns [`ResolverError::ZeroCacheCapacity`] when `cache_capacity` is zero.
    pub fn with_sources(
        process_source: impl ProcessSource + 'static,
        desktop_entries: DesktopEntryIndex,
        cache_capacity: usize,
    ) -> Result<Self, ResolverError> {
        if cache_capacity == 0 {
            return Err(ResolverError::ZeroCacheCapacity);
        }
        Ok(Self {
            process_source: Box::new(process_source),
            desktop_entries,
            cache_capacity,
            cache: BTreeMap::new(),
            recency: VecDeque::new(),
        })
    }

    /// Resolves identity hints without making missing or inaccessible process metadata fatal.
    ///
    /// The result is cached by all relevant input hints. Resolution reads no process command-line
    /// data and returns no desktop-entry icon path.
    pub fn resolve(&mut self, request: ResolutionRequest) -> ApplicationIdentity {
        if let Some(cached) = self.cache.get(&request).cloned() {
            self.touch(&request);
            return cached;
        }

        let process = request
            .pid
            .and_then(|pid| self.process_source.read_process(pid).ok());
        let resolved = self.resolve_uncached(&request, process.as_ref());
        self.insert_cache(request, resolved.clone());
        resolved
    }

    /// Invalidates one exact input key.
    pub fn invalidate(&mut self, request: &ResolutionRequest) -> bool {
        self.recency.retain(|key| key != request);
        self.cache.remove(request).is_some()
    }

    /// Clears all cached identities, for example after desktop-entry changes.
    pub fn clear_cache(&mut self) {
        self.cache.clear();
        self.recency.clear();
    }

    /// Returns the number of cached resolutions.
    #[must_use]
    pub fn cache_len(&self) -> usize {
        self.cache.len()
    }

    fn resolve_uncached(
        &self,
        request: &ResolutionRequest,
        process: Option<&ProcessSnapshot>,
    ) -> ApplicationIdentity {
        let metadata_name = request
            .metadata_display_name
            .as_deref()
            .and_then(plain_text);
        let request_app_id = request.app_id.as_deref().and_then(plain_text);
        let process_app_id = process
            .and_then(|process| process.flatpak_app_id.as_deref())
            .and_then(plain_text);
        let inferred_app_id = metadata_name
            .as_deref()
            .filter(|name| looks_like_app_id(name))
            .map(str::to_owned);
        let app_id = request_app_id.or(process_app_id).or(inferred_app_id);

        let request_binary = request.binary.as_deref().and_then(plain_text);
        let executable_name = process
            .and_then(|process| process.executable.as_deref())
            .and_then(|path| path.file_name())
            .and_then(OsStr::to_str)
            .and_then(plain_text);
        let process_name = process
            .and_then(|process| process.process_name.as_deref())
            .and_then(plain_text);
        let binary = request_binary
            .clone()
            .or(executable_name.clone())
            .or(process_name.clone());
        let desktop_entry = self.desktop_entries.find(
            app_id.as_deref(),
            binary
                .as_deref()
                .or(process_name.as_deref())
                .or(executable_name.as_deref()),
        );

        let metadata_is_machine_label = metadata_name.as_deref().is_some_and(|name| {
            app_id.as_deref() == Some(name)
                || binary.as_deref() == Some(name)
                || looks_like_app_id(name)
        });
        let display_name = if metadata_is_machine_label {
            desktop_entry
                .map(|entry| entry.name.clone())
                .or(metadata_name)
        } else {
            metadata_name.or_else(|| desktop_entry.map(|entry| entry.name.clone()))
        }
        .or(process_name)
        .or(executable_name)
        .or_else(|| binary.clone())
        .or_else(|| app_id.clone())
        .unwrap_or_else(|| String::from(UNKNOWN_APPLICATION));

        ApplicationIdentity {
            pid: request.pid,
            app_id,
            display_name,
            binary,
        }
    }

    fn insert_cache(&mut self, request: ResolutionRequest, identity: ApplicationIdentity) {
        if self.cache.len() == self.cache_capacity {
            if let Some(oldest) = self.recency.pop_front() {
                self.cache.remove(&oldest);
            }
        }
        self.recency.push_back(request.clone());
        self.cache.insert(request, identity);
    }

    fn touch(&mut self, request: &ResolutionRequest) {
        self.recency.retain(|key| key != request);
        self.recency.push_back(request.clone());
    }
}

fn plain_text(value: &str) -> Option<String> {
    let mut normalized = String::with_capacity(value.len());
    let mut previous_was_space = false;
    for character in value.trim().chars().take(MAX_IDENTITY_TEXT_CHARS) {
        let character = if character.is_control() {
            ' '
        } else {
            character
        };
        if character.is_whitespace() {
            if !previous_was_space {
                normalized.push(' ');
                previous_was_space = true;
            }
        } else {
            normalized.push(character);
            previous_was_space = false;
        }
    }
    let normalized = normalized.trim();
    (!normalized.is_empty()).then(|| normalized.to_owned())
}

fn looks_like_app_id(value: &str) -> bool {
    value.contains('.') && !value.chars().any(char::is_whitespace)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::{ApplicationResolver, ResolutionRequest};
    use crate::{DesktopEntry, DesktopEntryIndex, ProcessSnapshot, ProcessSource, ProcfsError};

    #[derive(Clone)]
    struct StubProcessSource {
        snapshot: Option<ProcessSnapshot>,
        permission_denied: bool,
        calls: Arc<AtomicUsize>,
    }

    impl StubProcessSource {
        fn with_snapshot(snapshot: ProcessSnapshot) -> Self {
            Self {
                snapshot: Some(snapshot),
                permission_denied: false,
                calls: Arc::new(AtomicUsize::new(0)),
            }
        }

        fn missing() -> Self {
            Self {
                snapshot: None,
                permission_denied: false,
                calls: Arc::new(AtomicUsize::new(0)),
            }
        }
    }

    impl ProcessSource for StubProcessSource {
        fn read_process(&self, pid: u32) -> Result<ProcessSnapshot, ProcfsError> {
            self.calls.fetch_add(1, Ordering::Relaxed);
            if self.permission_denied {
                return Err(ProcfsError::PermissionDenied {
                    pid,
                    path: PathBuf::from("/proc/blocked"),
                });
            }
            self.snapshot
                .clone()
                .ok_or_else(|| ProcfsError::MissingProcess {
                    pid,
                    path: PathBuf::from("/proc/missing"),
                })
        }
    }

    fn request() -> ResolutionRequest {
        ResolutionRequest {
            pid: Some(4242),
            app_id: None,
            metadata_display_name: None,
            binary: None,
        }
    }

    fn desktop_index() -> DesktopEntryIndex {
        DesktopEntryIndex::from_entries(vec![DesktopEntry {
            id: String::from("org.example.Camera"),
            name: String::from("Example Camera"),
            executable_name: Some(String::from("example-camera")),
            flatpak_app_id: Some(String::from("org.example.Camera")),
        }])
    }

    #[test]
    fn metadata_only_identity_is_preferred() {
        let mut resolver = ApplicationResolver::with_sources(
            StubProcessSource::missing(),
            DesktopEntryIndex::default(),
            4,
        )
        .unwrap();
        let identity = resolver.resolve(ResolutionRequest {
            pid: None,
            app_id: Some(String::from("org.example.Camera")),
            metadata_display_name: Some(String::from("Trusted Camera App")),
            binary: Some(String::from("camera-bin")),
        });
        assert_eq!(identity.display_name, "Trusted Camera App");
        assert_eq!(identity.app_id.as_deref(), Some("org.example.Camera"));
        assert_eq!(identity.binary.as_deref(), Some("camera-bin"));
    }

    #[test]
    fn process_name_and_executable_provide_pid_fallback() {
        let source = StubProcessSource::with_snapshot(ProcessSnapshot {
            process_name: Some(String::from("camera-worker")),
            executable: Some(PathBuf::from("/usr/bin/example-camera")),
            flatpak_app_id: None,
        });
        let mut resolver =
            ApplicationResolver::with_sources(source, DesktopEntryIndex::default(), 4).unwrap();
        let identity = resolver.resolve(request());
        assert_eq!(identity.display_name, "camera-worker");
        assert_eq!(identity.binary.as_deref(), Some("example-camera"));
    }

    #[test]
    fn desktop_entry_upgrades_machine_identifier_to_human_name() {
        let mut resolver =
            ApplicationResolver::with_sources(StubProcessSource::missing(), desktop_index(), 4)
                .unwrap();
        let identity = resolver.resolve(ResolutionRequest {
            pid: None,
            app_id: Some(String::from("org.example.Camera")),
            metadata_display_name: Some(String::from("org.example.Camera")),
            binary: None,
        });
        assert_eq!(identity.display_name, "Example Camera");
    }

    #[test]
    fn flatpak_process_metadata_matches_desktop_entry() {
        let source = StubProcessSource::with_snapshot(ProcessSnapshot {
            process_name: Some(String::from("bwrap")),
            executable: Some(PathBuf::from("/usr/bin/bwrap")),
            flatpak_app_id: Some(String::from("org.example.Camera")),
        });
        let mut resolver = ApplicationResolver::with_sources(source, desktop_index(), 4).unwrap();
        let identity = resolver.resolve(request());
        assert_eq!(identity.app_id.as_deref(), Some("org.example.Camera"));
        assert_eq!(identity.display_name, "Example Camera");
    }

    #[test]
    fn missing_and_permission_denied_processes_degrade_safely() {
        let mut permission_denied = StubProcessSource::missing();
        permission_denied.permission_denied = true;
        for source in [StubProcessSource::missing(), permission_denied] {
            let mut resolver =
                ApplicationResolver::with_sources(source, DesktopEntryIndex::default(), 4).unwrap();
            assert_eq!(
                resolver.resolve(request()).display_name,
                "Unknown application"
            );
        }
    }

    #[test]
    fn fallback_name_is_deterministic_plain_text() {
        let mut resolver = ApplicationResolver::with_sources(
            StubProcessSource::missing(),
            DesktopEntryIndex::default(),
            4,
        )
        .unwrap();
        let mut fallback_request = request();
        fallback_request.metadata_display_name = Some(String::from("Mystery\n\tCamera"));
        assert_eq!(
            resolver.resolve(fallback_request).display_name,
            "Mystery Camera"
        );
    }

    #[test]
    fn cache_hits_and_explicit_invalidation_are_observable() {
        let source = StubProcessSource::with_snapshot(ProcessSnapshot {
            process_name: Some(String::from("camera")),
            ..ProcessSnapshot::default()
        });
        let calls = Arc::clone(&source.calls);
        let mut resolver =
            ApplicationResolver::with_sources(source, DesktopEntryIndex::default(), 2).unwrap();
        let cache_key = request();
        resolver.resolve(cache_key.clone());
        resolver.resolve(cache_key.clone());
        assert_eq!(calls.load(Ordering::Relaxed), 1);
        assert_eq!(resolver.cache_len(), 1);

        assert!(resolver.invalidate(&cache_key));
        resolver.resolve(cache_key);
        assert_eq!(calls.load(Ordering::Relaxed), 2);

        let mut second = request();
        second.pid = Some(4243);
        resolver.resolve(second);
        let mut third = request();
        third.pid = Some(4244);
        resolver.resolve(third);
        assert_eq!(resolver.cache_len(), 2);
    }

    #[test]
    fn cache_and_external_identity_text_remain_bounded_under_churn() {
        let mut resolver = ApplicationResolver::with_sources(
            StubProcessSource::missing(),
            DesktopEntryIndex::default(),
            8,
        )
        .unwrap();

        for pid in 1..=1_000 {
            let identity = resolver.resolve(ResolutionRequest {
                pid: Some(pid),
                app_id: None,
                metadata_display_name: Some(format!("{pid}-{}", "📷".repeat(1_000))),
                binary: None,
            });
            assert!(identity.display_name.chars().count() <= 256);
        }

        assert_eq!(resolver.cache_len(), 8);
    }
}
