use std::cell::{Cell, RefCell};
use std::rc::Rc;

use tracing::warn;

use crate::{PipeWireError, RawGraph, RegistryEvent, observer::attach_registry};

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
    let graph_for_events = Rc::clone(&graph);
    let on_event: Rc<dyn Fn(RegistryEvent)> = Rc::new(move |event| {
        if let Err(error) = graph_for_events.borrow_mut().apply(event) {
            warn!(%error, "ignored malformed PipeWire registry metadata");
        }
    });
    let observation = attach_registry(&registry, &on_event);
    let callback_error = Rc::new(RefCell::new(None));

    run_initial_synchronization(&main_loop, &core, &callback_error)?;

    drop(observation);
    drop(on_event);
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
