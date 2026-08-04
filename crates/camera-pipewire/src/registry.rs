use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use pipewire::link::Link;
use pipewire::node::Node;
use pipewire::port::Port;
use pipewire::proxy::{Listener, ProxyT};
use pipewire::spa::utils::Direction;
use pipewire::types::ObjectType;
use tracing::{trace, warn};

use crate::{
    PipeWireError, PortDirection, PropertyMap, RawGraph, RegistryEvent, RegistryObjectKind,
};

struct BoundObject {
    _proxy: Box<dyn ProxyT>,
    _listener: Box<dyn Listener>,
}

/// Connects to the current user's `PipeWire` instance and returns an initial raw graph snapshot.
///
/// Two synchronization barriers allow initial global callbacks and property-info callbacks to be
/// processed before the function returns.
///
/// # Errors
///
/// Returns [`PipeWireError`] when initialization, connection, registry acquisition, server
/// synchronization, or a core callback fails.
pub fn inspect_pipewire() -> Result<RawGraph, PipeWireError> {
    pipewire::init();

    let main_loop = pipewire::main_loop::MainLoopRc::new(None).map_err(PipeWireError::MainLoop)?;
    let context =
        pipewire::context::ContextRc::new(&main_loop, None).map_err(PipeWireError::Context)?;
    let core = context.connect_rc(None).map_err(PipeWireError::Connect)?;
    let registry = core.get_registry_rc().map_err(PipeWireError::Registry)?;

    let graph = Rc::new(RefCell::new(RawGraph::new()));
    let bound_objects = Rc::new(RefCell::new(HashMap::<u32, BoundObject>::new()));
    let callback_error = Rc::new(RefCell::new(None));

    let registry_weak = registry.downgrade();
    let graph_for_global = Rc::clone(&graph);
    let bound_for_global = Rc::clone(&bound_objects);
    let graph_for_remove = Rc::clone(&graph);
    let bound_for_remove = Rc::clone(&bound_objects);
    let registry_listener = registry
        .add_listener_local()
        .global(move |object| {
            let properties = copy_properties(object.props.as_ref().map(AsRef::as_ref));
            let kind = object_kind(&object.type_);
            apply_event(
                &graph_for_global,
                RegistryEvent::GlobalAdded {
                    id: object.id,
                    kind,
                    properties,
                },
            );

            let Some(registry) = registry_weak.upgrade() else {
                warn!(
                    object_id = object.id,
                    "PipeWire registry disappeared before object binding"
                );
                return;
            };
            bind_relevant_object(&registry, object, &graph_for_global, &bound_for_global);
        })
        .global_remove(move |id| {
            bound_for_remove.borrow_mut().remove(&id);
            apply_event(&graph_for_remove, RegistryEvent::GlobalRemoved { id });
        })
        .register();

    run_initial_synchronization(&main_loop, &core, &callback_error)?;

    drop(registry_listener);
    drop(bound_objects);
    Rc::try_unwrap(graph)
        .map(RefCell::into_inner)
        .map_err(|_| PipeWireError::Core {
            object_id: pipewire::core::PW_ID_CORE,
            result: -1,
            message: String::from("internal graph references remained after inspection"),
        })
}

fn run_initial_synchronization(
    main_loop: &pipewire::main_loop::MainLoopRc,
    core: &pipewire::core::CoreRc,
    callback_error: &Rc<RefCell<Option<PipeWireError>>>,
) -> Result<(), PipeWireError> {
    let first_barrier = core.sync(0).map_err(PipeWireError::Synchronize)?;
    let pending_barrier = Rc::new(Cell::new(first_barrier));
    let barrier_phase = Rc::new(Cell::new(0_u8));
    let main_loop_weak = main_loop.downgrade();
    let core_weak = core.downgrade();
    let pending_for_done = Rc::clone(&pending_barrier);
    let phase_for_done = Rc::clone(&barrier_phase);
    let error_for_done = Rc::clone(callback_error);
    let main_loop_for_error = main_loop.downgrade();
    let error_for_core = Rc::clone(callback_error);
    let core_listener = core
        .add_listener_local()
        .done(move |id, sequence| {
            if id != pipewire::core::PW_ID_CORE || sequence != pending_for_done.get() {
                return;
            }
            if phase_for_done.get() == 0 {
                let Some(core) = core_weak.upgrade() else {
                    *error_for_done.borrow_mut() = Some(PipeWireError::Core {
                        object_id: id,
                        result: -1,
                        message: String::from("core disappeared during initial synchronization"),
                    });
                    if let Some(main_loop) = main_loop_weak.upgrade() {
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
                        *error_for_done.borrow_mut() = Some(PipeWireError::Synchronize(error));
                        if let Some(main_loop) = main_loop_weak.upgrade() {
                            main_loop.quit();
                        }
                    }
                }
            } else if let Some(main_loop) = main_loop_weak.upgrade() {
                main_loop.quit();
            }
        })
        .error(move |object_id, _sequence, result, message| {
            *error_for_core.borrow_mut() = Some(PipeWireError::Core {
                object_id,
                result,
                message: message.to_owned(),
            });
            if let Some(main_loop) = main_loop_for_error.upgrade() {
                main_loop.quit();
            }
        })
        .register();

    main_loop.run();
    drop(core_listener);
    callback_error.borrow_mut().take().map_or(Ok(()), Err)
}

fn bind_relevant_object<P>(
    registry: &pipewire::registry::RegistryRc,
    object: &pipewire::registry::GlobalObject<P>,
    graph: &Rc<RefCell<RawGraph>>,
    bound_objects: &Rc<RefCell<HashMap<u32, BoundObject>>>,
) where
    P: AsRef<pipewire::spa::utils::dict::DictRef>,
{
    let id = object.id;
    let result: Result<BoundObject, pipewire::Error> = match object.type_ {
        ObjectType::Node => registry.bind::<Node, _>(object).map(|node| {
            let graph = Rc::clone(graph);
            let listener = node
                .add_listener_local()
                .info(move |info| {
                    apply_event(
                        &graph,
                        RegistryEvent::NodePropertiesChanged {
                            id: info.id(),
                            properties: copy_properties(info.props()),
                        },
                    );
                })
                .register();
            BoundObject {
                _proxy: Box::new(node),
                _listener: Box::new(listener),
            }
        }),
        ObjectType::Port => registry.bind::<Port, _>(object).map(|port| {
            let graph = Rc::clone(graph);
            let listener = port
                .add_listener_local()
                .info(move |info| {
                    apply_event(
                        &graph,
                        RegistryEvent::PortPropertiesChanged {
                            id: info.id(),
                            direction: convert_direction(info.direction()),
                            properties: copy_properties(info.props()),
                        },
                    );
                })
                .register();
            BoundObject {
                _proxy: Box::new(port),
                _listener: Box::new(listener),
            }
        }),
        ObjectType::Link => registry.bind::<Link, _>(object).map(|link| {
            let graph = Rc::clone(graph);
            let listener = link
                .add_listener_local()
                .info(move |info| {
                    apply_event(
                        &graph,
                        RegistryEvent::LinkPropertiesChanged {
                            id: info.id(),
                            output_node_id: info.output_node_id(),
                            output_port_id: info.output_port_id(),
                            input_node_id: info.input_node_id(),
                            input_port_id: info.input_port_id(),
                            properties: copy_properties(info.props()),
                        },
                    );
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

fn apply_event(graph: &Rc<RefCell<RawGraph>>, event: RegistryEvent) {
    if let Err(error) = graph.borrow_mut().apply(event) {
        warn!(%error, "ignored malformed PipeWire registry metadata");
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
