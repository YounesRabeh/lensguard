use std::io;
use std::os::unix::fs::{FileTypeExt, MetadataExt, PermissionsExt};
use std::path::Path;

use camera_core::{
    MAX_OBSERVER_MESSAGE_SIZE, OBSERVER_SCHEMA_VERSION, ObserverAvailability,
    ObserverClientMessage, ObserverMessage, SuppressionDiagnostics,
};
use std::collections::BTreeMap;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::{RwLock, broadcast};

#[derive(Clone, Debug)]
pub struct RoutedMessage {
    pub user_id: Option<u32>,
    pub message: ObserverMessage,
}

pub struct SharedStatus {
    pub availability: RwLock<(ObserverAvailability, String)>,
    pub messages: broadcast::Sender<RoutedMessage>,
    pub diagnostics: RwLock<BTreeMap<u32, SuppressionDiagnostics>>,
}

pub async fn serve(listener: UnixListener, shared: std::sync::Arc<SharedStatus>) -> io::Result<()> {
    loop {
        let (stream, _) = listener.accept().await?;
        let shared = std::sync::Arc::clone(&shared);
        tokio::spawn(async move {
            if let Err(error) = serve_client(stream, shared).await {
                tracing::debug!(%error, "observer IPC client disconnected");
            }
        });
    }
}

async fn serve_client(
    mut stream: UnixStream,
    shared: std::sync::Arc<SharedStatus>,
) -> io::Result<()> {
    let credentials = stream.peer_cred()?;
    let user_id = credentials.uid();
    let process_id = credentials.pid().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::PermissionDenied,
            "IPC peer PID is unavailable",
        )
    })?;
    let process_id = u32::try_from(process_id)
        .map_err(|_| io::Error::new(io::ErrorKind::PermissionDenied, "IPC peer PID is invalid"))?;
    if !is_authorized_daemon(process_id) {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "IPC peer is not the installed LensGuard user daemon",
        ));
    }
    let hello: ObserverClientMessage = read_message(&mut stream).await?;
    if !matches!(
        hello,
        ObserverClientMessage::Hello { schema_version: OBSERVER_SCHEMA_VERSION, ref service_version }
            if service_version == camera_core::VERSION
    ) {
        write_message(
            &mut stream,
            &ObserverMessage::Availability {
                availability: ObserverAvailability::VersionMismatch,
                detail: String::from("observer and user daemon versions do not match"),
            },
        )
        .await?;
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "observer version mismatch",
        ));
    }
    write_message(
        &mut stream,
        &ObserverMessage::Hello {
            schema_version: OBSERVER_SCHEMA_VERSION,
            observer_version: camera_core::VERSION.to_owned(),
        },
    )
    .await?;
    let (availability, detail) = shared.availability.read().await.clone();
    write_message(
        &mut stream,
        &ObserverMessage::Availability {
            availability,
            detail,
        },
    )
    .await?;
    let diagnostics = shared
        .diagnostics
        .read()
        .await
        .get(&user_id)
        .copied()
        .unwrap_or_default();
    write_message(&mut stream, &ObserverMessage::Diagnostics { diagnostics }).await?;

    let mut messages = shared.messages.subscribe();
    loop {
        let routed = messages
            .recv()
            .await
            .map_err(|error| io::Error::new(io::ErrorKind::BrokenPipe, error))?;
        if routed.user_id.is_none() || routed.user_id == Some(user_id) {
            write_message(&mut stream, &routed.message).await?;
        }
    }
}

pub fn bind_socket(path: &Path) -> io::Result<UnixListener> {
    if let Ok(metadata) = std::fs::symlink_metadata(path) {
        let kind = if metadata.file_type().is_socket() {
            "an existing socket"
        } else {
            "a non-socket file"
        };
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!("observer socket path is occupied by {kind}"),
        ));
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let listener = UnixListener::bind(path)?;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o666))?;
    Ok(listener)
}

fn is_authorized_daemon(process_id: u32) -> bool {
    let Ok(executable) = std::fs::read_link(format!("/proc/{process_id}/exe")) else {
        return false;
    };
    let allowed = [
        Path::new("/usr/lib/lensguard/camera-monitor"),
        Path::new("/usr/libexec/lensguard/camera-monitor"),
    ];
    if !allowed.contains(&executable.as_path()) {
        return false;
    }
    std::fs::metadata(&executable).is_ok_and(|metadata| {
        metadata.is_file() && metadata.uid() == 0 && metadata.mode() & 0o022 == 0
    })
}

async fn read_message<T: serde::de::DeserializeOwned>(stream: &mut UnixStream) -> io::Result<T> {
    let length = usize::try_from(stream.read_u32().await?)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "invalid message length"))?;
    if length == 0 || length > MAX_OBSERVER_MESSAGE_SIZE {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "oversized observer message",
        ));
    }
    let mut payload = vec![0; length];
    stream.read_exact(&mut payload).await?;
    serde_json::from_slice(&payload)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

async fn write_message<T: serde::Serialize>(stream: &mut UnixStream, value: &T) -> io::Result<()> {
    let payload = serde_json::to_vec(value)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    if payload.len() > MAX_OBSERVER_MESSAGE_SIZE {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "oversized observer message",
        ));
    }
    stream
        .write_u32(u32::try_from(payload.len()).map_err(|_| {
            io::Error::new(io::ErrorKind::InvalidData, "oversized observer message")
        })?)
        .await?;
    stream.write_all(&payload).await?;
    stream.flush().await
}
