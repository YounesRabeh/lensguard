use camera_core::MonitorEvent;

use super::CorrelationEngine;
use crate::{PropertyMap, RegistryEvent, RegistryObjectKind, mapper::parse_fixture};

fn properties(values: &[(&str, &str)]) -> PropertyMap {
    values
        .iter()
        .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
        .collect()
}

fn node(id: u32, media_class: &str, name: &str) -> RegistryEvent {
    RegistryEvent::GlobalAdded {
        id,
        kind: RegistryObjectKind::Node,
        properties: properties(&[("media.class", media_class), ("node.description", name)]),
    }
}

fn port(id: u32, node_id: u32, direction: &str, media: &str) -> RegistryEvent {
    RegistryEvent::GlobalAdded {
        id,
        kind: RegistryObjectKind::Port,
        properties: properties(&[
            ("node.id", &node_id.to_string()),
            ("port.direction", direction),
            ("format.dsp", media),
        ]),
    }
}

fn link(
    id: u32,
    output_node: u32,
    output_port: u32,
    input_node: u32,
    input_port: u32,
) -> RegistryEvent {
    RegistryEvent::GlobalAdded {
        id,
        kind: RegistryObjectKind::Link,
        properties: properties(&[
            ("link.output.node", &output_node.to_string()),
            ("link.output.port", &output_port.to_string()),
            ("link.input.node", &input_node.to_string()),
            ("link.input.port", &input_port.to_string()),
        ]),
    }
}

fn apply_all(
    engine: &mut CorrelationEngine,
    events: impl IntoIterator<Item = RegistryEvent>,
    timestamp: u64,
) -> Vec<MonitorEvent> {
    events
        .into_iter()
        .flat_map(|event| engine.apply(event, timestamp).unwrap())
        .collect()
}

fn one_relationship() -> Vec<RegistryEvent> {
    vec![
        node(10, "Video/Source", "Integrated Camera"),
        port(11, 10, "out", "video/x-raw"),
        node(20, "Stream/Input/Video", "Camera App"),
        port(21, 20, "in", "video/x-raw"),
        link(50, 10, 11, 20, 21),
    ]
}

fn started(event: &MonitorEvent) -> &camera_core::CameraSession {
    let MonitorEvent::SessionStarted(session) = event else {
        panic!("expected SessionStarted, got {event:?}");
    };
    session
}

#[test]
fn link_creation_emits_one_start_and_duplicate_information_is_quiet() {
    let mut engine = CorrelationEngine::new();
    let events = apply_all(&mut engine, one_relationship(), 1_000);
    assert_eq!(events.len(), 1);
    let session = started(&events[0]);
    assert_eq!(session.application.display_name, "Camera App");
    assert_eq!(session.device.display_name, "Integrated Camera");
    assert_eq!(session.started_at_unix_ms, 1_000);
    assert!(session.id.as_str().starts_with("pipewire-"));
    assert_ne!(session.id.as_str(), "10-20");

    let duplicate = engine.apply(link(50, 10, 11, 20, 21), 2_000).unwrap();
    assert!(duplicate.is_empty());
    assert_eq!(engine.active_sessions().len(), 1);
}

#[test]
fn link_removal_emits_exactly_one_stop() {
    let mut engine = CorrelationEngine::new();
    let start = apply_all(&mut engine, one_relationship(), 1_000);
    let session_id = started(&start[0]).id.clone();

    assert_eq!(
        engine
            .apply(RegistryEvent::GlobalRemoved { id: 50 }, 2_000)
            .unwrap(),
        [MonitorEvent::SessionStopped(session_id)]
    );
    assert!(
        engine
            .apply(RegistryEvent::GlobalRemoved { id: 50 }, 3_000)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn harmless_link_replacement_does_not_churn_the_relationship() {
    let mut engine = CorrelationEngine::new();
    let start = apply_all(&mut engine, one_relationship(), 1_000);
    let session_id = started(&start[0]).id.clone();

    assert!(
        engine
            .apply(link(51, 10, 11, 20, 21), 2_000)
            .unwrap()
            .is_empty()
    );
    assert!(
        engine
            .apply(RegistryEvent::GlobalRemoved { id: 50 }, 3_000)
            .unwrap()
            .is_empty()
    );
    assert_eq!(engine.active_sessions().len(), 1);
    assert_eq!(
        engine
            .apply(RegistryEvent::GlobalRemoved { id: 51 }, 4_000)
            .unwrap(),
        [MonitorEvent::SessionStopped(session_id)]
    );
}

#[test]
fn node_removal_before_link_removal_emits_exactly_one_stop() {
    let mut engine = CorrelationEngine::new();
    let start = apply_all(&mut engine, one_relationship(), 1_000);
    let session_id = started(&start[0]).id.clone();

    assert_eq!(
        engine
            .apply(RegistryEvent::GlobalRemoved { id: 20 }, 2_000)
            .unwrap(),
        [MonitorEvent::SessionStopped(session_id)]
    );
    assert!(
        engine
            .apply(RegistryEvent::GlobalRemoved { id: 50 }, 3_000)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn late_application_metadata_emits_update_without_identity_churn() {
    let mut engine = CorrelationEngine::new();
    let initial = vec![
        node(10, "Video/Source", "Integrated Camera"),
        port(11, 10, "out", "video/x-raw"),
        node(20, "Stream/Input/Video", ""),
        port(21, 20, "in", "video/x-raw"),
        link(50, 10, 11, 20, 21),
    ];
    let start = apply_all(&mut engine, initial, 1_000);
    let original = started(&start[0]);
    assert_eq!(original.application.display_name, "Unknown application");

    let emitted = engine
        .apply(
            RegistryEvent::NodePropertiesChanged {
                id: 20,
                properties: properties(&[
                    ("application.name", "Video Chat"),
                    ("application.process.id", "4242"),
                ]),
            },
            9_000,
        )
        .unwrap();
    let [MonitorEvent::SessionUpdated(updated_session)] = emitted.as_slice() else {
        panic!("expected one session update, got {emitted:?}");
    };
    assert_eq!(updated_session.id, original.id);
    assert_eq!(updated_session.started_at_unix_ms, 1_000);
    assert_eq!(updated_session.application.display_name, "Video Chat");
    assert_eq!(updated_session.application.pid, Some(4242));
}

#[test]
fn unrelated_audio_and_video_output_graphs_emit_nothing() {
    for events in [
        vec![
            node(10, "Audio/Source", "Microphone"),
            port(11, 10, "out", "audio/x-raw"),
            node(20, "Stream/Input/Audio", "Recorder"),
            port(21, 20, "in", "audio/x-raw"),
            link(50, 10, 11, 20, 21),
        ],
        vec![
            node(10, "Video/Source", "Camera"),
            port(11, 10, "out", "video/x-raw"),
            node(20, "Stream/Output/Video", "Video Player"),
            port(21, 20, "in", "video/x-raw"),
            link(50, 10, 11, 20, 21),
        ],
    ] {
        let mut engine = CorrelationEngine::new();
        assert!(apply_all(&mut engine, events, 1_000).is_empty());
    }
}

#[test]
fn two_applications_can_share_one_camera() {
    let mut engine = CorrelationEngine::new();
    let mut events = one_relationship();
    events.extend([
        node(30, "Stream/Input/Video", "Second App"),
        port(31, 30, "in", "video/x-raw"),
        link(51, 10, 11, 30, 31),
    ]);

    let domain_events = apply_all(&mut engine, events, 1_000);
    assert_eq!(domain_events.len(), 2);
    assert!(
        domain_events
            .iter()
            .all(|event| matches!(event, MonitorEvent::SessionStarted(_)))
    );
    assert_eq!(engine.active_sessions().len(), 2);
}

#[test]
fn one_application_switching_cameras_has_distinct_lifecycles() {
    let mut engine = CorrelationEngine::new();
    let first = apply_all(&mut engine, one_relationship(), 1_000);
    let first_id = started(&first[0]).id.clone();
    let second_camera = apply_all(
        &mut engine,
        [
            node(30, "Video/Source", "USB Camera"),
            port(31, 30, "out", "video/x-raw"),
            link(51, 30, 31, 20, 21),
        ],
        2_000,
    );
    let second_id = started(&second_camera[0]).id.clone();
    assert_ne!(first_id, second_id);
    assert_eq!(engine.active_sessions().len(), 2);

    assert_eq!(
        engine
            .apply(RegistryEvent::GlobalRemoved { id: 50 }, 3_000)
            .unwrap(),
        [MonitorEvent::SessionStopped(first_id)]
    );
    assert_eq!(engine.active_sessions().len(), 1);
}

#[test]
fn backend_restart_stops_old_state_and_rebuilds_cleanly() {
    let mut engine = CorrelationEngine::new();
    let first = apply_all(&mut engine, one_relationship(), 1_000);
    let first_id = started(&first[0]).id.clone();

    let reset = engine.backend_unavailable("PipeWire disconnected");
    assert_eq!(
        reset,
        [
            MonitorEvent::SessionStopped(first_id.clone()),
            MonitorEvent::BackendUnavailable {
                reason: String::from("PipeWire disconnected"),
            },
        ]
    );
    assert_eq!(engine.active_sessions().len(), 0);

    let rebuilt = apply_all(&mut engine, one_relationship(), 5_000);
    let rebuilt_session = started(&rebuilt[0]);
    assert_eq!(rebuilt_session.id, first_id);
    assert_eq!(rebuilt_session.started_at_unix_ms, 5_000);
}

#[test]
fn sanitized_session_fixture_drives_a_complete_lifecycle() {
    let fixture = include_str!("../../../../tests/fixtures/pipewire/camera-session.properties");
    let mut engine = CorrelationEngine::new();
    let raw_events = parse_fixture(fixture).unwrap();
    let events = apply_all(&mut engine, raw_events, 1_000);

    assert_eq!(events.len(), 1);
    let session = started(&events[0]);
    assert_eq!(session.application.display_name, "Fixture Camera App");
    assert_eq!(session.device.display_name, "Fixture USB Camera");
}

#[test]
fn out_of_order_registry_events_reconcile_when_graph_becomes_complete() {
    let mut engine = CorrelationEngine::new();
    let events = apply_all(
        &mut engine,
        [
            link(50, 10, 11, 20, 21),
            port(21, 20, "in", "video/x-raw"),
            node(20, "Stream/Input/Video", "Camera App"),
            port(11, 10, "out", "video/x-raw"),
            node(10, "Video/Source", "Integrated Camera"),
        ],
        1_000,
    );

    assert_eq!(events.len(), 1);
    assert!(matches!(events[0], MonitorEvent::SessionStarted(_)));
}

#[test]
fn camera_unplug_stops_capture_before_late_link_cleanup() {
    let mut engine = CorrelationEngine::new();
    let start = apply_all(&mut engine, one_relationship(), 1_000);
    let id = started(&start[0]).id.clone();

    assert_eq!(
        engine
            .apply(RegistryEvent::GlobalRemoved { id: 10 }, 2_000)
            .unwrap(),
        [MonitorEvent::SessionStopped(id)]
    );
    assert!(
        engine
            .apply(RegistryEvent::GlobalRemoved { id: 50 }, 3_000)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn replug_changed_raw_ids_preserves_stable_device_identity() {
    let mut engine = CorrelationEngine::new();
    let mut first_graph = one_relationship();
    first_graph.insert(
        1,
        RegistryEvent::NodePropertiesChanged {
            id: 10,
            properties: properties(&[("node.name", "v4l2-stable-camera")]),
        },
    );
    let first = apply_all(&mut engine, first_graph, 1_000);
    let first = started(&first[0]).clone();
    engine
        .apply(RegistryEvent::GlobalRemoved { id: 10 }, 2_000)
        .unwrap();

    let second = apply_all(
        &mut engine,
        [
            node(30, "Video/Source", "Integrated Camera"),
            RegistryEvent::NodePropertiesChanged {
                id: 30,
                properties: properties(&[("node.name", "v4l2-stable-camera")]),
            },
            port(31, 30, "out", "video/x-raw"),
            node(40, "Stream/Input/Video", "Camera App"),
            port(41, 40, "in", "video/x-raw"),
            link(60, 30, 31, 40, 41),
        ],
        3_000,
    );
    let second = started(&second[0]);

    assert_eq!(second.device.id, first.device.id);
    assert_ne!(second.id, first.id);
}

#[test]
fn identical_camera_names_remain_distinct_devices() {
    let mut engine = CorrelationEngine::new();
    let mut events = one_relationship();
    events.insert(
        1,
        RegistryEvent::NodePropertiesChanged {
            id: 10,
            properties: properties(&[
                ("node.name", "camera-node-a"),
                ("device.name", "identical-model"),
            ]),
        },
    );
    events.extend([
        node(30, "Video/Source", "Integrated Camera"),
        RegistryEvent::NodePropertiesChanged {
            id: 30,
            properties: properties(&[
                ("node.name", "camera-node-b"),
                ("device.name", "identical-model"),
            ]),
        },
        port(31, 30, "out", "video/x-raw"),
        link(51, 30, 31, 20, 21),
    ]);
    let starts = apply_all(&mut engine, events, 1_000);
    let sessions: Vec<_> = starts.iter().map(started).collect();

    assert_eq!(sessions.len(), 2);
    assert_eq!(
        sessions[0].device.display_name,
        sessions[1].device.display_name
    );
    assert_ne!(sessions[0].device.id, sessions[1].device.id);
}

#[test]
fn two_processes_from_same_desktop_app_remain_distinct_sessions() {
    let mut engine = CorrelationEngine::new();
    let mut events = one_relationship();
    events.extend([
        RegistryEvent::NodePropertiesChanged {
            id: 20,
            properties: properties(&[
                ("application.id", "org.example.Camera"),
                ("application.process.id", "1001"),
            ]),
        },
        node(30, "Stream/Input/Video", "Camera App"),
        RegistryEvent::NodePropertiesChanged {
            id: 30,
            properties: properties(&[
                ("application.id", "org.example.Camera"),
                ("application.process.id", "1002"),
            ]),
        },
        port(31, 30, "in", "video/x-raw"),
        link(51, 10, 11, 30, 31),
    ]);
    let starts = apply_all(&mut engine, events, 1_000);
    let sessions: Vec<_> = starts
        .iter()
        .filter_map(|event| match event {
            MonitorEvent::SessionStarted(session) => Some(session),
            MonitorEvent::SessionUpdated(_)
            | MonitorEvent::SessionStopped(_)
            | MonitorEvent::BackendUnavailable { .. }
            | MonitorEvent::BackendRecovered => None,
        })
        .collect();

    assert_eq!(engine.active_sessions().len(), 2);
    assert_eq!(sessions.last().unwrap().application.pid, Some(1002));
    assert_eq!(
        sessions.last().unwrap().application.app_id.as_deref(),
        Some("org.example.Camera")
    );
}

#[test]
fn two_cameras_and_two_applications_have_four_independent_relationships() {
    let mut engine = CorrelationEngine::new();
    let events = [
        node(10, "Video/Source", "Camera A"),
        port(11, 10, "out", "video/x-raw"),
        node(20, "Video/Source", "Camera B"),
        port(21, 20, "out", "video/x-raw"),
        node(30, "Stream/Input/Video", "App A"),
        port(31, 30, "in", "video/x-raw"),
        node(40, "Stream/Input/Video", "App B"),
        port(41, 40, "in", "video/x-raw"),
        link(50, 10, 11, 30, 31),
        link(51, 10, 11, 40, 41),
        link(52, 20, 21, 30, 31),
        link(53, 20, 21, 40, 41),
    ];

    assert_eq!(apply_all(&mut engine, events, 1_000).len(), 4);
    assert_eq!(engine.active_sessions().len(), 4);
}

#[test]
fn malformed_relationship_metadata_is_rejected_without_poisoning_engine() {
    let mut engine = CorrelationEngine::new();
    let malformed = RegistryEvent::GlobalAdded {
        id: 50,
        kind: RegistryObjectKind::Link,
        properties: properties(&[("link.output.node", "not-a-number")]),
    };
    assert!(engine.apply(malformed, 1_000).is_err());
    assert_eq!(apply_all(&mut engine, one_relationship(), 2_000).len(), 1);
}
