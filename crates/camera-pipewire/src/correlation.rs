use std::collections::BTreeMap;

use camera_core::{
    ApplicationIdentity, CameraDevice, CameraSession, DetectionBackend, DeviceId, MonitorEvent,
    SessionId,
};
use tracing::debug;

use crate::{
    CorrelationError, NodeClassification, PortDirection, PropertyMap, RawGraph, RawNode,
    RegistryEvent,
};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct RelationshipKey {
    camera_node: u32,
    application_node: u32,
}

/// Stateful conversion from raw `PipeWire` graph changes to domain camera-session events.
///
/// A relationship is active only when a complete link connects an output port owned by a
/// `Video/Source` node to an input port owned by a `Stream/Input/Video` node. Multiple links for
/// the same node pair represent one session. Raw object IDs are used only as private correlation
/// keys and are converted to opaque hashes before crossing the domain boundary.
#[derive(Clone, Debug, Default)]
pub struct CorrelationEngine {
    graph: RawGraph,
    active: BTreeMap<RelationshipKey, CameraSession>,
}

impl CorrelationEngine {
    /// Creates an empty correlation engine.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Applies one raw registry event and returns the resulting domain lifecycle events.
    ///
    /// `observed_at_unix_ms` becomes the start timestamp only for newly active relationships.
    /// Existing sessions retain their original timestamp across metadata and link updates.
    ///
    /// # Errors
    ///
    /// Returns [`CorrelationError`] for malformed raw relationship metadata or invalid domain
    /// identifiers.
    pub fn apply(
        &mut self,
        event: RegistryEvent,
        observed_at_unix_ms: u64,
    ) -> Result<Vec<MonitorEvent>, CorrelationError> {
        self.graph.apply(event)?;
        self.reconcile(observed_at_unix_ms)
    }

    /// Clears all raw state after a backend failure and ends each active relationship once.
    ///
    /// The final event marks the backend unavailable so consumers never confuse loss of
    /// observation with an inactive camera.
    #[must_use]
    pub fn backend_unavailable(&mut self, reason: impl Into<String>) -> Vec<MonitorEvent> {
        let mut events = self
            .active
            .values()
            .map(|session| MonitorEvent::SessionStopped(session.id.clone()))
            .collect::<Vec<_>>();
        self.active.clear();
        self.graph = RawGraph::new();
        events.push(MonitorEvent::BackendUnavailable {
            reason: reason.into(),
        });
        events
    }

    /// Returns the current active sessions in deterministic relationship order.
    #[must_use]
    pub fn active_sessions(&self) -> impl ExactSizeIterator<Item = &CameraSession> {
        self.active.values()
    }

    fn reconcile(
        &mut self,
        observed_at_unix_ms: u64,
    ) -> Result<Vec<MonitorEvent>, CorrelationError> {
        let next = self.derive_sessions(observed_at_unix_ms)?;
        let mut events = Vec::new();

        for (key, active_session) in &self.active {
            if !next.contains_key(key) {
                debug!(session_id = %active_session.id, "camera relationship stopped");
                events.push(MonitorEvent::SessionStopped(active_session.id.clone()));
            }
        }
        for (key, next_session) in &next {
            match self.active.get(key) {
                None => {
                    debug!(session_id = %next_session.id, "camera relationship started");
                    events.push(MonitorEvent::SessionStarted(next_session.clone()));
                }
                Some(active_session) if active_session != next_session => {
                    debug!(session_id = %next_session.id, "camera relationship metadata updated");
                    events.push(MonitorEvent::SessionUpdated(next_session.clone()));
                }
                Some(_) => {}
            }
        }

        self.active = next;
        Ok(events)
    }

    fn derive_sessions(
        &self,
        observed_at_unix_ms: u64,
    ) -> Result<BTreeMap<RelationshipKey, CameraSession>, CorrelationError> {
        let mut sessions = BTreeMap::new();
        for link in self.graph.links() {
            let Some((camera_node, output_port, application_node, input_port)) = link
                .output_node_id
                .zip(link.output_port_id)
                .zip(link.input_node_id.zip(link.input_port_id))
                .map(
                    |((camera_node, output_port), (application_node, input_port))| {
                        (camera_node, output_port, application_node, input_port)
                    },
                )
            else {
                continue;
            };
            let key = RelationshipKey {
                camera_node,
                application_node,
            };
            let Some((camera, application)) =
                self.camera_relationship_nodes(key, output_port, input_port)
            else {
                continue;
            };

            let started_at = self
                .active
                .get(&key)
                .map_or(observed_at_unix_ms, |session| session.started_at_unix_ms);
            sessions.entry(key).or_insert(Self::build_session(
                key,
                camera,
                application,
                started_at,
            )?);
        }
        Ok(sessions)
    }

    fn camera_relationship_nodes(
        &self,
        key: RelationshipKey,
        output_port_id: u32,
        input_port_id: u32,
    ) -> Option<(&RawNode, &RawNode)> {
        let camera = self.graph.node(key.camera_node)?;
        let application = self.graph.node(key.application_node)?;
        let output_port = self.graph.port(output_port_id)?;
        let input_port = self.graph.port(input_port_id)?;

        (camera.classification() == NodeClassification::CameraSourceCandidate
            && application.classification() == NodeClassification::ApplicationVideoInputCandidate
            && output_port.node_id == Some(key.camera_node)
            && output_port.direction == PortDirection::Output
            && input_port.node_id == Some(key.application_node)
            && input_port.direction == PortDirection::Input)
            .then_some((camera, application))
    }

    fn build_session(
        key: RelationshipKey,
        camera: &RawNode,
        application: &RawNode,
        started_at_unix_ms: u64,
    ) -> Result<CameraSession, CorrelationError> {
        let session_hash = stable_hash(&[
            b"pipewire-session",
            &key.camera_node.to_le_bytes(),
            &key.application_node.to_le_bytes(),
        ]);
        let device_identity = property(
            &camera.properties,
            &["device.serial", "node.name", "device.name", "object.serial"],
        )
        .map_or_else(
            || {
                format!(
                    "node-{:016x}",
                    stable_hash(&[&key.camera_node.to_le_bytes()])
                )
            },
            ToOwned::to_owned,
        );
        let device_hash = stable_hash(&[b"pipewire-device", device_identity.as_bytes()]);

        Ok(CameraSession {
            id: SessionId::new(format!("pipewire-{session_hash:016x}"))?,
            application: ApplicationIdentity {
                pid: property(&application.properties, &["application.process.id"])
                    .and_then(|value| value.parse().ok()),
                app_id: property(
                    &application.properties,
                    &["application.id", "pipewire.access.portal.app_id"],
                )
                .map(ToOwned::to_owned),
                display_name: property(
                    &application.properties,
                    &[
                        "application.name",
                        "node.description",
                        "node.nick",
                        "node.name",
                    ],
                )
                .unwrap_or("Unknown application")
                .to_owned(),
                binary: property(&application.properties, &["application.process.binary"])
                    .map(ToOwned::to_owned),
            },
            device: CameraDevice {
                id: DeviceId::new(format!("pipewire-device-{device_hash:016x}"))?,
                display_name: property(
                    &camera.properties,
                    &["node.description", "node.nick", "node.name"],
                )
                .unwrap_or("Unknown camera")
                .to_owned(),
                node_name: property(&camera.properties, &["node.name"]).map(ToOwned::to_owned),
            },
            started_at_unix_ms,
            backend: DetectionBackend::PipeWire,
        })
    }
}

fn property<'a>(properties: &'a PropertyMap, keys: &[&str]) -> Option<&'a str> {
    keys.iter()
        .find_map(|key| properties.get(*key).map(String::as_str))
        .filter(|value| !value.trim().is_empty())
}

fn stable_hash(parts: &[&[u8]]) -> u64 {
    const OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;

    let mut hash = OFFSET_BASIS;
    for part in parts {
        for byte in *part {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(PRIME);
        }
        hash ^= 0xff;
        hash = hash.wrapping_mul(PRIME);
    }
    hash
}

#[cfg(test)]
mod tests;
