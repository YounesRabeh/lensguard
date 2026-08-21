use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

use camera_core::CaptureOwner;
use serde::Deserialize;
use thiserror::Error;

pub const TRUSTED_BROKER_POLICY_PATH: &str = "/usr/share/lensguard/trusted-brokers-v1.json";

#[derive(Debug, Error)]
pub enum PolicyError {
    #[error("failed to read trusted-broker policy: {0}")]
    Read(#[from] std::io::Error),
    #[error("trusted-broker policy is malformed: {0}")]
    Parse(#[from] serde_json::Error),
    #[error("unsupported trusted-broker policy schema {0}")]
    Version(u16),
    #[error("trusted-broker policy contains no broker executable paths")]
    Empty,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TrustedBrokerPolicy {
    schema_version: u16,
    brokers: Vec<TrustedBroker>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct TrustedBroker {
    id: String,
    executable_paths: Vec<PathBuf>,
}

impl TrustedBrokerPolicy {
    pub fn load() -> Result<Self, PolicyError> {
        Self::from_json(&fs::read_to_string(TRUSTED_BROKER_POLICY_PATH)?)
    }

    pub fn from_json(json: &str) -> Result<Self, PolicyError> {
        let policy: Self = serde_json::from_str(json)?;
        if policy.schema_version != 1 {
            return Err(PolicyError::Version(policy.schema_version));
        }
        if policy.brokers.is_empty()
            || policy
                .brokers
                .iter()
                .any(|broker| broker.id.trim().is_empty() || broker.executable_paths.is_empty())
        {
            return Err(PolicyError::Empty);
        }
        Ok(policy)
    }

    pub fn classify_process(&self, process_id: u32) -> CaptureOwner {
        let Ok(executable) = fs::read_link(format!("/proc/{process_id}/exe")) else {
            return CaptureOwner::Unknown;
        };
        self.classify_executable(&executable)
    }

    fn classify_executable(&self, executable: &Path) -> CaptureOwner {
        self.classify_executable_with(executable, is_package_owned)
    }

    fn classify_executable_with(
        &self,
        executable: &Path,
        trusted_identity: impl Fn(&Path) -> bool,
    ) -> CaptureOwner {
        for broker in &self.brokers {
            if broker
                .executable_paths
                .iter()
                .any(|path| path == executable)
            {
                return if trusted_identity(executable) {
                    CaptureOwner::BrokerOwned
                } else {
                    CaptureOwner::Unknown
                };
            }
        }

        let resembles_broker = executable
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| {
                self.brokers.iter().any(|broker| {
                    broker.executable_paths.iter().any(|path| {
                        path.file_name().and_then(|candidate| candidate.to_str()) == Some(name)
                    })
                })
            });
        if resembles_broker {
            CaptureOwner::Unknown
        } else {
            CaptureOwner::Direct
        }
    }
}

fn is_package_owned(path: &Path) -> bool {
    fs::metadata(path).is_ok_and(|metadata| {
        metadata.is_file() && metadata.uid() == 0 && metadata.mode() & 0o022 == 0
    })
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use camera_core::CaptureOwner;

    use super::{PolicyError, TrustedBrokerPolicy};

    #[test]
    fn rejects_unknown_schema_and_empty_policy() {
        assert!(matches!(
            TrustedBrokerPolicy::from_json(r#"{"schema_version":2,"brokers":[]}"#),
            Err(PolicyError::Version(2))
        ));
        assert!(matches!(
            TrustedBrokerPolicy::from_json(r#"{"schema_version":1,"brokers":[]}"#),
            Err(PolicyError::Empty)
        ));
    }

    #[test]
    fn exact_trusted_path_and_same_name_lookalike_are_distinct() {
        let policy = TrustedBrokerPolicy::from_json(
            r#"{"schema_version":1,"brokers":[{"id":"broker","executable_paths":["/bin/sh"]}]}"#,
        )
        .unwrap();
        assert_eq!(
            policy.classify_executable_with(Path::new("/bin/sh"), |_| true),
            CaptureOwner::BrokerOwned
        );
        assert_eq!(
            policy.classify_executable(Path::new("/tmp/sh")),
            CaptureOwner::Unknown
        );
        assert_eq!(
            policy.classify_executable(Path::new("/opt/direct-camera")),
            CaptureOwner::Direct
        );
    }
}
