use std::process::ExitCode;

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() -> ExitCode {
    match std::env::args().nth(1).as_deref() {
        Some("--version" | "-V") => {
            println!("camera-monitor {VERSION}");
            ExitCode::SUCCESS
        }
        None => {
            eprintln!(
                "camera-monitor {VERSION}: bootstrap executable; monitoring is not implemented yet"
            );
            ExitCode::SUCCESS
        }
        Some(argument) => {
            eprintln!("camera-monitor: unsupported argument '{argument}'; try --version");
            ExitCode::from(2)
        }
    }
}
