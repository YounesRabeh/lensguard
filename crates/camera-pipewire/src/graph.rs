use std::collections::{BTreeMap, btree_map::Entry};
use std::fmt::{self, Display, Formatter};

use tracing::{debug, trace};

use crate::MappingError;

/// Owned `PipeWire` properties safe to retain after a registry callback returns.
pub type PropertyMap = BTreeMap<String, String>;

/// Adapter-owned kinds of registry globals relevant to camera graph discovery.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RegistryObjectKind {
    Node,
    Port,
    Link,
    Other(String),
}

/// Synthetic representation of `PipeWire` registry callbacks.
///
/// Tests and future adapters can produce these values without constructing native `PipeWire`
/// objects. All numeric IDs remain inside this adapter crate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RegistryEvent {
    GlobalAdded {
        id: u32,
        kind: RegistryObjectKind,
        properties: PropertyMap,
    },
    NodePropertiesChanged {
        id: u32,
        properties: PropertyMap,
    },
    PortPropertiesChanged {
        id: u32,
        direction: PortDirection,
        properties: PropertyMap,
    },
    LinkPropertiesChanged {
        id: u32,
        output_node_id: u32,
        output_port_id: u32,
        input_node_id: u32,
        input_port_id: u32,
        properties: PropertyMap,
    },
    GlobalRemoved {
        id: u32,
    },
}

/// Classification of a raw node before graph correlation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NodeClassification {
    CameraSourceCandidate,
    ApplicationVideoInputCandidate,
    Other,
}

/// Raw node information retained from the registry.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RawNode {
    pub id: u32,
    pub properties: PropertyMap,
}

impl RawNode {
    /// Classifies the node using its current `media.class` property.
    #[must_use]
    pub fn classification(&self) -> NodeClassification {
        match self.properties.get("media.class").map(String::as_str) {
            Some("Video/Source") => NodeClassification::CameraSourceCandidate,
            Some("Stream/Input/Video") => NodeClassification::ApplicationVideoInputCandidate,
            _ => NodeClassification::Other,
        }
    }

    /// Returns the best available diagnostic name.
    #[must_use]
    pub fn diagnostic_name(&self) -> &str {
        property_or_unknown(
            &self.properties,
            &[
                "node.description",
                "node.nick",
                "application.name",
                "node.name",
                "media.name",
            ],
        )
    }
}

/// Direction of a raw `PipeWire` port.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PortDirection {
    Input,
    Output,
    Unknown,
}

/// Coarse port classification used by later correlation work.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PortClassification {
    VideoInput,
    VideoOutput,
    AudioInput,
    AudioOutput,
    Unknown,
}

/// Raw port information retained from the registry.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RawPort {
    pub id: u32,
    pub node_id: Option<u32>,
    pub direction: PortDirection,
    pub properties: PropertyMap,
}

impl RawPort {
    /// Classifies the port from direction and media-format properties.
    #[must_use]
    pub fn classification(&self) -> PortClassification {
        let media = self
            .properties
            .get("format.dsp")
            .or_else(|| self.properties.get("media.type"))
            .map(|value| value.to_ascii_lowercase());
        let is_video = media
            .as_deref()
            .is_some_and(|value| value.starts_with("video") || value.contains("video/"));
        let is_audio = media
            .as_deref()
            .is_some_and(|value| value.starts_with("audio") || value.contains("audio/"));

        match (self.direction, is_video, is_audio) {
            (PortDirection::Input, true, _) => PortClassification::VideoInput,
            (PortDirection::Output, true, _) => PortClassification::VideoOutput,
            (PortDirection::Input, _, true) => PortClassification::AudioInput,
            (PortDirection::Output, _, true) => PortClassification::AudioOutput,
            _ => PortClassification::Unknown,
        }
    }
}

/// Raw link endpoints. Missing endpoints represent incomplete registry metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RawLink {
    pub id: u32,
    pub output_node_id: Option<u32>,
    pub output_port_id: Option<u32>,
    pub input_node_id: Option<u32>,
    pub input_port_id: Option<u32>,
    pub properties: PropertyMap,
}

impl RawLink {
    /// Returns whether all four link endpoints are known.
    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.output_node_id.is_some()
            && self.output_port_id.is_some()
            && self.input_node_id.is_some()
            && self.input_port_id.is_some()
    }

    fn references_node(&self, id: u32) -> bool {
        self.output_node_id == Some(id) || self.input_node_id == Some(id)
    }

    fn references_port(&self, id: u32) -> bool {
        self.output_port_id == Some(id) || self.input_port_id == Some(id)
    }
}

/// Current raw `PipeWire` graph, deterministically ordered by global ID.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RawGraph {
    nodes: BTreeMap<u32, RawNode>,
    ports: BTreeMap<u32, RawPort>,
    links: BTreeMap<u32, RawLink>,
}

impl RawGraph {
    /// Creates an empty graph.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Applies one synthetic registry event.
    ///
    /// Returns `true` if relevant graph state changed. Repeated identical events and unknown
    /// removals are no-ops.
    ///
    /// # Errors
    ///
    /// Returns [`MappingError`] when a numeric relationship property is malformed.
    pub fn apply(&mut self, event: RegistryEvent) -> Result<bool, MappingError> {
        match event {
            RegistryEvent::GlobalAdded {
                id,
                kind,
                properties,
            } => match kind {
                RegistryObjectKind::Node => Ok(self.upsert_node(id, properties)),
                RegistryObjectKind::Port => self.upsert_port(id, None, properties),
                RegistryObjectKind::Link => self.upsert_link_from_properties(id, properties),
                RegistryObjectKind::Other(_) => Ok(false),
            },
            RegistryEvent::NodePropertiesChanged { id, properties } => {
                Ok(self.upsert_node(id, properties))
            }
            RegistryEvent::PortPropertiesChanged {
                id,
                direction,
                properties,
            } => self.upsert_port(id, Some(direction), properties),
            RegistryEvent::LinkPropertiesChanged {
                id,
                output_node_id,
                output_port_id,
                input_node_id,
                input_port_id,
                properties,
            } => Ok(self.upsert_link(
                id,
                LinkEndpoints {
                    output_node: Some(output_node_id),
                    output_port: Some(output_port_id),
                    input_node: Some(input_node_id),
                    input_port: Some(input_port_id),
                },
                properties,
            )),
            RegistryEvent::GlobalRemoved { id } => Ok(self.remove(id)),
        }
    }

    /// Returns raw nodes ordered by global ID.
    #[must_use]
    pub fn nodes(&self) -> impl ExactSizeIterator<Item = &RawNode> {
        self.nodes.values()
    }

    /// Returns raw ports ordered by global ID.
    #[must_use]
    pub fn ports(&self) -> impl ExactSizeIterator<Item = &RawPort> {
        self.ports.values()
    }

    /// Returns raw links ordered by global ID.
    #[must_use]
    pub fn links(&self) -> impl ExactSizeIterator<Item = &RawLink> {
        self.links.values()
    }

    /// Looks up a raw node by adapter-owned global ID.
    #[must_use]
    pub fn node(&self, id: u32) -> Option<&RawNode> {
        self.nodes.get(&id)
    }

    /// Looks up a raw port by adapter-owned global ID.
    #[must_use]
    pub fn port(&self, id: u32) -> Option<&RawPort> {
        self.ports.get(&id)
    }

    /// Looks up a raw link by adapter-owned global ID.
    #[must_use]
    pub fn link(&self, id: u32) -> Option<&RawLink> {
        self.links.get(&id)
    }

    /// Produces a concise deterministic graph summary for diagnostics.
    #[must_use]
    pub fn diagnostic_summary(&self) -> String {
        self.to_string()
    }

    fn upsert_node(&mut self, id: u32, properties: PropertyMap) -> bool {
        let changed = match self.nodes.entry(id) {
            Entry::Vacant(entry) => {
                entry.insert(RawNode { id, properties });
                true
            }
            Entry::Occupied(mut entry) => {
                merge_properties(&mut entry.get_mut().properties, properties)
            }
        };
        if changed {
            let classification = self.nodes[&id].classification();
            if classification == NodeClassification::Other {
                trace!(node_id = id, ?classification, "PipeWire node updated");
            } else {
                debug!(node_id = id, ?classification, "PipeWire node updated");
            }
        }
        changed
    }

    fn upsert_port(
        &mut self,
        id: u32,
        direction: Option<PortDirection>,
        properties: PropertyMap,
    ) -> Result<bool, MappingError> {
        let property_direction = port_direction_from_properties(&properties);
        let node_id = optional_u32(&properties, "node.id")?;
        let changed = match self.ports.entry(id) {
            Entry::Vacant(entry) => {
                entry.insert(RawPort {
                    id,
                    node_id,
                    direction: direction.unwrap_or(property_direction),
                    properties,
                });
                true
            }
            Entry::Occupied(mut entry) => {
                let port = entry.get_mut();
                let mut changed = merge_properties(&mut port.properties, properties);
                if let Some(node_id) = node_id
                    && port.node_id != Some(node_id)
                {
                    port.node_id = Some(node_id);
                    changed = true;
                }
                let new_direction = direction.unwrap_or(property_direction);
                if new_direction != PortDirection::Unknown && port.direction != new_direction {
                    port.direction = new_direction;
                    changed = true;
                }
                changed
            }
        };
        if changed {
            let classification = self.ports[&id].classification();
            if classification == PortClassification::Unknown {
                trace!(port_id = id, ?classification, "PipeWire port updated");
            } else {
                debug!(port_id = id, ?classification, "PipeWire port updated");
            }
        }
        Ok(changed)
    }

    fn upsert_link_from_properties(
        &mut self,
        id: u32,
        properties: PropertyMap,
    ) -> Result<bool, MappingError> {
        let endpoints = link_endpoints_from_properties(&properties)?;
        Ok(self.upsert_link(id, endpoints, properties))
    }

    fn upsert_link(&mut self, id: u32, endpoints: LinkEndpoints, properties: PropertyMap) -> bool {
        let changed = match self.links.entry(id) {
            Entry::Vacant(entry) => {
                entry.insert(RawLink {
                    id,
                    output_node_id: endpoints.output_node,
                    output_port_id: endpoints.output_port,
                    input_node_id: endpoints.input_node,
                    input_port_id: endpoints.input_port,
                    properties,
                });
                true
            }
            Entry::Occupied(mut entry) => {
                let link = entry.get_mut();
                let mut changed = merge_properties(&mut link.properties, properties);
                changed |= update_if_some(&mut link.output_node_id, endpoints.output_node);
                changed |= update_if_some(&mut link.output_port_id, endpoints.output_port);
                changed |= update_if_some(&mut link.input_node_id, endpoints.input_node);
                changed |= update_if_some(&mut link.input_port_id, endpoints.input_port);
                changed
            }
        };
        if changed {
            debug!(
                link_id = id,
                complete = self.links[&id].is_complete(),
                "PipeWire link updated"
            );
        }
        changed
    }

    fn remove(&mut self, id: u32) -> bool {
        if self.nodes.remove(&id).is_some() {
            self.ports.retain(|_, port| port.node_id != Some(id));
            self.links.retain(|_, link| !link.references_node(id));
            debug!(node_id = id, "PipeWire node removed");
            return true;
        }
        if self.ports.remove(&id).is_some() {
            self.links.retain(|_, link| !link.references_port(id));
            debug!(port_id = id, "PipeWire port removed");
            return true;
        }
        if self.links.remove(&id).is_some() {
            debug!(link_id = id, "PipeWire link removed");
            return true;
        }
        false
    }
}

impl Display for RawGraph {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        let camera_sources: Vec<_> = self
            .nodes()
            .filter(|node| node.classification() == NodeClassification::CameraSourceCandidate)
            .collect();
        let application_inputs: Vec<_> = self
            .nodes()
            .filter(|node| {
                node.classification() == NodeClassification::ApplicationVideoInputCandidate
            })
            .collect();
        let complete_links = self.links().filter(|link| link.is_complete()).count();

        writeln!(
            formatter,
            "PipeWire graph: {} nodes ({} camera-source candidates, {} application video-input candidates), {} ports, {} links ({} complete)",
            self.nodes.len(),
            camera_sources.len(),
            application_inputs.len(),
            self.ports.len(),
            self.links.len(),
            complete_links,
        )?;
        for node in camera_sources {
            writeln!(
                formatter,
                "  camera-source [{}] {:?}",
                node.id,
                node.diagnostic_name()
            )?;
        }
        for node in application_inputs {
            writeln!(
                formatter,
                "  application-input [{}] {:?}",
                node.id,
                node.diagnostic_name()
            )?;
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct LinkEndpoints {
    output_node: Option<u32>,
    output_port: Option<u32>,
    input_node: Option<u32>,
    input_port: Option<u32>,
}

fn link_endpoints_from_properties(properties: &PropertyMap) -> Result<LinkEndpoints, MappingError> {
    Ok(LinkEndpoints {
        output_node: optional_u32(properties, "link.output.node")?,
        output_port: optional_u32(properties, "link.output.port")?,
        input_node: optional_u32(properties, "link.input.node")?,
        input_port: optional_u32(properties, "link.input.port")?,
    })
}

fn optional_u32(properties: &PropertyMap, key: &str) -> Result<Option<u32>, MappingError> {
    properties
        .get(key)
        .map(|value| {
            value
                .parse()
                .map_err(|_| MappingError::InvalidUnsignedInteger {
                    key: key.to_owned(),
                    value: value.clone(),
                })
        })
        .transpose()
}

fn port_direction_from_properties(properties: &PropertyMap) -> PortDirection {
    match properties.get("port.direction").map(String::as_str) {
        Some("in" | "input" | "Input") => PortDirection::Input,
        Some("out" | "output" | "Output") => PortDirection::Output,
        _ => PortDirection::Unknown,
    }
}

fn merge_properties(target: &mut PropertyMap, incoming: PropertyMap) -> bool {
    let mut changed = false;
    for (key, value) in incoming {
        if target.get(&key) != Some(&value) {
            target.insert(key, value);
            changed = true;
        }
    }
    changed
}

fn update_if_some(target: &mut Option<u32>, incoming: Option<u32>) -> bool {
    if incoming.is_some() && *target != incoming {
        *target = incoming;
        true
    } else {
        false
    }
}

fn property_or_unknown<'a>(properties: &'a PropertyMap, keys: &[&str]) -> &'a str {
    keys.iter()
        .find_map(|key| properties.get(*key).map(String::as_str))
        .unwrap_or("unknown")
}

#[cfg(test)]
mod tests {
    use super::{
        NodeClassification, PortClassification, PortDirection, PropertyMap, RawGraph,
        RegistryEvent, RegistryObjectKind,
    };
    use crate::MappingError;

    fn properties(values: &[(&str, &str)]) -> PropertyMap {
        values
            .iter()
            .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
            .collect()
    }

    #[test]
    fn node_classification_handles_expected_and_missing_media_classes() {
        let cases = [
            (
                Some("Video/Source"),
                NodeClassification::CameraSourceCandidate,
            ),
            (
                Some("Stream/Input/Video"),
                NodeClassification::ApplicationVideoInputCandidate,
            ),
            (Some("Audio/Source"), NodeClassification::Other),
            (Some("Stream/Output/Video"), NodeClassification::Other),
            (None, NodeClassification::Other),
        ];

        for (index, (media_class, expected)) in cases.into_iter().enumerate() {
            let mut graph = RawGraph::new();
            let mut node_properties = PropertyMap::new();
            if let Some(media_class) = media_class {
                node_properties.insert(String::from("media.class"), media_class.to_owned());
            }
            graph
                .apply(RegistryEvent::GlobalAdded {
                    id: u32::try_from(index).unwrap(),
                    kind: RegistryObjectKind::Node,
                    properties: node_properties,
                })
                .unwrap();

            assert_eq!(
                graph
                    .node(u32::try_from(index).unwrap())
                    .unwrap()
                    .classification(),
                expected
            );
        }
    }

    #[test]
    fn port_classification_uses_direction_and_media_properties() {
        let cases = [
            ("in", "video/x-raw", PortClassification::VideoInput),
            ("out", "video/x-raw", PortClassification::VideoOutput),
            ("in", "audio/x-raw", PortClassification::AudioInput),
            ("out", "audio/x-raw", PortClassification::AudioOutput),
            ("unknown", "", PortClassification::Unknown),
        ];

        for (index, (direction, format, expected)) in cases.into_iter().enumerate() {
            let mut graph = RawGraph::new();
            graph
                .apply(RegistryEvent::GlobalAdded {
                    id: u32::try_from(index).unwrap(),
                    kind: RegistryObjectKind::Port,
                    properties: properties(&[
                        ("port.direction", direction),
                        ("format.dsp", format),
                    ]),
                })
                .unwrap();

            assert_eq!(
                graph
                    .port(u32::try_from(index).unwrap())
                    .unwrap()
                    .classification(),
                expected
            );
        }
    }

    #[test]
    fn link_parsing_supports_complete_and_incomplete_metadata() {
        let mut graph = RawGraph::new();
        graph
            .apply(RegistryEvent::GlobalAdded {
                id: 50,
                kind: RegistryObjectKind::Link,
                properties: properties(&[
                    ("link.output.node", "10"),
                    ("link.output.port", "11"),
                    ("link.input.node", "20"),
                    ("link.input.port", "21"),
                ]),
            })
            .unwrap();
        assert!(graph.link(50).unwrap().is_complete());

        graph
            .apply(RegistryEvent::GlobalAdded {
                id: 51,
                kind: RegistryObjectKind::Link,
                properties: properties(&[("link.output.node", "10")]),
            })
            .unwrap();
        assert!(!graph.link(51).unwrap().is_complete());
    }

    #[test]
    fn malformed_numeric_properties_return_typed_errors() {
        let mut graph = RawGraph::new();
        let result = graph.apply(RegistryEvent::GlobalAdded {
            id: 50,
            kind: RegistryObjectKind::Link,
            properties: properties(&[("link.output.node", "not-a-number")]),
        });

        assert_eq!(
            result,
            Err(MappingError::InvalidUnsignedInteger {
                key: String::from("link.output.node"),
                value: String::from("not-a-number"),
            })
        );
        assert_eq!(graph.link(50), None);
    }

    #[test]
    fn late_metadata_reclassifies_existing_objects() {
        let mut graph = RawGraph::new();
        graph
            .apply(RegistryEvent::GlobalAdded {
                id: 10,
                kind: RegistryObjectKind::Node,
                properties: PropertyMap::new(),
            })
            .unwrap();
        assert_eq!(
            graph.node(10).unwrap().classification(),
            NodeClassification::Other
        );

        graph
            .apply(RegistryEvent::NodePropertiesChanged {
                id: 10,
                properties: properties(&[("media.class", "Video/Source")]),
            })
            .unwrap();
        assert_eq!(
            graph.node(10).unwrap().classification(),
            NodeClassification::CameraSourceCandidate
        );
    }

    #[test]
    fn link_info_completes_late_endpoint_metadata() {
        let mut graph = RawGraph::new();
        graph
            .apply(RegistryEvent::GlobalAdded {
                id: 50,
                kind: RegistryObjectKind::Link,
                properties: PropertyMap::new(),
            })
            .unwrap();

        graph
            .apply(RegistryEvent::LinkPropertiesChanged {
                id: 50,
                output_node_id: 10,
                output_port_id: 11,
                input_node_id: 20,
                input_port_id: 21,
                properties: PropertyMap::new(),
            })
            .unwrap();

        assert!(graph.link(50).unwrap().is_complete());
    }

    #[test]
    fn object_removal_cleans_nodes_ports_and_connected_links() {
        let mut graph = RawGraph::new();
        for event in [
            RegistryEvent::GlobalAdded {
                id: 10,
                kind: RegistryObjectKind::Node,
                properties: PropertyMap::new(),
            },
            RegistryEvent::GlobalAdded {
                id: 11,
                kind: RegistryObjectKind::Port,
                properties: properties(&[("node.id", "10")]),
            },
            RegistryEvent::GlobalAdded {
                id: 50,
                kind: RegistryObjectKind::Link,
                properties: properties(&[
                    ("link.output.node", "10"),
                    ("link.output.port", "11"),
                    ("link.input.node", "20"),
                    ("link.input.port", "21"),
                ]),
            },
        ] {
            graph.apply(event).unwrap();
        }

        assert!(
            graph
                .apply(RegistryEvent::GlobalRemoved { id: 10 })
                .unwrap()
        );
        assert_eq!(graph.node(10), None);
        assert_eq!(graph.port(11), None);
        assert_eq!(graph.link(50), None);
        assert!(
            !graph
                .apply(RegistryEvent::GlobalRemoved { id: 10 })
                .unwrap()
        );
    }

    #[test]
    fn explicit_port_info_overrides_missing_registry_direction() {
        let mut graph = RawGraph::new();
        graph
            .apply(RegistryEvent::PortPropertiesChanged {
                id: 11,
                direction: PortDirection::Output,
                properties: properties(&[("format.dsp", "video/x-raw")]),
            })
            .unwrap();

        assert_eq!(
            graph.port(11).unwrap().classification(),
            PortClassification::VideoOutput
        );
    }

    #[test]
    fn diagnostic_summary_is_concise_and_deterministic() {
        let mut graph = RawGraph::new();
        graph
            .apply(RegistryEvent::GlobalAdded {
                id: 20,
                kind: RegistryObjectKind::Node,
                properties: properties(&[
                    ("media.class", "Stream/Input/Video"),
                    ("application.name", "Video Chat"),
                ]),
            })
            .unwrap();
        graph
            .apply(RegistryEvent::GlobalAdded {
                id: 10,
                kind: RegistryObjectKind::Node,
                properties: properties(&[
                    ("media.class", "Video/Source"),
                    ("node.description", "USB Camera"),
                ]),
            })
            .unwrap();

        let summary = graph.diagnostic_summary();
        assert!(summary.contains("1 camera-source candidates"));
        assert!(summary.contains("1 application video-input candidates"));
        assert!(summary.contains("camera-source [10] \"USB Camera\""));
        assert!(summary.contains("application-input [20] \"Video Chat\""));
    }
}
