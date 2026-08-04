use std::fmt::{self, Display, Formatter};

use crate::{DomainError, IdentifierKind};

/// Stable, backend-independent identity for a camera session.
///
/// Raw `PipeWire` object identifiers must never be exposed directly as this value. An adapter is
/// responsible for deriving an identity that remains stable for the lifetime of a relationship.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SessionId(String);

impl SessionId {
    /// Creates a session identifier, rejecting empty and whitespace-only values.
    ///
    /// # Errors
    ///
    /// Returns [`DomainError::EmptyIdentifier`] when `value` contains no visible characters.
    pub fn new(value: impl Into<String>) -> Result<Self, DomainError> {
        let value = value.into();
        validate_identifier(&value, IdentifierKind::Session)?;
        Ok(Self(value))
    }

    /// Returns the identifier as text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Consumes the identifier and returns its text.
    #[must_use]
    pub fn into_inner(self) -> String {
        self.0
    }
}

impl Display for SessionId {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl TryFrom<String> for SessionId {
    type Error = DomainError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl TryFrom<&str> for SessionId {
    type Error = DomainError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

/// Stable, backend-independent identity for a camera device.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DeviceId(String);

impl DeviceId {
    /// Creates a device identifier, rejecting empty and whitespace-only values.
    ///
    /// # Errors
    ///
    /// Returns [`DomainError::EmptyIdentifier`] when `value` contains no visible characters.
    pub fn new(value: impl Into<String>) -> Result<Self, DomainError> {
        let value = value.into();
        validate_identifier(&value, IdentifierKind::Device)?;
        Ok(Self(value))
    }

    /// Returns the identifier as text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Consumes the identifier and returns its text.
    #[must_use]
    pub fn into_inner(self) -> String {
        self.0
    }
}

impl Display for DeviceId {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl TryFrom<String> for DeviceId {
    type Error = DomainError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl TryFrom<&str> for DeviceId {
    type Error = DomainError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

fn validate_identifier(value: &str, kind: IdentifierKind) -> Result<(), DomainError> {
    if value.trim().is_empty() {
        Err(DomainError::EmptyIdentifier { kind })
    } else {
        Ok(())
    }
}

/// A camera device safe to expose to domain consumers.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CameraDevice {
    pub id: DeviceId,
    pub display_name: String,
    pub node_name: Option<String>,
}

/// Application information associated with a camera stream.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ApplicationIdentity {
    pub pid: Option<u32>,
    pub app_id: Option<String>,
    pub display_name: String,
    pub binary: Option<String>,
}

/// The adapter family that detected a camera session.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DetectionBackend {
    /// A session inferred from the `PipeWire` graph.
    PipeWire,
    /// A future backend not known to this crate version.
    Other(String),
}

/// One active relationship between an application and a camera device.
///
/// Session identity is defined exclusively by [`id`](Self::id). Updated metadata replaces the
/// remaining fields without changing that identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CameraSession {
    pub id: SessionId,
    pub application: ApplicationIdentity,
    pub device: CameraDevice,
    pub started_at_unix_ms: u64,
    pub backend: DetectionBackend,
}

#[cfg(test)]
mod tests {
    use super::{DeviceId, SessionId};
    use crate::{DomainError, IdentifierKind};

    #[test]
    fn typed_identifiers_preserve_valid_values() {
        let session_id = SessionId::new("session-42").unwrap();
        let device_id = DeviceId::new(String::from("camera-front")).unwrap();

        assert_eq!(session_id.as_str(), "session-42");
        assert_eq!(device_id.as_str(), "camera-front");
    }

    #[test]
    fn typed_identifiers_reject_whitespace_only_values() {
        assert_eq!(
            SessionId::new(" \t "),
            Err(DomainError::EmptyIdentifier {
                kind: IdentifierKind::Session,
            })
        );
        assert_eq!(
            DeviceId::new(""),
            Err(DomainError::EmptyIdentifier {
                kind: IdentifierKind::Device,
            })
        );
    }
}
