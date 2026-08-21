use std::collections::VecDeque;
use std::io::{self, Read, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::time::Duration;

use camera_core::{
    CaptureTransition, MAX_OBSERVER_MESSAGE_SIZE, MonitorEvent, OBSERVER_SCHEMA_VERSION,
    ObserverAvailability, ObserverClientMessage, ObserverMessage, V4l2SessionMachine,
};
use thiserror::Error;

pub const OBSERVER_SOCKET_PATH: &str = "/run/lensguard/v4l2-observer.sock";

#[derive(Debug, Error)]
pub enum ObserverError {
    #[error("failed to connect to the V4L2 observer at {path}: {source}")]
    Connect { path: String, source: io::Error },
    #[error("observer I/O failed: {0}")]
    Io(#[from] io::Error),
    #[error("observer message is malformed: {0}")]
    Json(#[from] serde_json::Error),
    #[error("observer message is larger than {MAX_OBSERVER_MESSAGE_SIZE} bytes")]
    OversizedMessage,
    #[error("observer protocol handshake is invalid")]
    InvalidHandshake,
    #[error("observer event was rejected: {0}")]
    RejectedEvent(#[from] camera_core::EventValidationError),
}

impl ObserverError {
    #[must_use]
    pub fn availability(&self) -> ObserverAvailability {
        match self {
            Self::Connect { source, .. } if source.kind() == io::ErrorKind::NotFound => {
                ObserverAvailability::NotInstalled
            }
            Self::InvalidHandshake => ObserverAvailability::VersionMismatch,
            Self::Connect { .. }
            | Self::Io(_)
            | Self::Json(_)
            | Self::OversizedMessage
            | Self::RejectedEvent(_) => ObserverAvailability::ConnectionFailed,
        }
    }
}

pub struct ObserverEventSource {
    stream: UnixStream,
    sessions: V4l2SessionMachine,
    pending: VecDeque<MonitorEvent>,
}

impl ObserverEventSource {
    /// Connects to the fixed production observer socket and negotiates protocol versions.
    ///
    /// # Errors
    ///
    /// Returns a typed connection, framing, serialization, or version error.
    pub fn connect() -> Result<Self, ObserverError> {
        Self::connect_to(OBSERVER_SOCKET_PATH)
    }

    /// Connects to an explicit socket path, primarily for synthetic integration tests.
    ///
    /// # Errors
    ///
    /// Returns a typed connection, framing, serialization, or version error.
    pub fn connect_to(path: impl AsRef<Path>) -> Result<Self, ObserverError> {
        let path = path.as_ref();
        let mut stream = UnixStream::connect(path).map_err(|source| ObserverError::Connect {
            path: path.display().to_string(),
            source,
        })?;
        stream.set_write_timeout(Some(Duration::from_secs(2)))?;
        write_message(
            &mut stream,
            &ObserverClientMessage::Hello {
                schema_version: OBSERVER_SCHEMA_VERSION,
                service_version: camera_core::VERSION.to_owned(),
            },
        )?;
        stream.set_read_timeout(Some(Duration::from_secs(2)))?;
        let hello: ObserverMessage = read_message(&mut stream)?;
        if !matches!(hello, ObserverMessage::Hello { schema_version: OBSERVER_SCHEMA_VERSION, ref observer_version } if observer_version == camera_core::VERSION)
        {
            return Err(ObserverError::InvalidHandshake);
        }
        Ok(Self {
            stream,
            sessions: V4l2SessionMachine::new(),
            pending: VecDeque::new(),
        })
    }

    /// Waits up to `timeout` for one validated and translated domain event.
    ///
    /// # Errors
    ///
    /// Returns a framing, I/O, serialization, or event-validation error.
    pub fn next_event_timeout(
        &mut self,
        timeout: Duration,
    ) -> Result<Option<MonitorEvent>, ObserverError> {
        if let Some(event) = self.pending.pop_front() {
            return Ok(Some(event));
        }
        self.stream.set_read_timeout(Some(timeout))?;
        let message = match read_message(&mut self.stream) {
            Ok(message) => message,
            Err(ObserverError::Io(error))
                if matches!(
                    error.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                ) =>
            {
                return Ok(None);
            }
            Err(error) => return Err(error),
        };
        self.translate(message)?;
        Ok(self.pending.pop_front())
    }

    fn translate(&mut self, message: ObserverMessage) -> Result<(), ObserverError> {
        match message {
            ObserverMessage::Hello { .. } => return Err(ObserverError::InvalidHandshake),
            ObserverMessage::Availability {
                availability,
                detail,
            } => {
                if !availability.is_available() {
                    queue_ended(&mut self.pending, self.sessions.backend_lost());
                }
                self.pending
                    .push_back(MonitorEvent::ObserverAvailabilityChanged {
                        availability,
                        detail,
                    });
            }
            ObserverMessage::Diagnostics { diagnostics } => self
                .pending
                .push_back(MonitorEvent::SuppressionDiagnosticsChanged(diagnostics)),
            ObserverMessage::Capture { event } => {
                queue_ended_or_started(&mut self.pending, self.sessions.apply(&event)?);
            }
        }
        Ok(())
    }
}

fn queue_ended_or_started(events: &mut VecDeque<MonitorEvent>, transition: CaptureTransition) {
    match transition {
        CaptureTransition::None => {}
        CaptureTransition::Started(session) => {
            events.push_back(MonitorEvent::SessionStarted(session));
        }
        CaptureTransition::Ended { session, .. } => {
            events.push_back(MonitorEvent::SessionStopped(session.id));
        }
        CaptureTransition::EndedMany(ended) => events.extend(
            ended
                .into_iter()
                .map(|(session, _)| MonitorEvent::SessionStopped(session.id)),
        ),
    }
}

fn queue_ended(events: &mut VecDeque<MonitorEvent>, transition: CaptureTransition) {
    queue_ended_or_started(events, transition);
}

/// Writes one bounded, length-prefixed observer protocol message.
///
/// # Errors
///
/// Returns an I/O, serialization, length-conversion, or size-limit error.
pub fn write_message<T: serde::Serialize>(
    writer: &mut impl Write,
    value: &T,
) -> Result<(), ObserverError> {
    let payload = serde_json::to_vec(value)?;
    if payload.len() > MAX_OBSERVER_MESSAGE_SIZE {
        return Err(ObserverError::OversizedMessage);
    }
    let length = u32::try_from(payload.len()).map_err(|_| ObserverError::OversizedMessage)?;
    writer.write_all(&length.to_be_bytes())?;
    writer.write_all(&payload)?;
    writer.flush()?;
    Ok(())
}

/// Reads one bounded, length-prefixed observer protocol message.
///
/// # Errors
///
/// Returns an I/O, deserialization, length-conversion, or size-limit error.
pub fn read_message<T: serde::de::DeserializeOwned>(
    reader: &mut impl Read,
) -> Result<T, ObserverError> {
    let mut length = [0_u8; 4];
    reader.read_exact(&mut length)?;
    let length =
        usize::try_from(u32::from_be_bytes(length)).map_err(|_| ObserverError::OversizedMessage)?;
    if length == 0 || length > MAX_OBSERVER_MESSAGE_SIZE {
        return Err(ObserverError::OversizedMessage);
    }
    let mut payload = vec![0; length];
    reader.read_exact(&mut payload)?;
    Ok(serde_json::from_slice(&payload)?)
}

#[cfg(test)]
mod tests {
    use std::os::unix::net::UnixListener;
    use std::path::PathBuf;
    use std::thread;
    use std::time::Duration;

    use camera_core::{
        CaptureConfidence, CaptureOwner, DirectCaptureEvent, DirectCaptureOperation, MonitorEvent,
        OBSERVER_SCHEMA_VERSION, ObserverAvailability, ObserverClientMessage, ObserverMessage,
        PhysicalCameraId,
    };

    use super::{ObserverError, ObserverEventSource, read_message, write_message};

    fn event(
        owner: CaptureOwner,
        operation: DirectCaptureOperation,
        timestamp: u64,
    ) -> DirectCaptureEvent {
        DirectCaptureEvent {
            schema_version: OBSERVER_SCHEMA_VERSION,
            timestamp_monotonic_ns: timestamp,
            process_id: 42,
            thread_group_id: 42,
            process_start_time_ticks: 10,
            file_descriptor: 3,
            device: PhysicalCameraId {
                major: 81,
                minor: 0,
                ..PhysicalCameraId::default()
            },
            operation,
            result: 0,
            confidence: CaptureConfidence::Confirmed,
            owner,
        }
    }

    fn socket_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("lensguard-{name}-{}.sock", std::process::id()))
    }

    fn spawn_server(path: &PathBuf, messages: Vec<ObserverMessage>) -> thread::JoinHandle<()> {
        let listener = UnixListener::bind(path).unwrap();
        thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let hello: ObserverClientMessage = read_message(&mut stream).unwrap();
            assert!(matches!(
                hello,
                ObserverClientMessage::Hello {
                    schema_version: OBSERVER_SCHEMA_VERSION,
                    ..
                }
            ));
            write_message(
                &mut stream,
                &ObserverMessage::Hello {
                    schema_version: OBSERVER_SCHEMA_VERSION,
                    observer_version: camera_core::VERSION.to_owned(),
                },
            )
            .unwrap();
            for message in messages {
                write_message(&mut stream, &message).unwrap();
            }
        })
    }

    #[test]
    fn synthetic_direct_start_and_stop_become_monitor_events() {
        let path = socket_path("direct");
        let server = spawn_server(
            &path,
            vec![
                ObserverMessage::Availability {
                    availability: ObserverAvailability::Available,
                    detail: String::new(),
                },
                ObserverMessage::Capture {
                    event: Box::new(event(
                        CaptureOwner::Direct,
                        DirectCaptureOperation::StreamStarted,
                        1,
                    )),
                },
                ObserverMessage::Capture {
                    event: Box::new(event(
                        CaptureOwner::Direct,
                        DirectCaptureOperation::StreamStopped,
                        2,
                    )),
                },
            ],
        );
        let mut source = ObserverEventSource::connect_to(&path).unwrap();
        assert!(matches!(
            source.next_event_timeout(Duration::from_secs(1)).unwrap(),
            Some(MonitorEvent::ObserverAvailabilityChanged {
                availability: ObserverAvailability::Available,
                ..
            })
        ));
        assert!(matches!(
            source.next_event_timeout(Duration::from_secs(1)).unwrap(),
            Some(MonitorEvent::SessionStarted(_))
        ));
        assert!(matches!(
            source.next_event_timeout(Duration::from_secs(1)).unwrap(),
            Some(MonitorEvent::SessionStopped(_))
        ));
        server.join().unwrap();
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn synthetic_non_direct_event_is_rejected() {
        let path = socket_path("rejected");
        let server = spawn_server(
            &path,
            vec![ObserverMessage::Capture {
                event: Box::new(event(
                    CaptureOwner::BrokerOwned,
                    DirectCaptureOperation::StreamStarted,
                    1,
                )),
            }],
        );
        let mut source = ObserverEventSource::connect_to(&path).unwrap();
        assert!(matches!(
            source.next_event_timeout(Duration::from_secs(1)),
            Err(ObserverError::RejectedEvent(_))
        ));
        server.join().unwrap();
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn oversized_frame_is_rejected_before_allocation() {
        let bytes =
            (u32::try_from(camera_core::MAX_OBSERVER_MESSAGE_SIZE).unwrap() + 1).to_be_bytes();
        assert!(matches!(
            read_message::<ObserverMessage>(&mut bytes.as_slice()),
            Err(ObserverError::OversizedMessage)
        ));
    }

    #[test]
    fn unknown_protocol_version_is_rejected() {
        let path = socket_path("version");
        let listener = UnixListener::bind(&path).unwrap();
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let _: ObserverClientMessage = read_message(&mut stream).unwrap();
            write_message(
                &mut stream,
                &ObserverMessage::Hello {
                    schema_version: 99,
                    observer_version: camera_core::VERSION.to_owned(),
                },
            )
            .unwrap();
        });
        assert!(matches!(
            ObserverEventSource::connect_to(&path),
            Err(ObserverError::InvalidHandshake)
        ));
        server.join().unwrap();
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn absent_socket_reports_not_installed() {
        let path = socket_path("absent");
        let Err(error) = ObserverEventSource::connect_to(path) else {
            panic!("absent observer socket unexpectedly connected");
        };
        assert_eq!(error.availability(), ObserverAvailability::NotInstalled);
    }
}
