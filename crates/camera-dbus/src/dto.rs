use camera_core::{CameraSession, MonitorSnapshot};
use serde::{Deserialize, Serialize};
use zbus::zvariant::Type;

/// Stable D-Bus representation of one active camera relationship.
///
/// The field order is part of the public wire contract and has signature `(ssssstu)`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, Type)]
#[zvariant(crate = "zbus::zvariant")]
pub struct SessionDto {
    pub session_id: String,
    pub application_id: String,
    pub application_name: String,
    pub device_id: String,
    pub device_name: String,
    pub started_at_monotonic_ns: u64,
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
            started_at_monotonic_ns: session.started_at_monotonic_ns,
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
        ApplicationIdentity, CameraDevice, CameraSession, CameraSessionState, DeviceId, SessionId,
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
            process_start_time_ticks: 10,
            thread_group_id: 4_200,
            capture_file_descriptor: 3,
            started_at_monotonic_ns: 1_234,
            last_observed_at_monotonic_ns: 1_234,
            state: CameraSessionState::Active,
        };

        let dto = SessionDto::from(&session);
        assert_eq!(SessionDto::SIGNATURE, "(ssssstu)");
        assert_eq!(dto.session_id, "stable-session");
        assert_eq!(dto.application_id, "org.example.Camera");
        assert_eq!(dto.application_name, "Example Camera");
        assert_eq!(dto.device_id, "stable-device");
        assert_eq!(dto.device_name, "Front Camera");
        assert_eq!(dto.started_at_monotonic_ns, 1_234);
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
            process_start_time_ticks: 1,
            thread_group_id: 1,
            capture_file_descriptor: 0,
            started_at_monotonic_ns: 0,
            last_observed_at_monotonic_ns: 0,
            state: CameraSessionState::Active,
        };

        let dto = SessionDto::from(&session);
        assert!(dto.application_id.is_empty());
        assert_eq!(dto.process_id, 0);
    }
}
