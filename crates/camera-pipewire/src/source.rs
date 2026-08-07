use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, SyncSender, TrySendError};
use std::thread::{self, JoinHandle};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use camera_core::{CameraEventSource, MonitorEvent};
use tracing::warn;

use crate::{CorrelationEngine, PipeWireError, RegistryEvent, observer::attach_registry};

struct Shutdown;

type ReadySignal = Rc<RefCell<Option<SyncSender<Result<(), String>>>>>;

/// Maximum number of domain events retained between the `PipeWire` callback and its consumer.
pub const PIPEWIRE_EVENT_QUEUE_CAPACITY: usize = 256;

/// Blocking domain-event source backed by a dedicated `PipeWire` main-loop thread.
///
/// All native `PipeWire` objects remain on that thread. The public source exchanges only domain
/// events and a shutdown notification, keeping raw object identifiers inside the adapter.
pub struct PipeWireEventSource {
    events: Receiver<MonitorEvent>,
    shutdown: pipewire::channel::Sender<Shutdown>,
    worker: Option<JoinHandle<()>>,
}

impl PipeWireEventSource {
    /// Connects to the current user's `PipeWire` instance and waits for the initial graph to settle.
    ///
    /// # Errors
    ///
    /// Returns [`PipeWireError`] if the monitor thread cannot start, `PipeWire` cannot be reached,
    /// or initial registry synchronization fails.
    pub fn connect() -> Result<Self, PipeWireError> {
        let (ready_sender, ready_receiver) = mpsc::sync_channel(1);
        let (event_sender, events) = mpsc::sync_channel(PIPEWIRE_EVENT_QUEUE_CAPACITY);
        let (shutdown, shutdown_receiver) = pipewire::channel::channel();
        let worker = thread::Builder::new()
            .name(String::from("lensguard-pipewire"))
            .spawn(move || monitor_thread(&ready_sender, &event_sender, shutdown_receiver))
            .map_err(PipeWireError::MonitorThread)?;

        match ready_receiver.recv() {
            Ok(Ok(())) => Ok(Self {
                events,
                shutdown,
                worker: Some(worker),
            }),
            Ok(Err(details)) => {
                let _ = worker.join();
                Err(PipeWireError::MonitorInitialization { details })
            }
            Err(_) => {
                let _ = worker.join();
                Err(PipeWireError::InitializationChannelClosed)
            }
        }
    }

    /// Waits up to `timeout` for the next event.
    ///
    /// `Ok(None)` means that the timeout elapsed and allows an owner to observe cancellation.
    ///
    /// # Errors
    ///
    /// Returns [`PipeWireError::EventChannelClosed`] when the monitor thread has ended.
    pub fn next_event_timeout(
        &mut self,
        timeout: Duration,
    ) -> Result<Option<MonitorEvent>, PipeWireError> {
        match self.events.recv_timeout(timeout) {
            Ok(event) => Ok(Some(event)),
            Err(RecvTimeoutError::Timeout) => Ok(None),
            Err(RecvTimeoutError::Disconnected) => Err(PipeWireError::EventChannelClosed),
        }
    }
}

impl CameraEventSource for PipeWireEventSource {
    type Error = PipeWireError;

    fn next_event(&mut self) -> Result<Option<MonitorEvent>, Self::Error> {
        self.events
            .recv()
            .map(Some)
            .map_err(|_| PipeWireError::EventChannelClosed)
    }
}

impl Drop for PipeWireEventSource {
    fn drop(&mut self) {
        let _ = self.shutdown.send(Shutdown);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

fn monitor_thread(
    ready_sender: &SyncSender<Result<(), String>>,
    event_sender: &SyncSender<MonitorEvent>,
    shutdown_receiver: pipewire::channel::Receiver<Shutdown>,
) {
    if let Err(error) = run_monitor(ready_sender, event_sender, shutdown_receiver) {
        let _ = ready_sender.send(Err(error.to_string()));
    }
}

fn run_monitor(
    ready_sender: &SyncSender<Result<(), String>>,
    event_sender: &SyncSender<MonitorEvent>,
    shutdown_receiver: pipewire::channel::Receiver<Shutdown>,
) -> Result<(), PipeWireError> {
    pipewire::init();
    let main_loop = pipewire::main_loop::MainLoopRc::new(None).map_err(PipeWireError::MainLoop)?;
    let context =
        pipewire::context::ContextRc::new(&main_loop, None).map_err(PipeWireError::Context)?;
    let core = context.connect_rc(None).map_err(PipeWireError::Connect)?;
    let registry = core.get_registry_rc().map_err(PipeWireError::Registry)?;
    let engine = Rc::new(RefCell::new(CorrelationEngine::new()));

    let engine_for_events = Rc::clone(&engine);
    let sender_for_events = event_sender.clone();
    let main_loop_for_events = main_loop.downgrade();
    let on_event: Rc<dyn Fn(RegistryEvent)> = Rc::new(move |event| {
        if !apply_registry_event(&engine_for_events, &sender_for_events, event) {
            if let Some(main_loop) = main_loop_for_events.upgrade() {
                main_loop.quit();
            }
        }
    });
    let _observation = attach_registry(&registry, &on_event);

    let first_barrier = core.sync(0).map_err(PipeWireError::Synchronize)?;
    let pending_barrier = Rc::new(Cell::new(first_barrier));
    let phase = Rc::new(Cell::new(0_u8));
    let ready = Rc::new(RefCell::new(Some(ready_sender.clone())));
    let core_weak = core.downgrade();
    let pending_for_done = Rc::clone(&pending_barrier);
    let phase_for_done = Rc::clone(&phase);
    let ready_for_done = Rc::clone(&ready);
    let main_loop_for_done = main_loop.downgrade();
    let ready_for_error = Rc::clone(&ready);
    let engine_for_error = Rc::clone(&engine);
    let sender_for_error = event_sender.clone();
    let main_loop_for_error = main_loop.downgrade();
    let _core_listener = core
        .add_listener_local()
        .done(move |id, sequence| {
            if id != pipewire::core::PW_ID_CORE || sequence != pending_for_done.get() {
                return;
            }
            if phase_for_done.get() == 0 {
                let Some(core) = core_weak.upgrade() else {
                    signal_initial_error(
                        &ready_for_done,
                        "PipeWire core disappeared during initial synchronization",
                    );
                    if let Some(main_loop) = main_loop_for_done.upgrade() {
                        main_loop.quit();
                    }
                    return;
                };
                match core.sync(sequence.seq()) {
                    Ok(next) => {
                        pending_for_done.set(next);
                        phase_for_done.set(1);
                    }
                    Err(error) => {
                        signal_initial_error(
                            &ready_for_done,
                            &format!("failed to synchronize with PipeWire: {error}"),
                        );
                        if let Some(main_loop) = main_loop_for_done.upgrade() {
                            main_loop.quit();
                        }
                    }
                }
            } else if let Some(sender) = ready_for_done.borrow_mut().take() {
                let _ = sender.send(Ok(()));
            }
        })
        .error(move |object_id, _sequence, result, message| {
            let reason =
                format!("PipeWire core error for object {object_id} (result {result}): {message}");
            if ready_for_error.borrow().is_some() {
                signal_initial_error(&ready_for_error, &reason);
            } else {
                let events = engine_for_error.borrow_mut().backend_unavailable(reason);
                send_events(&sender_for_error, events);
            }
            if let Some(main_loop) = main_loop_for_error.upgrade() {
                main_loop.quit();
            }
        })
        .register();

    let main_loop_for_shutdown = main_loop.downgrade();
    let _shutdown_receiver = shutdown_receiver.attach(main_loop.loop_(), move |_| {
        if let Some(main_loop) = main_loop_for_shutdown.upgrade() {
            main_loop.quit();
        }
    });

    main_loop.run();
    if ready.borrow().is_some() {
        signal_initial_error(
            &ready,
            "PipeWire main loop ended before initial synchronization",
        );
    }
    Ok(())
}

fn apply_registry_event(
    engine: &RefCell<CorrelationEngine>,
    sender: &SyncSender<MonitorEvent>,
    event: RegistryEvent,
) -> bool {
    match engine.borrow_mut().apply(event, current_unix_ms()) {
        Ok(events) => send_events(sender, events),
        Err(error) => {
            warn!(%error, "ignored malformed PipeWire graph change");
            true
        }
    }
}

fn signal_initial_error(ready: &ReadySignal, details: &str) {
    if let Some(sender) = ready.borrow_mut().take() {
        let _ = sender.send(Err(details.to_owned()));
    }
}

fn send_events(sender: &SyncSender<MonitorEvent>, events: Vec<MonitorEvent>) -> bool {
    for event in events {
        match sender.try_send(event) {
            Ok(()) => {}
            Err(TrySendError::Full(_)) => {
                warn!(
                    capacity = PIPEWIRE_EVENT_QUEUE_CAPACITY,
                    "PipeWire event queue is full; reconnecting to reconcile state"
                );
                return false;
            }
            Err(TrySendError::Disconnected(_)) => return false,
        }
    }
    true
}

fn current_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| {
            u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
        })
}
