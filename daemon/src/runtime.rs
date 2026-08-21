use std::io;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use camera_app_resolver::{ApplicationResolver, ResolverError};
use camera_core::{MonitorEvent, MonitorState};
use camera_dbus::{DbusService, ServiceError};
use thiserror::Error;
use tokio::sync::mpsc;
use tokio::task::{JoinError, JoinHandle};
use tokio::time::timeout;
use tracing::{info, warn};

use crate::application::{ApplicationError, run_application};
use crate::backend::{ObserverBackendFactory, SupervisorExit, run_backend_supervisor};
use crate::backoff::BackoffPolicy;

/// Capacity of each application-layer channel. Backpressure is deliberate and bounded.
pub const APPLICATION_QUEUE_CAPACITY: usize = 128;
const RESOLVER_CACHE_CAPACITY: usize = 128;
const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(5);
const INITIAL_OBSERVER_REASON: &str = "V4L2 observer connection is pending";

/// Long-running daemon failure.
#[derive(Debug, Error)]
pub enum RuntimeError {
    #[error("failed to construct the application resolver: {0}")]
    Resolver(#[from] ResolverError),
    #[error(transparent)]
    Dbus(#[from] ServiceError),
    #[error("failed to install or receive a shutdown signal: {0}")]
    Signal(#[from] io::Error),
    #[error("daemon task failed: {0}")]
    Task(#[from] JoinError),
    #[error(transparent)]
    Application(#[from] ApplicationError),
    #[error("daemon task '{0}' exited before shutdown was requested")]
    UnexpectedTaskExit(&'static str),
    #[error("daemon task '{0}' did not stop within the shutdown deadline")]
    ShutdownTimeout(&'static str),
}

enum FirstExit {
    Signal(io::Result<()>),
    Backend,
    Application,
    Publisher,
}

/// Runs the production daemon until SIGINT, SIGTERM, or an unexpected task exit.
///
/// # Errors
///
/// Returns a typed startup, task, D-Bus, signal, or bounded-shutdown failure.
pub async fn run_daemon() -> Result<(), RuntimeError> {
    let initial_state = initializing_state();
    let service = DbusService::session(initial_state.clone()).await?;
    let resolver = ApplicationResolver::new(RESOLVER_CACHE_CAPACITY)?;
    let (backend_tx, backend_rx) = mpsc::channel(APPLICATION_QUEUE_CAPACITY);
    let (publication_tx, publication_rx) = mpsc::channel(APPLICATION_QUEUE_CAPACITY);
    let shutdown = Arc::new(AtomicBool::new(false));

    let backend_shutdown = Arc::clone(&shutdown);
    let mut backend_task = tokio::task::spawn_blocking(move || {
        run_backend_supervisor(
            &ObserverBackendFactory,
            &backend_tx,
            &backend_shutdown,
            BackoffPolicy::default(),
        )
    });
    let mut application_task = tokio::spawn(run_application(
        backend_rx,
        publication_tx,
        resolver,
        initial_state,
    ));
    let mut publisher_task = tokio::spawn(publish_events(publication_rx, service));

    info!(
        bus_name = camera_dbus::BUS_NAME,
        object_path = camera_dbus::OBJECT_PATH,
        queue_capacity = APPLICATION_QUEUE_CAPACITY,
        "camera monitor daemon ready"
    );

    let mut backend_result = None;
    let mut application_result = None;
    let mut publisher_result = None;
    let first_exit = tokio::select! {
        result = shutdown_signal() => {
            FirstExit::Signal(result)
        }
        result = &mut backend_task => {
            backend_result = Some(result);
            FirstExit::Backend
        }
        result = &mut application_task => {
            application_result = Some(result);
            FirstExit::Application
        }
        result = &mut publisher_task => {
            publisher_result = Some(result);
            FirstExit::Publisher
        }
    };

    shutdown.store(true, Ordering::Release);
    let backend_outcome = match backend_result {
        Some(result) => result.map_err(RuntimeError::Task),
        None => await_task("backend", &mut backend_task).await,
    };
    let application_outcome = match application_result {
        Some(result) => result.map_err(RuntimeError::Task),
        None => await_task("application", &mut application_task).await,
    };
    let publisher_outcome = match publisher_result {
        Some(result) => result.map_err(RuntimeError::Task),
        None => await_task("publisher", &mut publisher_task).await,
    };
    let publisher_cleanup = match publisher_outcome {
        Ok(Ok(service)) => service.shutdown().await.map_err(RuntimeError::Dbus),
        Ok(Err(error)) => Err(RuntimeError::Dbus(error)),
        Err(error) => Err(error),
    };

    if let FirstExit::Signal(result) = &first_exit {
        result.as_ref().map_err(|error| {
            RuntimeError::Signal(io::Error::new(error.kind(), error.to_string()))
        })?;
    }
    let backend_exit = backend_outcome?;
    let _final_state = application_outcome??;
    publisher_cleanup?;

    if matches!(first_exit, FirstExit::Signal(_)) && backend_exit == SupervisorExit::Shutdown {
        info!("camera monitor daemon stopped cleanly");
        Ok(())
    } else {
        let task = match first_exit {
            FirstExit::Signal(_) | FirstExit::Backend => "backend",
            FirstExit::Application => "application",
            FirstExit::Publisher => "publisher",
        };
        Err(RuntimeError::UnexpectedTaskExit(task))
    }
}

/// Runs the Step 6 standalone D-Bus endpoint until SIGINT or SIGTERM.
///
/// # Errors
///
/// Returns a D-Bus or signal-handling failure.
pub async fn run_standalone_dbus() -> Result<(), RuntimeError> {
    let service = DbusService::session(MonitorState::new()).await?;
    info!(
        bus_name = camera_dbus::BUS_NAME,
        object_path = camera_dbus::OBJECT_PATH,
        "standalone D-Bus camera monitor ready"
    );
    shutdown_signal().await?;
    service.shutdown().await?;
    Ok(())
}

/// Applies ordered application events to the D-Bus service and returns ownership for cleanup.
///
/// # Errors
///
/// Returns the first D-Bus publication failure.
pub async fn publish_events(
    mut events: mpsc::Receiver<MonitorEvent>,
    service: DbusService,
) -> Result<DbusService, ServiceError> {
    while let Some(event) = events.recv().await {
        service.apply_event(event).await?;
    }
    Ok(service)
}

/// Initial state explicitly distinguishes startup delay from an available inactive backend.
#[must_use]
pub fn initializing_state() -> MonitorState {
    let mut state = MonitorState::new();
    state.apply(MonitorEvent::ObserverAvailabilityChanged {
        availability: camera_core::ObserverAvailability::ConnectionFailed,
        detail: String::from(INITIAL_OBSERVER_REASON),
    });
    state
}

async fn await_task<T>(name: &'static str, task: &mut JoinHandle<T>) -> Result<T, RuntimeError> {
    if let Ok(result) = timeout(SHUTDOWN_TIMEOUT, &mut *task).await {
        Ok(result?)
    } else {
        task.abort();
        warn!(task = name, "daemon task exceeded shutdown deadline");
        Err(RuntimeError::ShutdownTimeout(name))
    }
}

/// Waits for either supported process termination signal.
///
/// # Errors
///
/// Returns an operating-system error if signal handlers cannot be installed or their stream ends.
pub async fn shutdown_signal() -> io::Result<()> {
    #[cfg(unix)]
    {
        let mut terminate =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
        tokio::select! {
            result = tokio::signal::ctrl_c() => result,
            signal = terminate.recv() => signal.map_or_else(
                || Err(io::Error::new(io::ErrorKind::BrokenPipe, "SIGTERM stream ended")),
                |()| Ok(()),
            ),
        }
    }
    #[cfg(not(unix))]
    {
        tokio::signal::ctrl_c().await
    }
}
