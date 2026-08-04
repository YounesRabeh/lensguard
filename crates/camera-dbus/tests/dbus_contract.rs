use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use camera_core::{
    ApplicationIdentity, CameraDevice, CameraSession, DetectionBackend, DeviceId, MonitorEvent,
    MonitorState, SessionId,
};
use camera_dbus::{
    BUS_NAME, DbusService, INTERFACE_NAME, OBJECT_PATH, PING_RESPONSE, SessionDto, VERSION,
};
use futures_util::StreamExt;
use tokio::time::timeout;
use zbus::{Connection, Proxy, connection};

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
        let mut lines = BufReader::new(stdout).lines();
        let address = lines.next().unwrap_or_else(|| {
            let mut stderr = String::new();
            if let Some(mut pipe) = child.stderr.take() {
                use std::io::Read;
                pipe.read_to_string(&mut stderr).unwrap();
            }
            panic!("isolated dbus-daemon exited before printing an address: {stderr}");
        });
        let address = address.unwrap();
        assert!(!address.is_empty());
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

fn session(id: &str) -> CameraSession {
    CameraSession {
        id: SessionId::new(id).unwrap(),
        application: ApplicationIdentity {
            pid: Some(7_200),
            app_id: Some(String::from("org.example.Camera")),
            display_name: String::from("Example Camera"),
            binary: Some(String::from("example-camera")),
        },
        device: CameraDevice {
            id: DeviceId::new(format!("device-{id}")).unwrap(),
            display_name: String::from("Front Camera"),
            node_name: None,
        },
        started_at_unix_ms: 99,
        backend: DetectionBackend::PipeWire,
    }
}

async fn proxy(connection: &Connection) -> Proxy<'_> {
    Proxy::new(connection, BUS_NAME, OBJECT_PATH, INTERFACE_NAME)
        .await
        .unwrap()
}

#[tokio::test(flavor = "current_thread")]
async fn service_owns_name_and_exposes_initial_contract_then_releases_cleanly() {
    let bus = TestBus::start();
    let server_connection = bus.connect().await;
    let service = DbusService::on_connection(server_connection, MonitorState::new())
        .await
        .unwrap();
    let client_connection = bus.connect().await;
    let client = proxy(&client_connection).await;

    let pong: String = client.call("Ping", &()).await.unwrap();
    assert_eq!(pong, PING_RESPONSE);
    assert!(!client.get_property::<bool>("Active").await.unwrap());
    assert_eq!(
        client
            .get_property::<u32>("ActiveSessionCount")
            .await
            .unwrap(),
        0
    );
    assert!(
        client
            .get_property::<bool>("BackendAvailable")
            .await
            .unwrap()
    );
    assert_eq!(
        client.get_property::<String>("Version").await.unwrap(),
        VERSION
    );
    let sessions: Vec<SessionDto> = client.call("GetActiveSessions", &()).await.unwrap();
    assert!(sessions.is_empty());

    let introspection = zbus::fdo::IntrospectableProxy::builder(&client_connection)
        .destination(BUS_NAME)
        .unwrap()
        .path(OBJECT_PATH)
        .unwrap()
        .build()
        .await
        .unwrap()
        .introspect()
        .await
        .unwrap();
    assert!(introspection.contains(INTERFACE_NAME));
    assert!(introspection.contains("GetActiveSessions"));
    assert!(introspection.contains("type=\"a(sssssstu)\""));

    service.shutdown().await.unwrap();
    drop(client);

    let replacement_connection = bus.connect().await;
    let replacement = DbusService::on_connection(replacement_connection, MonitorState::new())
        .await
        .unwrap();
    replacement.shutdown().await.unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn start_event_updates_properties_method_and_signals() {
    let bus = TestBus::start();
    let service = DbusService::on_connection(bus.connect().await, MonitorState::new())
        .await
        .unwrap();
    let client_connection = bus.connect().await;
    let client = proxy(&client_connection).await;
    let mut active_changes = client.receive_property_changed::<bool>("Active").await;
    let mut count_changes = client
        .receive_property_changed::<u32>("ActiveSessionCount")
        .await;
    let initial_active = timeout(Duration::from_secs(2), active_changes.next())
        .await
        .unwrap()
        .unwrap();
    assert!(!initial_active.get().await.unwrap());
    let initial_count = timeout(Duration::from_secs(2), count_changes.next())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(initial_count.get().await.unwrap(), 0);
    let mut state_changes = client.receive_signal("StateChanged").await.unwrap();
    let mut starts = client.receive_signal("SessionStarted").await.unwrap();

    let active_session = session("stable-one");
    assert!(
        service
            .apply_event(MonitorEvent::SessionStarted(active_session.clone()))
            .await
            .unwrap()
    );
    let active_change = timeout(Duration::from_secs(2), active_changes.next())
        .await
        .unwrap()
        .unwrap();
    assert!(active_change.get().await.unwrap());
    let count_change = timeout(Duration::from_secs(2), count_changes.next())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(count_change.get().await.unwrap(), 1);
    let state_signal = timeout(Duration::from_secs(2), state_changes.next())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        state_signal
            .body()
            .deserialize::<(bool, u32, bool)>()
            .unwrap(),
        (true, 1, true)
    );
    let start_signal = timeout(Duration::from_secs(2), starts.next())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        start_signal
            .body()
            .deserialize::<SessionDto>()
            .unwrap()
            .session_id,
        "stable-one"
    );
    assert!(client.get_property::<bool>("Active").await.unwrap());
    let sessions: Vec<SessionDto> = client.call("GetActiveSessions", &()).await.unwrap();
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0], SessionDto::from(&active_session));

    service.shutdown().await.unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn stop_event_updates_properties_and_signals() {
    let bus = TestBus::start();
    let active_session = session("stable-one");
    let mut initial_state = MonitorState::new();
    initial_state.apply(MonitorEvent::SessionStarted(active_session.clone()));
    let service = DbusService::on_connection(bus.connect().await, initial_state)
        .await
        .unwrap();
    let client_connection = bus.connect().await;
    let client = proxy(&client_connection).await;
    let mut active_changes = client.receive_property_changed::<bool>("Active").await;
    let mut count_changes = client
        .receive_property_changed::<u32>("ActiveSessionCount")
        .await;
    assert!(
        timeout(Duration::from_secs(2), active_changes.next())
            .await
            .unwrap()
            .unwrap()
            .get()
            .await
            .unwrap()
    );
    assert_eq!(
        timeout(Duration::from_secs(2), count_changes.next())
            .await
            .unwrap()
            .unwrap()
            .get()
            .await
            .unwrap(),
        1
    );
    let mut state_changes = client.receive_signal("StateChanged").await.unwrap();

    let mut stops = client.receive_signal("SessionStopped").await.unwrap();
    assert!(
        service
            .apply_event(MonitorEvent::SessionStopped(active_session.id.clone()))
            .await
            .unwrap()
    );
    let stop_signal = timeout(Duration::from_secs(2), stops.next())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        stop_signal.body().deserialize::<String>().unwrap(),
        "stable-one"
    );
    let inactive_change = timeout(Duration::from_secs(2), active_changes.next())
        .await
        .unwrap()
        .unwrap();
    assert!(!inactive_change.get().await.unwrap());
    let empty_count_change = timeout(Duration::from_secs(2), count_changes.next())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(empty_count_change.get().await.unwrap(), 0);
    let stopped_state_signal = timeout(Duration::from_secs(2), state_changes.next())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        stopped_state_signal
            .body()
            .deserialize::<(bool, u32, bool)>()
            .unwrap(),
        (false, 0, true)
    );
    assert!(!client.get_property::<bool>("Active").await.unwrap());
    assert!(service.snapshot().await.unwrap().active_sessions.is_empty());

    service.shutdown().await.unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn backend_event_updates_property_and_signal() {
    let bus = TestBus::start();
    let service = DbusService::on_connection(bus.connect().await, MonitorState::new())
        .await
        .unwrap();
    let client_connection = bus.connect().await;
    let client = proxy(&client_connection).await;
    let mut availability = client
        .receive_signal("BackendAvailabilityChanged")
        .await
        .unwrap();
    let mut backend_changes = client
        .receive_property_changed::<bool>("BackendAvailable")
        .await;
    let initial_backend = timeout(Duration::from_secs(2), backend_changes.next())
        .await
        .unwrap()
        .unwrap();
    assert!(initial_backend.get().await.unwrap());
    service
        .apply_event(MonitorEvent::BackendUnavailable {
            reason: String::from("test backend stopped"),
        })
        .await
        .unwrap();
    let unavailable = timeout(Duration::from_secs(2), availability.next())
        .await
        .unwrap()
        .unwrap();
    assert!(!unavailable.body().deserialize::<bool>().unwrap());
    let backend_change = timeout(Duration::from_secs(2), backend_changes.next())
        .await
        .unwrap()
        .unwrap();
    assert!(!backend_change.get().await.unwrap());
    assert!(
        !client
            .get_property::<bool>("BackendAvailable")
            .await
            .unwrap()
    );

    service.shutdown().await.unwrap();
}
