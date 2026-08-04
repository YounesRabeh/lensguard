use std::process::ExitCode;

use camera_app_resolver::{ApplicationResolver, ResolutionRequest};
use camera_core::{CameraEventSource, MonitorEvent, MonitorState};

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() -> ExitCode {
    match std::env::args().nth(1).as_deref() {
        Some("--version" | "-V") => {
            println!("camera-monitor {VERSION}");
            ExitCode::SUCCESS
        }
        Some("inspect-pipewire") => inspect_pipewire(),
        Some("watch-pipewire") => watch_pipewire(),
        Some("serve-dbus") => serve_dbus(),
        None => {
            eprintln!(
                "camera-monitor {VERSION}: use inspect-pipewire, watch-pipewire, or serve-dbus"
            );
            ExitCode::SUCCESS
        }
        Some(argument) => {
            eprintln!("camera-monitor: unsupported argument '{argument}'; try --version");
            ExitCode::from(2)
        }
    }
}

fn serve_dbus() -> ExitCode {
    if !initialize_logging() {
        return ExitCode::FAILURE;
    }
    let runtime = match tokio::runtime::Runtime::new() {
        Ok(runtime) => runtime,
        Err(error) => {
            eprintln!("camera-monitor: failed to create D-Bus runtime: {error}");
            return ExitCode::FAILURE;
        }
    };
    runtime.block_on(async {
        let service = match camera_dbus::DbusService::session(MonitorState::new()).await {
            Ok(service) => service,
            Err(error) => {
                eprintln!("camera-monitor: D-Bus service failed to start: {error}");
                return ExitCode::FAILURE;
            }
        };
        println!(
            "D-Bus camera monitor ready at {} {}; press Ctrl+C to stop",
            camera_dbus::BUS_NAME,
            camera_dbus::OBJECT_PATH
        );
        if let Err(error) = tokio::signal::ctrl_c().await {
            eprintln!("camera-monitor: failed to wait for shutdown signal: {error}");
            return ExitCode::FAILURE;
        }
        if let Err(error) = service.shutdown().await {
            eprintln!("camera-monitor: D-Bus service shutdown failed: {error}");
            return ExitCode::FAILURE;
        }
        ExitCode::SUCCESS
    })
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
