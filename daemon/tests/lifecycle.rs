use std::collections::VecDeque;
use std::fs;
use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, mpsc as std_mpsc};
use std::thread;
use std::time::{Duration, Instant};

use camera_app_resolver::ResolutionRequest;
use camera_core::{
    ApplicationIdentity, CameraDevice, CameraSession, CameraSessionState, DeviceId, MonitorEvent,
    MonitorState, ObserverAvailability, SessionId,
};
use camera_dbus::{BUS_NAME, DbusService, INTERFACE_NAME, OBJECT_PATH, SessionDto};
use camera_monitor::application::{IdentityResolver, run_application};
use camera_monitor::backend::{
    BackendFactory, BackendSource, SupervisorExit, run_backend_supervisor,
};
use camera_monitor::backoff::BackoffPolicy;
use camera_monitor::runtime::{initializing_state, publish_events};
use thiserror::Error;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio::time::timeout;
use zbus::{Connection, Proxy, connection};

static DBUS_TEST_LOCK: Mutex<()> = Mutex::new(());

#[derive(Clone, Debug, Error)]
#[error("{0}")]
struct FakeError(&'static str);

struct ChannelSource {
    events: std_mpsc::Receiver<Result<MonitorEvent, FakeError>>,
    observed: Arc<AtomicUsize>,
}

impl BackendSource for ChannelSource {
    type Error = FakeError;

    fn next_event_timeout(
        &mut self,
        duration: Duration,
    ) -> Result<Option<MonitorEvent>, Self::Error> {
        match self.events.recv_timeout(duration) {
            Ok(Ok(event)) => {
                self.observed.fetch_add(1, Ordering::Release);
                Ok(Some(event))
            }
            Ok(Err(error)) => Err(error),
            Err(std_mpsc::RecvTimeoutError::Timeout) => Ok(None),
            Err(std_mpsc::RecvTimeoutError::Disconnected) => {
                Err(FakeError("fake backend channel disconnected"))
            }
        }
    }
}

struct ChannelFactory {
    sources: Mutex<VecDeque<Result<ChannelSource, FakeError>>>,
}

type FakeEventSender = std_mpsc::Sender<Result<MonitorEvent, FakeError>>;
type FactoryFixture = (Arc<ChannelFactory>, Vec<FakeEventSender>, Arc<AtomicUsize>);

impl BackendFactory for ChannelFactory {
    type Source = ChannelSource;
    type Error = FakeError;

    fn connect(&self) -> Result<Self::Source, Self::Error> {
        self.sources
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or(Err(FakeError("no fake backend connection available")))
    }
}

fn channel_factory(source_count: usize) -> FactoryFixture {
    let observed = Arc::new(AtomicUsize::new(0));
    let mut sources = VecDeque::new();
    let mut senders = Vec::new();
    for _ in 0..source_count {
        let (sender, events) = std_mpsc::channel();
        senders.push(sender);
        sources.push_back(Ok(ChannelSource {
            events,
            observed: Arc::clone(&observed),
        }));
    }
    (
        Arc::new(ChannelFactory {
            sources: Mutex::new(sources),
        }),
        senders,
        observed,
    )
}

fn spawn_supervisor(
    factory: Arc<ChannelFactory>,
    input: mpsc::Sender<MonitorEvent>,
    shutdown: Arc<AtomicBool>,
    backoff: BackoffPolicy,
) -> JoinHandle<SupervisorExit> {
    tokio::task::spawn_blocking(move || {
        run_backend_supervisor(factory.as_ref(), &input, &shutdown, backoff)
    })
}

struct TestResolver {
    delay: Duration,
}

impl IdentityResolver for TestResolver {
    fn resolve(&mut self, request: ResolutionRequest) -> ApplicationIdentity {
        thread::sleep(self.delay);
        ApplicationIdentity {
            pid: request.pid,
            app_id: request
                .app_id
                .or_else(|| Some(String::from("org.example.Resolved"))),
            display_name: request
                .metadata_display_name
                .unwrap_or_else(|| String::from("Resolved Camera")),
            binary: request.binary,
        }
    }
}

fn session(id: &str, name: &str) -> CameraSession {
    CameraSession {
        id: SessionId::new(id).unwrap(),
        application: ApplicationIdentity {
            pid: Some(7_200),
            app_id: None,
            display_name: name.to_owned(),
            binary: Some(String::from("camera-test")),
        },
        device: CameraDevice {
            id: DeviceId::new(format!("device-{id}")).unwrap(),
            display_name: String::from("Test Camera"),
            node_name: None,
        },
        process_start_time_ticks: 10,
        thread_group_id: 7_200,
        capture_file_descriptor: 3,
        started_at_monotonic_ns: 99,
        last_observed_at_monotonic_ns: 99,
        state: CameraSessionState::Active,
    }
}

struct TestBus {
    child: Child,
    address: String,
}

impl TestBus {
    fn start() -> Self {
        let mut child = Command::new("dbus-daemon")
            .args(["--session", "--nofork", "--print-address=1"])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let stdout = child.stdout.take().unwrap();
        let address = BufReader::new(stdout)
            .lines()
            .next()
            .expect("isolated D-Bus daemon did not print an address")
            .unwrap();
        Self { child, address }
    }

    async fn connect(&self) -> Connection {
        connection::Builder::address(self.address.as_str())
            .unwrap()
            .build()
            .await
            .unwrap()
    }
}

impl Drop for TestBus {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

async fn proxy(connection: &Connection) -> Proxy<'_> {
    Proxy::new(connection, BUS_NAME, OBJECT_PATH, INTERFACE_NAME)
        .await
        .unwrap()
}

struct Pipeline {
    input: mpsc::Sender<MonitorEvent>,
    application: JoinHandle<Result<MonitorState, camera_monitor::application::ApplicationError>>,
    publisher: JoinHandle<Result<DbusService, camera_dbus::ServiceError>>,
}

async fn start_pipeline(bus: &TestBus, initial_state: MonitorState, delay: Duration) -> Pipeline {
    let service = DbusService::on_connection(bus.connect().await, initial_state.clone())
        .await
        .unwrap();
    let (input, incoming) = mpsc::channel(16);
    let (publication, published_rx) = mpsc::channel(16);
    let application = tokio::spawn(run_application(
        incoming,
        publication,
        TestResolver { delay },
        initial_state,
    ));
    let publisher = tokio::spawn(publish_events(published_rx, service));
    Pipeline {
        input,
        application,
        publisher,
    }
}

async fn stop_pipeline(pipeline: Pipeline) {
    drop(pipeline.input);
    timeout(Duration::from_secs(2), pipeline.application)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    let service = timeout(Duration::from_secs(2), pipeline.publisher)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    service.shutdown().await.unwrap();
}

async fn wait_property(client: &Proxy<'_>, name: &str, expected: bool) {
    timeout(Duration::from_secs(2), async {
        loop {
            if client.get_property::<bool>(name).await.unwrap() == expected {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
}

async fn wait_session_name(client: &Proxy<'_>, expected: &str) {
    timeout(Duration::from_secs(2), async {
        loop {
            let sessions: Vec<SessionDto> = client.call("GetActiveSessions", &()).await.unwrap();
            if sessions
                .first()
                .is_some_and(|session| session.application_name == expected)
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
}

async fn wait_session_ids(client: &Proxy<'_>, expected: &[&str]) {
    timeout(Duration::from_secs(2), async {
        loop {
            let sessions: Vec<SessionDto> = client.call("GetActiveSessions", &()).await.unwrap();
            let ids: Vec<_> = sessions
                .iter()
                .map(|session| session.session_id.as_str())
                .collect();
            if ids == expected {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
}

fn process_task_count() -> usize {
    fs::read_dir("/proc/self/task").unwrap().count()
}

fn process_rss_kib() -> usize {
    fs::read_to_string("/proc/self/status")
        .unwrap()
        .lines()
        .find_map(|line| line.strip_prefix("VmRSS:"))
        .and_then(|value| value.split_whitespace().next())
        .unwrap()
        .parse()
        .unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[allow(clippy::await_holding_lock)]
async fn fake_backend_start_update_and_stop_reach_dbus() {
    let _dbus_test = DBUS_TEST_LOCK.lock().unwrap();
    let bus = TestBus::start();
    let pipeline = start_pipeline(&bus, initializing_state(), Duration::ZERO).await;
    let client_connection = bus.connect().await;
    let client = proxy(&client_connection).await;
    let (factory, senders, _) = channel_factory(1);
    let source = senders[0].clone();
    let shutdown = Arc::new(AtomicBool::new(false));
    let supervisor = spawn_supervisor(
        factory,
        pipeline.input.clone(),
        Arc::clone(&shutdown),
        BackoffPolicy::default(),
    );
    wait_property(&client, "ObserverAvailable", true).await;

    let initial = session("one", "Unknown application");
    source
        .send(Ok(MonitorEvent::SessionStarted(initial.clone())))
        .unwrap();
    wait_property(&client, "Active", true).await;
    wait_session_name(&client, "Resolved Camera").await;

    let mut updated = initial.clone();
    updated.application.display_name = String::from("Updated Camera");
    source
        .send(Ok(MonitorEvent::SessionUpdated(updated)))
        .unwrap();
    wait_session_name(&client, "Updated Camera").await;

    source
        .send(Ok(MonitorEvent::SessionStopped(initial.id)))
        .unwrap();
    wait_property(&client, "Active", false).await;
    shutdown.store(true, Ordering::Release);
    assert_eq!(supervisor.await.unwrap(), SupervisorExit::Shutdown);
    stop_pipeline(pipeline).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[allow(clippy::await_holding_lock)]
async fn backend_failure_clears_stale_sessions_and_updates_availability() {
    let _dbus_test = DBUS_TEST_LOCK.lock().unwrap();
    let bus = TestBus::start();
    let pipeline = start_pipeline(&bus, initializing_state(), Duration::ZERO).await;
    let client_connection = bus.connect().await;
    let client = proxy(&client_connection).await;
    let (factory, senders, _) = channel_factory(1);
    let source = senders[0].clone();
    let shutdown = Arc::new(AtomicBool::new(false));
    let supervisor = spawn_supervisor(
        factory,
        pipeline.input.clone(),
        Arc::clone(&shutdown),
        BackoffPolicy::new(Duration::from_millis(25), Duration::from_millis(25), 1),
    );
    wait_property(&client, "ObserverAvailable", true).await;

    source
        .send(Ok(MonitorEvent::SessionStarted(session("stale", "Camera"))))
        .unwrap();
    wait_property(&client, "Active", true).await;
    source.send(Err(FakeError("simulated failure"))).unwrap();
    wait_property(&client, "ObserverAvailable", false).await;
    wait_property(&client, "Active", false).await;

    let sessions: Vec<SessionDto> = client.call("GetActiveSessions", &()).await.unwrap();
    assert!(sessions.is_empty());
    shutdown.store(true, Ordering::Release);
    assert_eq!(supervisor.await.unwrap(), SupervisorExit::Shutdown);
    stop_pipeline(pipeline).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[allow(clippy::await_holding_lock)]
async fn reconnect_snapshot_replaces_state_after_a_missed_remove() {
    let _dbus_test = DBUS_TEST_LOCK.lock().unwrap();
    let bus = TestBus::start();
    let pipeline = start_pipeline(&bus, initializing_state(), Duration::ZERO).await;
    let client_connection = bus.connect().await;
    let client = proxy(&client_connection).await;
    let (factory, senders, _) = channel_factory(2);
    let first = senders[0].clone();
    let second = senders[1].clone();
    let shutdown = Arc::new(AtomicBool::new(false));
    let supervisor = spawn_supervisor(
        factory,
        pipeline.input.clone(),
        Arc::clone(&shutdown),
        BackoffPolicy::new(Duration::from_millis(25), Duration::from_millis(25), 1),
    );

    wait_property(&client, "ObserverAvailable", true).await;
    first
        .send(Ok(MonitorEvent::SessionStarted(session("first", "Camera"))))
        .unwrap();
    wait_property(&client, "Active", true).await;
    first.send(Err(FakeError("simulated disconnect"))).unwrap();
    wait_property(&client, "ObserverAvailable", false).await;
    wait_property(&client, "Active", false).await;
    wait_property(&client, "ObserverAvailable", true).await;

    second
        .send(Ok(MonitorEvent::SessionStarted(session(
            "second",
            "Recovered Camera",
        ))))
        .unwrap();
    second
        .send(Ok(MonitorEvent::SessionStarted(session(
            "third",
            "Snapshot Camera",
        ))))
        .unwrap();
    wait_property(&client, "Active", true).await;
    wait_session_ids(&client, &["second", "third"]).await;

    shutdown.store(true, Ordering::Release);
    assert_eq!(
        timeout(Duration::from_secs(1), supervisor)
            .await
            .unwrap()
            .unwrap(),
        SupervisorExit::Shutdown
    );
    stop_pipeline(pipeline).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[allow(clippy::await_holding_lock)]
async fn application_exit_before_observer_cleanup_keeps_privacy_state_active() {
    let _dbus_test = DBUS_TEST_LOCK.lock().unwrap();
    let bus = TestBus::start();
    let pipeline = start_pipeline(&bus, initializing_state(), Duration::ZERO).await;
    let client_connection = bus.connect().await;
    let client = proxy(&client_connection).await;
    let exiting = session("exited-app", "Exited camera process");

    pipeline
        .input
        .send(MonitorEvent::SessionStarted(exiting.clone()))
        .await
        .unwrap();
    wait_property(&client, "Active", true).await;
    tokio::time::sleep(Duration::from_millis(75)).await;
    assert!(client.get_property::<bool>("Active").await.unwrap());

    pipeline
        .input
        .send(MonitorEvent::SessionStopped(exiting.id))
        .await
        .unwrap();
    wait_property(&client, "Active", false).await;
    stop_pipeline(pipeline).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn slow_resolver_does_not_block_bounded_backend_ingestion() {
    let (factory, senders, observed) = channel_factory(1);
    let source = senders[0].clone();
    let (input, incoming) = mpsc::channel(8);
    let (publication, mut published) = mpsc::channel(8);
    let shutdown = Arc::new(AtomicBool::new(false));
    let supervisor = spawn_supervisor(
        factory,
        input,
        Arc::clone(&shutdown),
        BackoffPolicy::default(),
    );
    let application = tokio::spawn(run_application(
        incoming,
        publication,
        TestResolver {
            delay: Duration::from_millis(200),
        },
        initializing_state(),
    ));

    for id in ["one", "two", "three"] {
        source
            .send(Ok(MonitorEvent::SessionStarted(session(id, "Camera"))))
            .unwrap();
    }
    timeout(Duration::from_millis(100), async {
        while observed.load(Ordering::Acquire) != 3 {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("backend ingestion waited for the slow resolver");

    shutdown.store(true, Ordering::Release);
    assert_eq!(supervisor.await.unwrap(), SupervisorExit::Shutdown);
    let final_state = timeout(Duration::from_secs(2), application)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(final_state.active_session_count(), 3);
    let mut starts = 0;
    while let Some(event) = published.recv().await {
        if matches!(event, MonitorEvent::SessionStarted(_)) {
            starts += 1;
        }
    }
    assert_eq!(starts, 3);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn idle_backend_shutdown_completes_within_a_bounded_duration() {
    let (factory, _senders, _) = channel_factory(1);
    let (input, mut incoming) = mpsc::channel(2);
    let shutdown = Arc::new(AtomicBool::new(false));
    let supervisor = spawn_supervisor(
        factory,
        input,
        Arc::clone(&shutdown),
        BackoffPolicy::default(),
    );
    assert!(matches!(
        incoming.recv().await,
        Some(MonitorEvent::ObserverAvailabilityChanged {
            availability: ObserverAvailability::Available,
            ..
        })
    ));

    let started = Instant::now();
    shutdown.store(true, Ordering::Release);
    let exit = timeout(Duration::from_secs(1), supervisor)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(exit, SupervisorExit::Shutdown);
    assert!(started.elapsed() < Duration::from_secs(1));
}

#[test]
fn repeated_session_churn_keeps_memory_and_task_counts_bounded() {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .max_blocking_threads(1)
        .enable_all()
        .build()
        .unwrap()
        .block_on(repeated_session_churn_keeps_memory_and_task_counts_bounded_async());
}

#[allow(clippy::await_holding_lock)]
async fn repeated_session_churn_keeps_memory_and_task_counts_bounded_async() {
    const CHURN_CYCLES: usize = 2_000;

    let _dbus_test = DBUS_TEST_LOCK.lock().unwrap();
    let bus = TestBus::start();
    let tasks_before = process_task_count();
    let rss_before = process_rss_kib();
    let pipeline = start_pipeline(&bus, initializing_state(), Duration::ZERO).await;
    let client_connection = bus.connect().await;
    let client = proxy(&client_connection).await;

    for index in 0..CHURN_CYCLES {
        let transient = session(&format!("churn-{index}"), "Churn Camera");
        pipeline
            .input
            .send(MonitorEvent::SessionStarted(transient.clone()))
            .await
            .unwrap();
        pipeline
            .input
            .send(MonitorEvent::SessionStopped(transient.id))
            .await
            .unwrap();
    }
    let sentinel = session("churn-sentinel", "Churn Sentinel");
    pipeline
        .input
        .send(MonitorEvent::SessionStarted(sentinel.clone()))
        .await
        .unwrap();
    wait_property(&client, "Active", true).await;
    pipeline
        .input
        .send(MonitorEvent::SessionStopped(sentinel.id))
        .await
        .unwrap();
    wait_property(&client, "Active", false).await;
    stop_pipeline(pipeline).await;
    tokio::time::sleep(Duration::from_millis(50)).await;

    let task_growth = process_task_count().saturating_sub(tasks_before);
    let rss_growth_kib = process_rss_kib().saturating_sub(rss_before);
    eprintln!(
        "LENSGUARD_LONGEVITY cycles={CHURN_CYCLES} task_growth={task_growth} rss_growth_kib={rss_growth_kib}"
    );
    assert!(task_growth <= 3, "task count grew by {task_growth}");
    assert!(
        rss_growth_kib <= 32 * 1_024,
        "resident memory grew by {rss_growth_kib} KiB"
    );
}
