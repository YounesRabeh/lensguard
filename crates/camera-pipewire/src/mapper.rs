#[cfg(test)]
use crate::{MappingError, PropertyMap, RegistryEvent, RegistryObjectKind};

#[cfg(test)]
pub(crate) fn parse_fixture(input: &str) -> Result<Vec<RegistryEvent>, MappingError> {
    let mut events = Vec::new();
    let mut id = None;
    let mut kind = None;
    let mut properties = PropertyMap::new();
    let mut last_line = 0;

    for (index, raw_line) in input.lines().enumerate() {
        let line_number = index + 1;
        last_line = line_number;
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line == "---" {
            push_fixture_event(
                &mut events,
                &mut id,
                &mut kind,
                &mut properties,
                line_number,
            )?;
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            return Err(MappingError::InvalidFixture {
                line: line_number,
                details: String::from("expected key=value"),
            });
        };
        match key {
            "@id" => {
                id = Some(value.parse().map_err(|_| MappingError::InvalidFixture {
                    line: line_number,
                    details: format!("invalid @id value '{value}'"),
                })?);
            }
            "@type" => {
                kind = Some(match value {
                    "Node" => RegistryObjectKind::Node,
                    "Port" => RegistryObjectKind::Port,
                    "Link" => RegistryObjectKind::Link,
                    other => {
                        return Err(MappingError::UnsupportedFixtureObjectType {
                            line: line_number,
                            value: other.to_owned(),
                        });
                    }
                });
            }
            _ if key.starts_with('@') => {
                return Err(MappingError::InvalidFixture {
                    line: line_number,
                    details: format!("unknown fixture field '{key}'"),
                });
            }
            _ => {
                properties.insert(key.to_owned(), value.to_owned());
            }
        }
    }

    if id.is_some() || kind.is_some() || !properties.is_empty() {
        push_fixture_event(&mut events, &mut id, &mut kind, &mut properties, last_line)?;
    }
    Ok(events)
}

#[cfg(test)]
fn push_fixture_event(
    events: &mut Vec<RegistryEvent>,
    id: &mut Option<u32>,
    kind: &mut Option<RegistryObjectKind>,
    properties: &mut PropertyMap,
    line: usize,
) -> Result<(), MappingError> {
    let id = id
        .take()
        .ok_or(MappingError::MissingFixtureField { line, field: "@id" })?;
    let kind = kind.take().ok_or(MappingError::MissingFixtureField {
        line,
        field: "@type",
    })?;
    events.push(RegistryEvent::GlobalAdded {
        id,
        kind,
        properties: std::mem::take(properties),
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::parse_fixture;
    use crate::{NodeClassification, RawGraph, RawNode};

    const PHYSICAL_CAMERA: &str =
        include_str!("../../../tests/fixtures/pipewire/physical-camera.properties");
    const APPLICATION_INPUT: &str =
        include_str!("../../../tests/fixtures/pipewire/application-video-input.properties");
    const UNRELATED_AUDIO: &str =
        include_str!("../../../tests/fixtures/pipewire/unrelated-audio.properties");
    const VIDEO_OUTPUT: &str =
        include_str!("../../../tests/fixtures/pipewire/unrelated-video-output.properties");

    #[test]
    fn sanitized_fixtures_classify_expected_nodes() {
        let mut graph = RawGraph::new();
        for fixture in [
            PHYSICAL_CAMERA,
            APPLICATION_INPUT,
            UNRELATED_AUDIO,
            VIDEO_OUTPUT,
        ] {
            for event in parse_fixture(fixture).unwrap() {
                graph.apply(event).unwrap();
            }
        }

        let classifications: Vec<_> = graph.nodes().map(RawNode::classification).collect();
        assert_eq!(
            classifications,
            [
                NodeClassification::CameraSourceCandidate,
                NodeClassification::ApplicationVideoInputCandidate,
                NodeClassification::Other,
                NodeClassification::Other,
            ]
        );
    }

    #[test]
    fn fixture_parser_reports_malformed_input() {
        let error = parse_fixture("@type=Node\nmedia.class=Video/Source").unwrap_err();
        assert!(error.to_string().contains("missing @id"));
    }
}
