use std::collections::BTreeMap;
use std::fmt::{self, Display, Formatter};

use serde::{Deserialize, Serialize};

use crate::{
    ApplicationIdentity, CameraDevice, CameraSession, CameraSessionState, DeviceId, SessionId,
};

pub const OBSERVER_SCHEMA_VERSION: u16 = 1;
pub const MAX_OBSERVER_MESSAGE_SIZE: usize = 16 * 1024;
const MAX_IDENTITY_FIELD_BYTES: usize = 512;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CaptureOwner {
    Direct,
    BrokerOwned,
    Unknown,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CaptureConfidence {
    Confirmed,
    OpenOnly,
    Unknown,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DirectCaptureOperation {
    DeviceOpened,
    StreamStarted,
    StreamStopped,
    CaptureRead,
    DeviceClosed,
    ProcessExited,
    DeviceRemoved,
    BackendLost,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PhysicalCameraId {
    pub major: u32,
    pub minor: u32,
    pub udev_syspath: Option<String>,
    pub media_device: Option<String>,
    pub serial: Option<String>,
    pub hardware_path: Option<String>,
    pub vendor_id: Option<String>,
    pub product_id: Option<String>,
    pub bus_info: Option<String>,
    pub driver: Option<String>,
    pub display_name: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DirectCaptureEvent {
    pub schema_version: u16,
    pub timestamp_monotonic_ns: u64,
    pub process_id: u32,
    pub thread_group_id: u32,
    pub process_start_time_ticks: u64,
    pub file_descriptor: i32,
    pub device: PhysicalCameraId,
    pub operation: DirectCaptureOperation,
    pub result: i64,
    pub confidence: CaptureConfidence,
    pub owner: CaptureOwner,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ObserverAvailability {
    Available,
    Disabled,
    NotInstalled,
    UnsupportedKernel,
    MissingCapability,
    BlockedByPolicy,
    VersionMismatch,
    ConnectionFailed,
    BackendLost,
}

impl ObserverAvailability {
    #[must_use]
    pub const fn is_available(self) -> bool {
        matches!(self, Self::Available)
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Available => "available",
            Self::Disabled => "disabled",
            Self::NotInstalled => "not-installed",
            Self::UnsupportedKernel => "unsupported-kernel",
            Self::MissingCapability => "missing-capability",
            Self::BlockedByPolicy => "blocked-by-policy",
            Self::VersionMismatch => "version-mismatch",
            Self::ConnectionFailed => "connection-failed",
            Self::BackendLost => "backend-lost",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SuppressionDiagnostics {
    pub suppressed_broker_events: u64,
    pub suppressed_unknown_events: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum ObserverClientMessage {
    Hello {
        schema_version: u16,
        service_version: String,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum ObserverMessage {
    Hello {
        schema_version: u16,
        observer_version: String,
    },
    Availability {
        availability: ObserverAvailability,
        detail: String,
    },
    Capture {
        event: Box<DirectCaptureEvent>,
    },
    Diagnostics {
        diagnostics: SuppressionDiagnostics,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EventValidationError {
    UnsupportedSchema(u16),
    NonDirectOwner,
    InvalidProcessIdentity,
    InvalidFileDescriptor,
    InvalidResult,
    InvalidConfidence,
    IdentityFieldTooLong,
}

impl Display for EventValidationError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedSchema(version) => {
                write!(formatter, "unsupported schema version {version}")
            }
            Self::NonDirectOwner => {
                formatter.write_str("session-capable event owner is not direct")
            }
            Self::InvalidProcessIdentity => formatter.write_str("invalid process identity"),
            Self::InvalidFileDescriptor => formatter.write_str("invalid file descriptor"),
            Self::InvalidResult => formatter.write_str("operation result cannot confirm the event"),
            Self::InvalidConfidence => formatter.write_str("operation confidence is invalid"),
            Self::IdentityFieldTooLong => {
                formatter.write_str("physical camera identity field exceeds the limit")
            }
        }
    }
}

impl std::error::Error for EventValidationError {}

impl DirectCaptureEvent {
    /// Validates untrusted observer metadata before it can affect session state.
    ///
    /// # Errors
    ///
    /// Returns the first schema, ownership, identity, result, confidence, or size violation.
    pub fn validate(&self) -> Result<(), EventValidationError> {
        if self.schema_version != OBSERVER_SCHEMA_VERSION {
            return Err(EventValidationError::UnsupportedSchema(self.schema_version));
        }
        if self.owner != CaptureOwner::Direct {
            return Err(EventValidationError::NonDirectOwner);
        }
        if self.process_id == 0 || self.thread_group_id == 0 || self.process_start_time_ticks == 0 {
            return Err(EventValidationError::InvalidProcessIdentity);
        }
        if self.file_descriptor < 0
            && !matches!(
                self.operation,
                DirectCaptureOperation::ProcessExited
                    | DirectCaptureOperation::DeviceRemoved
                    | DirectCaptureOperation::BackendLost
            )
        {
            return Err(EventValidationError::InvalidFileDescriptor);
        }
        let confirmed_result = match self.operation {
            DirectCaptureOperation::StreamStarted | DirectCaptureOperation::StreamStopped => {
                self.result == 0
            }
            DirectCaptureOperation::CaptureRead => self.result > 0,
            DirectCaptureOperation::DeviceOpened
            | DirectCaptureOperation::DeviceClosed
            | DirectCaptureOperation::ProcessExited
            | DirectCaptureOperation::DeviceRemoved
            | DirectCaptureOperation::BackendLost => self.result >= 0,
        };
        if !confirmed_result {
            return Err(EventValidationError::InvalidResult);
        }
        let expected_confidence = match self.operation {
            DirectCaptureOperation::DeviceOpened => CaptureConfidence::OpenOnly,
            DirectCaptureOperation::BackendLost => CaptureConfidence::Unknown,
            _ => CaptureConfidence::Confirmed,
        };
        if self.confidence != expected_confidence {
            return Err(EventValidationError::InvalidConfidence);
        }
        if self
            .device
            .text_fields()
            .into_iter()
            .flatten()
            .any(|field| field.len() > MAX_IDENTITY_FIELD_BYTES)
        {
            return Err(EventValidationError::IdentityFieldTooLong);
        }
        Ok(())
    }
}

impl PhysicalCameraId {
    fn text_fields(&self) -> [Option<&str>; 9] {
        [
            self.udev_syspath.as_deref(),
            self.media_device.as_deref(),
            self.serial.as_deref(),
            self.hardware_path.as_deref(),
            self.vendor_id.as_deref(),
            self.product_id.as_deref(),
            self.bus_info.as_deref(),
            self.driver.as_deref(),
            self.display_name.as_deref(),
        ]
    }

    #[must_use]
    pub fn stable_node_id(&self) -> String {
        format!("v4l2-{}:{}", self.major, self.minor)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CaptureTransition {
    None,
    Started(CameraSession),
    Ended {
        session: CameraSession,
        state: CameraSessionState,
    },
    EndedMany(Vec<(CameraSession, CameraSessionState)>),
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct CaptureKey {
    thread_group_id: u32,
    process_start_time_ticks: u64,
    file_descriptor: i32,
    major: u32,
    minor: u32,
}

#[derive(Default)]
pub struct V4l2SessionMachine {
    active: BTreeMap<CaptureKey, CameraSession>,
    last_timestamp_monotonic_ns: u64,
}

impl V4l2SessionMachine {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Applies one validated direct-capture observation.
    ///
    /// # Errors
    ///
    /// Returns [`EventValidationError`] when the event fails the strict observer contract.
    pub fn apply(
        &mut self,
        event: &DirectCaptureEvent,
    ) -> Result<CaptureTransition, EventValidationError> {
        event.validate()?;
        if event.timestamp_monotonic_ns < self.last_timestamp_monotonic_ns {
            return Ok(CaptureTransition::None);
        }
        self.last_timestamp_monotonic_ns = event.timestamp_monotonic_ns;
        let key = CaptureKey::from(event);
        match event.operation {
            DirectCaptureOperation::StreamStarted | DirectCaptureOperation::CaptureRead => {
                if let Some(session) = self.active.get_mut(&key) {
                    session.last_observed_at_monotonic_ns = event.timestamp_monotonic_ns;
                    return Ok(CaptureTransition::None);
                }
                let session = session_from_event(event)?;
                self.active.insert(key, session.clone());
                Ok(CaptureTransition::Started(session))
            }
            DirectCaptureOperation::StreamStopped | DirectCaptureOperation::DeviceClosed => {
                Ok(self.end_one(&key, CameraSessionState::Stopped))
            }
            DirectCaptureOperation::ProcessExited => Ok(self.end_matching(
                |candidate| {
                    candidate.thread_group_id == event.thread_group_id
                        && candidate.process_start_time_ticks == event.process_start_time_ticks
                },
                CameraSessionState::Stopped,
            )),
            DirectCaptureOperation::DeviceRemoved => Ok(self.end_matching(
                |candidate| {
                    candidate.major == event.device.major && candidate.minor == event.device.minor
                },
                CameraSessionState::Interrupted,
            )),
            DirectCaptureOperation::BackendLost => Ok(self.backend_lost()),
            DirectCaptureOperation::DeviceOpened => Ok(CaptureTransition::None),
        }
    }

    #[must_use]
    pub fn backend_lost(&mut self) -> CaptureTransition {
        let ended = std::mem::take(&mut self.active)
            .into_values()
            .map(|session| (session, CameraSessionState::Interrupted))
            .collect();
        CaptureTransition::EndedMany(ended)
    }

    #[must_use]
    pub fn active_session_count(&self) -> usize {
        self.active.len()
    }

    fn end_one(&mut self, key: &CaptureKey, state: CameraSessionState) -> CaptureTransition {
        self.active
            .remove(key)
            .map_or(CaptureTransition::None, |session| {
                CaptureTransition::Ended { session, state }
            })
    }

    fn end_matching(
        &mut self,
        predicate: impl Fn(&CaptureKey) -> bool,
        state: CameraSessionState,
    ) -> CaptureTransition {
        let keys: Vec<_> = self
            .active
            .keys()
            .filter(|key| predicate(key))
            .cloned()
            .collect();
        let ended = keys
            .into_iter()
            .filter_map(|key| self.active.remove(&key).map(|session| (session, state)))
            .collect();
        CaptureTransition::EndedMany(ended)
    }
}

impl From<&DirectCaptureEvent> for CaptureKey {
    fn from(event: &DirectCaptureEvent) -> Self {
        Self {
            thread_group_id: event.thread_group_id,
            process_start_time_ticks: event.process_start_time_ticks,
            file_descriptor: event.file_descriptor,
            major: event.device.major,
            minor: event.device.minor,
        }
    }
}

fn session_from_event(event: &DirectCaptureEvent) -> Result<CameraSession, EventValidationError> {
    let session_text = format!(
        "v4l2-{}-{}-{}-{}:{}-{}",
        event.thread_group_id,
        event.process_start_time_ticks,
        event.file_descriptor,
        event.device.major,
        event.device.minor,
        event.timestamp_monotonic_ns
    );
    let device_text = event.device.stable_node_id();
    Ok(CameraSession {
        id: SessionId::new(session_text)
            .map_err(|_| EventValidationError::InvalidProcessIdentity)?,
        application: ApplicationIdentity {
            pid: Some(event.process_id),
            app_id: None,
            display_name: String::from("Unknown application"),
            binary: None,
        },
        device: CameraDevice {
            id: DeviceId::new(device_text)
                .map_err(|_| EventValidationError::InvalidProcessIdentity)?,
            display_name: event
                .device
                .display_name
                .clone()
                .unwrap_or_else(|| String::from("Unknown camera")),
            node_name: event.device.udev_syspath.clone(),
        },
        process_start_time_ticks: event.process_start_time_ticks,
        thread_group_id: event.thread_group_id,
        capture_file_descriptor: event.file_descriptor,
        started_at_monotonic_ns: event.timestamp_monotonic_ns,
        last_observed_at_monotonic_ns: event.timestamp_monotonic_ns,
        state: CameraSessionState::Active,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(operation: DirectCaptureOperation, timestamp: u64) -> DirectCaptureEvent {
        DirectCaptureEvent {
            schema_version: OBSERVER_SCHEMA_VERSION,
            timestamp_monotonic_ns: timestamp,
            process_id: 42,
            thread_group_id: 42,
            process_start_time_ticks: 100,
            file_descriptor: 3,
            device: PhysicalCameraId {
                major: 81,
                minor: 0,
                display_name: Some(String::from("Test Camera")),
                ..PhysicalCameraId::default()
            },
            operation,
            result: 0,
            confidence: CaptureConfidence::Confirmed,
            owner: CaptureOwner::Direct,
        }
    }

    #[test]
    fn open_only_and_failed_stream_start_never_activate() {
        let mut machine = V4l2SessionMachine::new();
        let mut opened = event(DirectCaptureOperation::DeviceOpened, 1);
        opened.confidence = CaptureConfidence::OpenOnly;
        assert_eq!(machine.apply(&opened).unwrap(), CaptureTransition::None);

        let mut failed = event(DirectCaptureOperation::StreamStarted, 2);
        failed.result = -16;
        assert_eq!(
            machine.apply(&failed),
            Err(EventValidationError::InvalidResult)
        );
        assert_eq!(machine.active_session_count(), 0);
    }

    #[test]
    fn non_direct_and_unknown_events_are_rejected() {
        let mut machine = V4l2SessionMachine::new();
        for owner in [CaptureOwner::BrokerOwned, CaptureOwner::Unknown] {
            let mut candidate = event(DirectCaptureOperation::StreamStarted, 1);
            candidate.owner = owner;
            assert_eq!(
                machine.apply(&candidate),
                Err(EventValidationError::NonDirectOwner)
            );
        }
        assert_eq!(machine.active_session_count(), 0);
    }

    #[test]
    fn successful_start_is_idempotent_and_stop_ends_it() {
        let mut machine = V4l2SessionMachine::new();
        assert!(matches!(
            machine
                .apply(&event(DirectCaptureOperation::StreamStarted, 1))
                .unwrap(),
            CaptureTransition::Started(_)
        ));
        assert_eq!(
            machine
                .apply(&event(DirectCaptureOperation::StreamStarted, 2))
                .unwrap(),
            CaptureTransition::None
        );
        assert_eq!(machine.active_session_count(), 1);
        assert!(matches!(
            machine
                .apply(&event(DirectCaptureOperation::StreamStopped, 3))
                .unwrap(),
            CaptureTransition::Ended {
                state: CameraSessionState::Stopped,
                ..
            }
        ));
        assert_eq!(machine.active_session_count(), 0);
    }

    #[test]
    fn close_process_exit_device_removal_and_backend_loss_end_sessions() {
        for (operation, expected) in [
            (
                DirectCaptureOperation::DeviceClosed,
                CameraSessionState::Stopped,
            ),
            (
                DirectCaptureOperation::ProcessExited,
                CameraSessionState::Stopped,
            ),
            (
                DirectCaptureOperation::DeviceRemoved,
                CameraSessionState::Interrupted,
            ),
            (
                DirectCaptureOperation::BackendLost,
                CameraSessionState::Interrupted,
            ),
        ] {
            let mut machine = V4l2SessionMachine::new();
            machine
                .apply(&event(DirectCaptureOperation::StreamStarted, 1))
                .unwrap();
            let mut ending = event(operation, 2);
            if operation == DirectCaptureOperation::BackendLost {
                ending.confidence = CaptureConfidence::Unknown;
            }
            let transition = machine.apply(&ending).unwrap();
            match transition {
                CaptureTransition::Ended { state, .. } => assert_eq!(state, expected),
                CaptureTransition::EndedMany(values) => {
                    assert_eq!(values.len(), 1);
                    assert_eq!(values[0].1, expected);
                }
                other => panic!("unexpected transition: {other:?}"),
            }
            assert_eq!(machine.active_session_count(), 0);
        }
    }

    #[test]
    fn out_of_order_stop_and_pid_reuse_are_harmless() {
        let mut machine = V4l2SessionMachine::new();
        assert_eq!(
            machine
                .apply(&event(DirectCaptureOperation::StreamStopped, 2))
                .unwrap(),
            CaptureTransition::None
        );
        machine
            .apply(&event(DirectCaptureOperation::StreamStarted, 3))
            .unwrap();
        let mut reused = event(DirectCaptureOperation::ProcessExited, 4);
        reused.process_start_time_ticks = 101;
        assert_eq!(
            machine.apply(&reused).unwrap(),
            CaptureTransition::EndedMany(Vec::new())
        );
        assert_eq!(machine.active_session_count(), 1);
    }

    #[test]
    fn unknown_schema_and_oversized_identity_are_rejected() {
        let mut unknown = event(DirectCaptureOperation::StreamStarted, 1);
        unknown.schema_version = 99;
        assert_eq!(
            unknown.validate(),
            Err(EventValidationError::UnsupportedSchema(99))
        );

        let mut oversized = event(DirectCaptureOperation::StreamStarted, 1);
        oversized.device.serial = Some("x".repeat(MAX_IDENTITY_FIELD_BYTES + 1));
        assert_eq!(
            oversized.validate(),
            Err(EventValidationError::IdentityFieldTooLong)
        );
    }
}
