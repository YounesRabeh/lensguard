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
