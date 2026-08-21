use std::collections::BTreeMap;

use crate::{CameraSession, MonitorEvent, ObserverAvailability, SessionId, SuppressionDiagnostics};

/// A deterministic, transport-independent snapshot of monitor state.
///
/// `active_sessions` is always ordered by [`SessionId`]. Active state is derived from whether
/// this collection is empty and is never stored as a second source of truth.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MonitorSnapshot {
    pub active_sessions: Vec<CameraSession>,
    pub observer_availability: ObserverAvailability,
    pub observer_status_detail: String,
    pub suppression_diagnostics: SuppressionDiagnostics,
}

impl MonitorSnapshot {
    /// Returns whether at least one camera session is active.
    #[must_use]
    pub fn active(&self) -> bool {
        !self.active_sessions.is_empty()
    }

    /// Returns the number of active camera sessions.
    #[must_use]
    pub fn active_session_count(&self) -> usize {
        self.active_sessions.len()
    }
}

/// Owns the current active-session registry and backend availability.
///
/// # Invariants
///
/// - At most one session exists for each [`SessionId`].
/// - Applying a duplicate event does not change state.
/// - Unknown stop and update events are harmless.
/// - Snapshots are ordered by session identifier.
/// - Active state is derived exclusively from the session registry.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MonitorState {
    active_sessions: BTreeMap<SessionId, CameraSession>,
    observer_availability: ObserverAvailability,
    observer_status_detail: String,
    suppression_diagnostics: SuppressionDiagnostics,
}

impl Default for MonitorState {
    fn default() -> Self {
        Self {
            active_sessions: BTreeMap::new(),
            observer_availability: ObserverAvailability::Available,
            observer_status_detail: String::new(),
            suppression_diagnostics: SuppressionDiagnostics::default(),
        }
    }
}

impl MonitorState {
    /// Creates empty state with the backend initially considered available.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Applies one event without performing I/O.
    ///
    /// Returns `true` exactly when the observable state changed. A start for an existing ID
    /// replaces differing session metadata, while an update for an unknown ID is ignored.
    pub fn apply(&mut self, event: MonitorEvent) -> bool {
        match event {
            MonitorEvent::SessionStarted(session) => self.insert_or_replace(session),
            MonitorEvent::SessionUpdated(session) => self.update_existing(session),
            MonitorEvent::SessionStopped(session_id) => {
                self.active_sessions.remove(&session_id).is_some()
            }
            MonitorEvent::ObserverAvailabilityChanged {
                availability,
                detail,
            } => {
                let changed = self.observer_availability != availability
                    || self.observer_status_detail != detail;
                self.observer_availability = availability;
                self.observer_status_detail = detail;
                changed
            }
            MonitorEvent::SuppressionDiagnosticsChanged(diagnostics) => {
                let changed = self.suppression_diagnostics != diagnostics;
                self.suppression_diagnostics = diagnostics;
                changed
            }
        }
    }

    /// Returns whether at least one camera session is active.
    #[must_use]
    pub fn active(&self) -> bool {
        !self.active_sessions.is_empty()
    }

    /// Returns the number of active camera sessions.
    #[must_use]
    pub fn active_session_count(&self) -> usize {
        self.active_sessions.len()
    }

    /// Looks up an active session by stable identity.
    #[must_use]
    pub fn active_session(&self, session_id: &SessionId) -> Option<&CameraSession> {
        self.active_sessions.get(session_id)
    }

    /// Returns whether the monitoring backend is currently available.
    #[must_use]
    pub fn observer_available(&self) -> bool {
        self.observer_availability.is_available()
    }

    /// Returns the current observer availability category.
    #[must_use]
    pub fn observer_availability(&self) -> ObserverAvailability {
        self.observer_availability
    }

    #[must_use]
    pub fn observer_status_detail(&self) -> &str {
        &self.observer_status_detail
    }

    #[must_use]
    pub fn suppression_diagnostics(&self) -> SuppressionDiagnostics {
        self.suppression_diagnostics
    }

    /// Returns a cloned public snapshot ordered by session identifier.
    #[must_use]
    pub fn snapshot(&self) -> MonitorSnapshot {
        MonitorSnapshot {
            active_sessions: self.active_sessions.values().cloned().collect(),
            observer_availability: self.observer_availability,
            observer_status_detail: self.observer_status_detail.clone(),
            suppression_diagnostics: self.suppression_diagnostics,
        }
    }

    fn insert_or_replace(&mut self, session: CameraSession) -> bool {
        if self.active_sessions.get(&session.id) == Some(&session) {
            return false;
        }

        self.active_sessions.insert(session.id.clone(), session);
        true
    }

    fn update_existing(&mut self, session: CameraSession) -> bool {
        let Some(existing) = self.active_sessions.get_mut(&session.id) else {
            return false;
        };

        if existing == &session {
            return false;
        }

        *existing = session;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::MonitorState;
    use crate::{
        ApplicationIdentity, CameraDevice, CameraSession, CameraSessionState, DeviceId,
        MonitorEvent, ObserverAvailability, SessionId, SuppressionDiagnostics,
    };

    fn session(id: &str, app_name: &str, device_name: &str) -> CameraSession {
        CameraSession {
            id: SessionId::new(id).unwrap(),
            application: ApplicationIdentity {
                pid: None,
                app_id: None,
                display_name: app_name.to_owned(),
                binary: None,
            },
            device: CameraDevice {
                id: DeviceId::new(format!("device-{id}")).unwrap(),
                display_name: device_name.to_owned(),
                node_name: None,
            },
            process_start_time_ticks: 10,
            thread_group_id: 42,
            capture_file_descriptor: 3,
            started_at_monotonic_ns: 1_000,
            last_observed_at_monotonic_ns: 1_000,
            state: CameraSessionState::Active,
        }
    }

    #[test]
    fn start_event_adds_one_session() {
        let mut state = MonitorState::new();
        let expected = session("one", "Camera App", "Front Camera");

        assert!(state.apply(MonitorEvent::SessionStarted(expected.clone())));
        assert_eq!(state.active_session_count(), 1);
        assert_eq!(state.active_session(&expected.id), Some(&expected));
        assert!(state.active());
    }

    #[test]
    fn duplicate_start_remains_one_session() {
        let mut state = MonitorState::new();
        let event = MonitorEvent::SessionStarted(session("one", "Camera App", "Front Camera"));

        assert!(state.apply(event.clone()));
        assert!(!state.apply(event));
        assert_eq!(state.active_session_count(), 1);
    }

    #[test]
    fn stop_removes_the_matching_session() {
        let mut state = MonitorState::new();
        let active_session = session("one", "Camera App", "Front Camera");
        state.apply(MonitorEvent::SessionStarted(active_session.clone()));

        assert!(state.apply(MonitorEvent::SessionStopped(active_session.id.clone())));
        assert_eq!(state.active_session(&active_session.id), None);
        assert!(!state.active());
        assert!(!state.apply(MonitorEvent::SessionStopped(active_session.id)));
    }

    #[test]
    fn unknown_stop_is_harmless() {
        let mut state = MonitorState::new();

        assert!(!state.apply(MonitorEvent::SessionStopped(
            SessionId::new("unknown").unwrap()
        )));
        assert_eq!(state, MonitorState::new());
    }

    #[test]
    fn update_enriches_metadata_without_changing_identity() {
        let mut state = MonitorState::new();
        let initial = session("one", "Unknown application", "Camera");
        state.apply(MonitorEvent::SessionStarted(initial.clone()));

        let mut enriched = initial;
        enriched.application.pid = Some(42);
        enriched.application.app_id = Some(String::from("org.example.Camera"));
        enriched.application.display_name = String::from("Example Camera");
        enriched.device.node_name = Some(String::from("v4l2_input.usb-camera"));

        assert!(state.apply(MonitorEvent::SessionUpdated(enriched.clone())));
        assert_eq!(state.active_session(&enriched.id), Some(&enriched));
        assert_eq!(state.active_session_count(), 1);
        assert!(!state.apply(MonitorEvent::SessionUpdated(enriched)));
    }

    #[test]
    fn unknown_update_is_harmless() {
        let mut state = MonitorState::new();

        assert!(!state.apply(MonitorEvent::SessionUpdated(session(
            "unknown",
            "Camera App",
            "Camera"
        ))));
        assert_eq!(state, MonitorState::new());
    }

    #[test]
    fn start_replaces_changed_metadata_for_the_same_identity() {
        let mut state = MonitorState::new();
        let initial = session("one", "Unknown application", "Camera");
        state.apply(MonitorEvent::SessionStarted(initial.clone()));

        let mut replacement = initial;
        replacement.application.display_name = String::from("Resolved application");

        assert!(state.apply(MonitorEvent::SessionStarted(replacement.clone())));
        assert_eq!(state.active_session(&replacement.id), Some(&replacement));
        assert_eq!(state.active_session_count(), 1);
    }

    #[test]
    fn multiple_simultaneous_sessions_are_supported() {
        let mut state = MonitorState::new();
        state.apply(MonitorEvent::SessionStarted(session(
            "one",
            "First App",
            "Front Camera",
        )));
        state.apply(MonitorEvent::SessionStarted(session(
            "two",
            "Second App",
            "Rear Camera",
        )));

        assert_eq!(state.active_session_count(), 2);
        assert!(state.active());
    }

    #[test]
    fn snapshot_ordering_is_deterministic() {
        let mut state = MonitorState::new();
        for id in ["session-c", "session-a", "session-b"] {
            state.apply(MonitorEvent::SessionStarted(session(id, id, "Camera")));
        }

        let ordered_ids: Vec<_> = state
            .snapshot()
            .active_sessions
            .into_iter()
            .map(|active_session| active_session.id.into_inner())
            .collect();

        assert_eq!(ordered_ids, ["session-a", "session-b", "session-c"]);
    }

    #[test]
    fn active_becomes_false_only_after_the_final_session_stops() {
        let mut state = MonitorState::new();
        let first = session("one", "First App", "Camera");
        let second = session("two", "Second App", "Camera");
        state.apply(MonitorEvent::SessionStarted(first.clone()));
        state.apply(MonitorEvent::SessionStarted(second.clone()));

        state.apply(MonitorEvent::SessionStopped(first.id));
        assert!(state.active());

        state.apply(MonitorEvent::SessionStopped(second.id));
        assert!(!state.active());
    }

    #[test]
    fn reapplying_an_identical_event_sequence_produces_the_same_state() {
        let events = [
            MonitorEvent::SessionStarted(session("one", "Unknown", "Camera")),
            MonitorEvent::SessionUpdated(session("one", "Resolved", "Camera")),
            MonitorEvent::ObserverAvailabilityChanged {
                availability: ObserverAvailability::BackendLost,
                detail: String::from("observer restarting"),
            },
        ];
        let mut state = MonitorState::new();
        for event in events.iter().cloned() {
            state.apply(event);
        }
        let once = state.clone();

        for event in events {
            state.apply(event);
        }

        assert_eq!(state, once);
    }

    #[test]
    fn duplicate_starts_never_change_session_count() {
        for duplicate_count in [1, 2, 5, 20] {
            let mut state = MonitorState::new();
            let event = MonitorEvent::SessionStarted(session("one", "App", "Camera"));

            for _ in 0..duplicate_count {
                state.apply(event.clone());
            }

            assert_eq!(state.active_session_count(), 1);
        }
    }

    #[test]
    fn observer_availability_events_are_idempotent() {
        let mut state = MonitorState::new();
        let unavailable = MonitorEvent::ObserverAvailabilityChanged {
            availability: ObserverAvailability::BackendLost,
            detail: String::from("connection lost"),
        };

        assert!(state.apply(unavailable.clone()));
        assert!(!state.apply(unavailable));
        assert!(!state.observer_available());
        assert_eq!(state.observer_status_detail(), "connection lost");

        let recovered = MonitorEvent::ObserverAvailabilityChanged {
            availability: ObserverAvailability::Available,
            detail: String::new(),
        };
        assert!(state.apply(recovered.clone()));
        assert!(!state.apply(recovered));
        assert!(state.observer_available());
        assert_eq!(state.observer_status_detail(), "");
    }

    #[test]
    fn snapshot_derives_active_state_and_count() {
        let mut state = MonitorState::new();
        let empty = state.snapshot();
        assert!(!empty.active());
        assert_eq!(empty.active_session_count(), 0);

        state.apply(MonitorEvent::SessionStarted(session(
            "one", "App", "Camera",
        )));
        let active = state.snapshot();
        assert!(active.active());
        assert_eq!(active.active_session_count(), 1);
    }

    #[test]
    fn suppression_diagnostics_are_non_session_state() {
        let mut state = MonitorState::new();
        let diagnostics = SuppressionDiagnostics {
            suppressed_broker_events: 2,
            suppressed_unknown_events: 1,
        };
        assert!(state.apply(MonitorEvent::SuppressionDiagnosticsChanged(diagnostics)));
        assert_eq!(state.suppression_diagnostics(), diagnostics);
        assert!(!state.active());
        assert_eq!(state.active_session_count(), 0);
    }
}
