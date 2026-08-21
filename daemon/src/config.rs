use std::ffi::OsString;

use thiserror::Error;
use tracing::Level;

/// Operation selected on the daemon command line.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Command {
    Run,
    InspectV4l2,
    WatchV4l2,
    ServeDbus,
    Version,
    Help,
}

/// Validated process configuration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Config {
    pub command: Command,
    pub log_level: Level,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            command: Command::Run,
            log_level: Level::INFO,
        }
    }
}

impl Config {
    /// Parses arguments excluding the executable name.
    ///
    /// # Errors
    ///
    /// Returns a typed error for unknown options, conflicting commands, missing values, or an
    /// unsupported log level.
    pub fn parse_from<I, S>(arguments: I) -> Result<Self, ConfigError>
    where
        I: IntoIterator<Item = S>,
        S: Into<OsString>,
    {
        let mut config = Self::default();
        let mut command = None;
        let mut arguments = arguments.into_iter().map(Into::into);

        while let Some(argument) = arguments.next() {
            let argument = argument
                .into_string()
                .map_err(|_| ConfigError::NonUnicodeArgument)?;
            if argument == "--log-level" {
                let value = arguments
                    .next()
                    .ok_or(ConfigError::MissingLogLevel)?
                    .into_string()
                    .map_err(|_| ConfigError::NonUnicodeArgument)?;
                config.log_level = parse_log_level(&value)?;
                continue;
            }
            if let Some(value) = argument.strip_prefix("--log-level=") {
                config.log_level = parse_log_level(value)?;
                continue;
            }

            let parsed = match argument.as_str() {
                "run" => Command::Run,
                "inspect-v4l2" => Command::InspectV4l2,
                "watch-v4l2" => Command::WatchV4l2,
                "serve-dbus" => Command::ServeDbus,
                "--version" | "-V" => Command::Version,
                "--help" | "-h" => Command::Help,
                _ => return Err(ConfigError::UnknownArgument(argument)),
            };
            if let Some(first) = command.replace(parsed) {
                return Err(ConfigError::ConflictingCommands {
                    first,
                    second: parsed,
                });
            }
        }

        if let Some(command) = command {
            config.command = command;
        }
        Ok(config)
    }
}

/// Command-line validation failure.
#[derive(Debug, Error, Eq, PartialEq)]
pub enum ConfigError {
    #[error("command-line arguments must be valid UTF-8")]
    NonUnicodeArgument,
    #[error("--log-level requires one of trace, debug, info, warn, or error")]
    MissingLogLevel,
    #[error("unsupported log level '{0}'; expected trace, debug, info, warn, or error")]
    InvalidLogLevel(String),
    #[error("unsupported argument '{0}'")]
    UnknownArgument(String),
    #[error("conflicting commands {first:?} and {second:?}")]
    ConflictingCommands { first: Command, second: Command },
}

fn parse_log_level(value: &str) -> Result<Level, ConfigError> {
    match value.to_ascii_lowercase().as_str() {
        "trace" => Ok(Level::TRACE),
        "debug" => Ok(Level::DEBUG),
        "info" => Ok(Level::INFO),
        "warn" => Ok(Level::WARN),
        "error" => Ok(Level::ERROR),
        _ => Err(ConfigError::InvalidLogLevel(value.to_owned())),
    }
}

/// Human-readable command usage.
pub const USAGE: &str = "Usage: camera-monitor [--log-level LEVEL] [run|inspect-v4l2|watch-v4l2|serve-dbus]\n\
       camera-monitor --version\n\
       camera-monitor --help";

#[cfg(test)]
mod tests {
    use tracing::Level;

    use super::{Command, Config, ConfigError};

    #[test]
    fn defaults_to_the_long_running_daemon() {
        assert_eq!(
            Config::parse_from(Vec::<String>::new()).unwrap(),
            Config::default()
        );
    }

    #[test]
    fn parses_commands_and_log_levels_in_either_order() {
        let first = Config::parse_from(["--log-level", "debug", "run"]).unwrap();
        let second = Config::parse_from(["inspect-v4l2", "--log-level=trace"]).unwrap();
        assert_eq!(first.command, Command::Run);
        assert_eq!(first.log_level, Level::DEBUG);
        assert_eq!(second.command, Command::InspectV4l2);
        assert_eq!(second.log_level, Level::TRACE);
    }

    #[test]
    fn rejects_missing_invalid_and_conflicting_values() {
        assert_eq!(
            Config::parse_from(["--log-level"]).unwrap_err(),
            ConfigError::MissingLogLevel
        );
        assert_eq!(
            Config::parse_from(["--log-level=noisy"]).unwrap_err(),
            ConfigError::InvalidLogLevel(String::from("noisy"))
        );
        assert!(matches!(
            Config::parse_from(["run", "serve-dbus"]),
            Err(ConfigError::ConflictingCommands { .. })
        ));
    }
}
