use std::error::Error;
use std::fmt::{self, Display, Formatter};

/// Identifies which stable domain identifier failed validation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IdentifierKind {
    /// A [`SessionId`](crate::SessionId).
    Session,
    /// A [`DeviceId`](crate::DeviceId).
    Device,
}

impl Display for IdentifierKind {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Session => formatter.write_str("session"),
            Self::Device => formatter.write_str("device"),
        }
    }
}

/// Errors caused by invalid domain values.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DomainError {
    /// A stable identifier contained no visible characters.
    EmptyIdentifier { kind: IdentifierKind },
}

impl Display for DomainError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyIdentifier { kind } => write!(
                formatter,
                "{kind} identifier must contain at least one non-whitespace character"
            ),
        }
    }
}

impl Error for DomainError {}
