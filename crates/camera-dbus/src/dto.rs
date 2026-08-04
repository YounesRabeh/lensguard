use camera_core::{CameraSession, DetectionBackend, MonitorSnapshot};
use serde::{Deserialize, Serialize};
use zbus::zvariant::Type;

/// Stable D-Bus representation of one active camera relationship.
///
/// The field order is part of the public wire contract and has signature `(sssssstu)`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, Type)]
#[zvariant(crate = "zbus::zvariant")]
pub struct SessionDto {
    pub session_id: String,
    pub application_id: String,
    pub application_name: String,
    pub device_id: String,
    pub device_name: String,
    pub backend: String,
    pub started_at_unix_ms: u64,
    pub process_id: u32,
}

impl From<&CameraSession> for SessionDto {
    fn from(session: &CameraSession) -> Self {
        Self {
            session_id: session.id.as_str().to_owned(),
            application_id: session.application.app_id.clone().unwrap_or_default(),
            application_name: session.application.display_name.clone(),
            device_id: session.device.id.as_str().to_owned(),
            device_name: session.device.display_name.clone(),
            backend: match &session.backend {
                DetectionBackend::PipeWire => String::from("pipewire"),
                DetectionBackend::Other(name) => name.clone(),
            },
            started_at_unix_ms: session.started_at_unix_ms,
            process_id: session.application.pid.unwrap_or(0),
        }
    }
}

pub(crate) fn snapshot_sessions(snapshot: &MonitorSnapshot) -> Vec<SessionDto> {
    let mut sessions = snapshot
        .active_sessions
        .iter()
        .map(SessionDto::from)
        .collect::<Vec<_>>();
    sessions.sort_by(|left, right| left.session_id.cmp(&right.session_id));
    sessions
}

#[cfg(test)]
mod tests {
    use camera_core::{
        ApplicationIdentity, CameraDevice, CameraSession, DetectionBackend, DeviceId, SessionId,
    };
    use zbus::zvariant::Type;

    use super::SessionDto;

    #[test]
    fn converts_domain_fields_in_stable_wire_order() {
        let session = CameraSession {
            id: SessionId::new("stable-session").unwrap(),
            application: ApplicationIdentity {
                pid: Some(4200),
                app_id: Some(String::from("org.example.Camera")),
                display_name: String::from("Example Camera"),
                binary: Some(String::from("camera-bin")),
            },
            device: CameraDevice {
                id: DeviceId::new("stable-device").unwrap(),
                display_name: String::from("Front Camera"),
                node_name: Some(String::from("raw-node-name-is-not-exported")),
            },
            started_at_unix_ms: 1_234,
            backend: DetectionBackend::PipeWire,
        };

        let dto = SessionDto::from(&session);
        assert_eq!(SessionDto::SIGNATURE, "(sssssstu)");
        assert_eq!(dto.session_id, "stable-session");
        assert_eq!(dto.application_id, "org.example.Camera");
        assert_eq!(dto.application_name, "Example Camera");
        assert_eq!(dto.device_id, "stable-device");
        assert_eq!(dto.device_name, "Front Camera");
        assert_eq!(dto.backend, "pipewire");
        assert_eq!(dto.started_at_unix_ms, 1_234);
        assert_eq!(dto.process_id, 4_200);
    }

    #[test]
    fn absent_optional_values_have_documented_sentinels() {
        let session = CameraSession {
            id: SessionId::new("fallback").unwrap(),
            application: ApplicationIdentity {
                pid: None,
                app_id: None,
                display_name: String::from("Unknown application"),
                binary: None,
            },
            device: CameraDevice {
                id: DeviceId::new("device").unwrap(),
                display_name: String::from("Camera"),
                node_name: None,
            },
            started_at_unix_ms: 0,
            backend: DetectionBackend::Other(String::from("future-backend")),
        };

        let dto = SessionDto::from(&session);
        assert!(dto.application_id.is_empty());
        assert_eq!(dto.process_id, 0);
        assert_eq!(dto.backend, "future-backend");
    }
}
