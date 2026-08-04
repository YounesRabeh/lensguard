use thiserror::Error;
use zbus::DBusError;

/// Typed errors returned to D-Bus callers.
#[derive(Debug, DBusError)]
#[zbus(prefix = "io.github.younesrabeh.CameraMonitor.Error")]
pub enum CameraMonitorError {
    Internal(String),
    #[zbus(error)]
    ZBus(zbus::Error),
}

/// Failures while publishing or updating the D-Bus service.
#[derive(Debug, Error)]
pub enum ServiceError {
    #[error("D-Bus service operation failed: {0}")]
    ZBus(#[from] zbus::Error),
}

#[cfg(test)]
mod tests {
    use zbus::DBusError;

    use super::CameraMonitorError;

    #[test]
    fn api_errors_have_a_stable_useful_name() {
        let error = CameraMonitorError::Internal(String::from("state unavailable"));
        assert_eq!(
            error.name().as_str(),
            "io.github.younesrabeh.CameraMonitor.Error.Internal"
        );
        assert!(error.description().unwrap().contains("state unavailable"));
    }
}
