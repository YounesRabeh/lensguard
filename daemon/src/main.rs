use std::process::ExitCode;

use camera_app_resolver::{ApplicationResolver, ResolutionRequest};
use camera_core::{CameraEventSource, MonitorEvent};

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() -> ExitCode {
    match std::env::args().nth(1).as_deref() {
        Some("--version" | "-V") => {
            println!("camera-monitor {VERSION}");
            ExitCode::SUCCESS
        }
        Some("inspect-pipewire") => inspect_pipewire(),
        Some("watch-pipewire") => watch_pipewire(),
        None => {
            eprintln!(
                "camera-monitor {VERSION}: use inspect-pipewire or watch-pipewire for diagnostics"
            );
            ExitCode::SUCCESS
        }
        Some(argument) => {
            eprintln!("camera-monitor: unsupported argument '{argument}'; try --version");
            ExitCode::from(2)
        }
    }
}

fn inspect_pipewire() -> ExitCode {
    if !initialize_logging() {
        return ExitCode::FAILURE;
    }

    match camera_pipewire::inspect_pipewire() {
        Ok(graph) => {
            print!("{}", graph.diagnostic_summary());
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("camera-monitor: PipeWire inspection failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn watch_pipewire() -> ExitCode {
    if !initialize_logging() {
        return ExitCode::FAILURE;
    }
    let mut source = match camera_pipewire::PipeWireEventSource::connect() {
        Ok(source) => source,
        Err(error) => {
            eprintln!("camera-monitor: PipeWire monitor failed to start: {error}");
            return ExitCode::FAILURE;
        }
    };
    let mut resolver = match ApplicationResolver::new(128) {
        Ok(resolver) => resolver,
        Err(error) => {
            eprintln!("camera-monitor: application resolver failed to start: {error}");
            return ExitCode::FAILURE;
        }
    };
    println!("PipeWire camera relationship monitor ready; press Ctrl+C to stop");

    loop {
        match source.next_event() {
            Ok(Some(event)) => {
                let event = resolve_application(event, &mut resolver);
                print_monitor_event(&event);
            }
            Ok(None) => return ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("camera-monitor: PipeWire monitor ended: {error}");
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

fn initialize_logging() -> bool {
    if let Err(error) = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .with_target(false)
        .compact()
        .try_init()
    {
        eprintln!("camera-monitor: failed to initialize diagnostic logging: {error}");
        false
    } else {
        true
    }
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
        MonitorEvent::BackendUnavailable { reason } => {
            println!("BACKEND_UNAVAILABLE reason={reason:?}");
        }
        MonitorEvent::BackendRecovered => println!("BACKEND_RECOVERED"),
    }
}
