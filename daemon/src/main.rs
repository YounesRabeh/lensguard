use std::process::ExitCode;

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() -> ExitCode {
    match std::env::args().nth(1).as_deref() {
        Some("--version" | "-V") => {
            println!("camera-monitor {VERSION}");
            ExitCode::SUCCESS
        }
        Some("inspect-pipewire") => inspect_pipewire(),
        None => {
            eprintln!(
                "camera-monitor {VERSION}: use inspect-pipewire for a one-time graph diagnostic"
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
    if let Err(error) = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .with_target(false)
        .compact()
        .try_init()
    {
        eprintln!("camera-monitor: failed to initialize diagnostic logging: {error}");
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
