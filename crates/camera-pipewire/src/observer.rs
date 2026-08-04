use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use pipewire::link::Link;
use pipewire::node::Node;
use pipewire::port::Port;
use pipewire::proxy::{Listener as ProxyListener, ProxyT};
use pipewire::spa::utils::Direction;
use pipewire::types::ObjectType;
use tracing::{trace, warn};

use crate::{PortDirection, PropertyMap, RegistryEvent, RegistryObjectKind};

type EventCallback = Rc<dyn Fn(RegistryEvent)>;

struct BoundObject {
    _proxy: Box<dyn ProxyT>,
    _listener: Box<dyn ProxyListener>,
}

/// Owns native listeners and proxies for as long as registry observation is required.
pub(crate) struct RegistryObservation {
    _listener: pipewire::registry::Listener,
    _bound_objects: Rc<RefCell<HashMap<u32, BoundObject>>>,
}

pub(crate) fn attach_registry(
    registry: &pipewire::registry::RegistryRc,
    on_event: &EventCallback,
) -> RegistryObservation {
    let bound_objects = Rc::new(RefCell::new(HashMap::<u32, BoundObject>::new()));
    let registry_weak = registry.downgrade();
    let event_for_global = Rc::clone(on_event);
    let bound_for_global = Rc::clone(&bound_objects);
    let event_for_remove = Rc::clone(on_event);
    let bound_for_remove = Rc::clone(&bound_objects);
    let listener = registry
        .add_listener_local()
        .global(move |object| {
            event_for_global(RegistryEvent::GlobalAdded {
                id: object.id,
                kind: object_kind(&object.type_),
                properties: copy_properties(object.props.as_ref().map(AsRef::as_ref)),
            });

            let Some(registry) = registry_weak.upgrade() else {
                warn!(
                    object_id = object.id,
                    "PipeWire registry disappeared before object binding"
                );
                return;
            };
            bind_relevant_object(&registry, object, &event_for_global, &bound_for_global);
        })
        .global_remove(move |id| {
            bound_for_remove.borrow_mut().remove(&id);
            event_for_remove(RegistryEvent::GlobalRemoved { id });
        })
        .register();

    RegistryObservation {
        _listener: listener,
        _bound_objects: bound_objects,
    }
}

fn bind_relevant_object<P>(
    registry: &pipewire::registry::RegistryRc,
    object: &pipewire::registry::GlobalObject<P>,
    on_event: &EventCallback,
    bound_objects: &Rc<RefCell<HashMap<u32, BoundObject>>>,
) where
    P: AsRef<pipewire::spa::utils::dict::DictRef>,
{
    let id = object.id;
    let result: Result<BoundObject, pipewire::Error> = match object.type_ {
        ObjectType::Node => registry.bind::<Node, _>(object).map(|node| {
            let on_event = Rc::clone(on_event);
            let listener = node
                .add_listener_local()
                .info(move |info| {
                    on_event(RegistryEvent::NodePropertiesChanged {
                        id: info.id(),
                        properties: copy_properties(info.props()),
                    });
                })
                .register();
            BoundObject {
                _proxy: Box::new(node),
                _listener: Box::new(listener),
            }
        }),
        ObjectType::Port => registry.bind::<Port, _>(object).map(|port| {
            let on_event = Rc::clone(on_event);
            let listener = port
                .add_listener_local()
                .info(move |info| {
                    on_event(RegistryEvent::PortPropertiesChanged {
                        id: info.id(),
                        direction: convert_direction(info.direction()),
                        properties: copy_properties(info.props()),
                    });
                })
                .register();
            BoundObject {
                _proxy: Box::new(port),
                _listener: Box::new(listener),
            }
        }),
        ObjectType::Link => registry.bind::<Link, _>(object).map(|link| {
            let on_event = Rc::clone(on_event);
            let listener = link
                .add_listener_local()
                .info(move |info| {
                    on_event(RegistryEvent::LinkPropertiesChanged {
                        id: info.id(),
                        output_node_id: info.output_node_id(),
                        output_port_id: info.output_port_id(),
                        input_node_id: info.input_node_id(),
                        input_port_id: info.input_port_id(),
                        properties: copy_properties(info.props()),
                    });
                })
                .register();
            BoundObject {
                _proxy: Box::new(link),
                _listener: Box::new(listener),
            }
        }),
        _ => return,
    };

    match result {
        Ok(bound) => {
            bound_objects.borrow_mut().insert(id, bound);
            trace!(object_id = id, object_type = %object.type_, "bound PipeWire registry object");
        }
        Err(error) => {
            warn!(object_id = id, object_type = %object.type_, %error, "failed to bind PipeWire registry object");
        }
    }
}

fn copy_properties(properties: Option<&pipewire::spa::utils::dict::DictRef>) -> PropertyMap {
    properties
        .map(|properties| {
            properties
                .iter()
                .map(|(key, value)| (key.to_owned(), value.to_owned()))
                .collect()
        })
        .unwrap_or_default()
}

fn object_kind(object_type: &ObjectType) -> RegistryObjectKind {
    match object_type {
        ObjectType::Node => RegistryObjectKind::Node,
        ObjectType::Port => RegistryObjectKind::Port,
        ObjectType::Link => RegistryObjectKind::Link,
        other => RegistryObjectKind::Other(other.to_string()),
    }
}

fn convert_direction(direction: Direction) -> PortDirection {
    if direction == Direction::Input {
        PortDirection::Input
    } else if direction == Direction::Output {
        PortDirection::Output
    } else {
        PortDirection::Unknown
    }
}
