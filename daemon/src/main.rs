use std::future::Future;
use std::process::ExitCode;

use camera_app_resolver::{ApplicationResolver, ResolutionRequest};
use camera_core::MonitorEvent;
use camera_monitor::observer::ObserverEventSource;
use camera_monitor::{Command, Config, USAGE};

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() -> ExitCode {
    let config = match Config::parse_from(std::env::args_os().skip(1)) {
        Ok(config) => config,
        Err(error) => {
            eprintln!("camera-monitor: {error}\n{USAGE}");
            return ExitCode::from(2);
        }
    };

    match config.command {
        Command::Version => {
            println!("camera-monitor {VERSION}");
            ExitCode::SUCCESS
        }
        Command::Help => {
            println!("{USAGE}");
            ExitCode::SUCCESS
        }
        command => {
            if let Err(error) = initialize_logging(config.log_level) {
                eprintln!("camera-monitor: failed to initialize logging: {error}");
                return ExitCode::FAILURE;
            }
            dispatch(command)
        }
    }
}

fn dispatch(command: Command) -> ExitCode {
    match command {
        Command::Run => run_async(camera_monitor::run_daemon()),
        Command::InspectV4l2 => inspect_v4l2(),
        Command::WatchV4l2 => watch_v4l2(),
        Command::ServeDbus => run_async(camera_monitor::run_standalone_dbus()),
        Command::Version | Command::Help => unreachable!("handled before logging initialization"),
    }
}

fn run_async(future: impl Future<Output = Result<(), camera_monitor::RuntimeError>>) -> ExitCode {
    let runtime = match tokio::runtime::Runtime::new() {
        Ok(runtime) => runtime,
        Err(error) => {
            tracing::error!(%error, "failed to create asynchronous runtime");
            return ExitCode::FAILURE;
        }
    };
    match runtime.block_on(future) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            tracing::error!(%error, "daemon operation failed");
            ExitCode::FAILURE
        }
    }
}

fn inspect_v4l2() -> ExitCode {
    let mut source = match ObserverEventSource::connect() {
        Ok(source) => source,
        Err(error) => {
            tracing::error!(%error, availability = error.availability().as_str(), "V4L2 observer unavailable");
            return ExitCode::FAILURE;
        }
    };
    match source.next_event_timeout(std::time::Duration::from_secs(2)) {
        Ok(Some(event)) => {
            print_monitor_event(&event);
            ExitCode::SUCCESS
        }
        Ok(None) => {
            println!("V4L2 observer connected; no status message received");
            ExitCode::SUCCESS
        }
        Err(error) => {
            tracing::error!(%error, "V4L2 observer status failed");
            ExitCode::FAILURE
        }
    }
}

fn watch_v4l2() -> ExitCode {
    let mut source = match ObserverEventSource::connect() {
        Ok(source) => source,
        Err(error) => {
            tracing::error!(%error, "V4L2 observer failed to start");
            return ExitCode::FAILURE;
        }
    };
    let mut resolver = match ApplicationResolver::new(128) {
        Ok(resolver) => resolver,
        Err(error) => {
            tracing::error!(%error, "application resolver failed to start");
            return ExitCode::FAILURE;
        }
    };
    println!("Direct V4L2 camera monitor ready; press Ctrl+C to stop");

    loop {
        match source.next_event_timeout(std::time::Duration::from_secs(60)) {
            Ok(Some(event)) => print_monitor_event(&resolve_application(event, &mut resolver)),
            Ok(None) => {}
            Err(error) => {
                tracing::error!(%error, "V4L2 observer ended");
                return ExitCode::FAILURE;
            }
        }
    }
}

fn resolve_application(event: MonitorEvent, resolver: &mut ApplicationResolver) -> MonitorEvent {
    match event {
        MonitorEvent::SessionStarted(mut session) => {
            session.application = resolver.resolve(ResolutionRequest::from(&session.application));
            MonitorEvent::SessionStarted(session)
        }
        MonitorEvent::SessionUpdated(mut session) => {
            session.application = resolver.resolve(ResolutionRequest::from(&session.application));
            MonitorEvent::SessionUpdated(session)
        }
        other => other,
    }
}

fn initialize_logging(
    level: tracing::Level,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    tracing_subscriber::fmt()
        .with_max_level(level)
        .with_target(false)
        .compact()
        .try_init()
}

fn print_monitor_event(event: &MonitorEvent) {
    match event {
        MonitorEvent::SessionStarted(session) => println!(
            "START session={} application={:?} camera={:?}",
            session.id, session.application.display_name, session.device.display_name
        ),
        MonitorEvent::SessionUpdated(session) => println!(
            "UPDATE session={} application={:?} camera={:?}",
            session.id, session.application.display_name, session.device.display_name
        ),
        MonitorEvent::SessionStopped(session_id) => println!("STOP session={session_id}"),
        MonitorEvent::ObserverAvailabilityChanged {
            availability,
            detail,
        } => {
            println!(
                "OBSERVER availability={} detail={detail:?}",
                availability.as_str()
            );
        }
        MonitorEvent::SuppressionDiagnosticsChanged(diagnostics) => println!(
            "SUPPRESSED broker={} unknown={}",
            diagnostics.suppressed_broker_events, diagnostics.suppressed_unknown_events,
        ),
    }
}
