mod bpf;
mod identity;
mod ipc;
mod policy;

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

use bpf::{KernelEvent, KernelOperation};
use camera_core::{
    CaptureConfidence, CaptureOwner, DirectCaptureEvent, DirectCaptureOperation,
    OBSERVER_SCHEMA_VERSION, ObserverAvailability, ObserverMessage, PhysicalCameraId,
};
use ipc::{RoutedMessage, SharedStatus};
use policy::TrustedBrokerPolicy;
use tokio::sync::{RwLock, broadcast, mpsc};

const SOCKET_PATH: &str = "/run/lensguard/v4l2-observer.sock";

#[derive(Clone)]
struct Candidate {
    process_id: u32,
    thread_group_id: u32,
    process_start_time_ticks: u64,
    file_descriptor: i32,
    device: PhysicalCameraId,
    owner: CaptureOwner,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct CandidateKey {
    user_id: u32,
    thread_group_id: u32,
    file_descriptor: i32,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_target(false)
        .compact()
        .init();

    let listener = ipc::bind_socket(Path::new(SOCKET_PATH))?;
    let (messages, _) = broadcast::channel(1024);
    let shared = Arc::new(SharedStatus {
        availability: RwLock::new((
            ObserverAvailability::ConnectionFailed,
            String::from("observer initialization is pending"),
        )),
        messages,
        diagnostics: RwLock::new(BTreeMap::new()),
    });

    let policy = match TrustedBrokerPolicy::load() {
        Ok(policy) => Some(policy),
        Err(error) => {
            tracing::error!(%error, "trusted-broker policy unavailable; all capture is unknown");
            set_availability(
                &shared,
                ObserverAvailability::BlockedByPolicy,
                &error.to_string(),
            )
            .await;
            None
        }
    };

    let (kernel_tx, kernel_rx) = mpsc::channel(1024);
    let attached = match bpf::attach(&kernel_tx) {
        Ok(attached) => {
            if policy.is_some() {
                set_availability(
                    &shared,
                    ObserverAvailability::Available,
                    "direct V4L2 tracepoints attached",
                )
                .await;
            }
            Some(attached)
        }
        Err(error) => {
            let availability = classify_bpf_error(&error.to_string());
            tracing::error!(%error, status = availability.as_str(), "eBPF observer unavailable");
            set_availability(&shared, availability, &error.to_string()).await;
            None
        }
    };

    let server = tokio::spawn(ipc::serve(listener, Arc::clone(&shared)));
    let processor = tokio::spawn(process_kernel_events(
        kernel_rx,
        Arc::clone(&shared),
        policy,
    ));

    tracing::info!(socket = SOCKET_PATH, "LensGuard V4L2 observer ready");
    tokio::signal::ctrl_c().await?;
    set_availability(
        &shared,
        ObserverAvailability::BackendLost,
        "observer is shutting down",
    )
    .await;
    drop(attached);
    server.abort();
    processor.abort();
    let _ = std::fs::remove_file(SOCKET_PATH);
    tracing::info!("LensGuard V4L2 observer stopped; all BPF links detached");
    Ok(())
}

async fn set_availability(shared: &SharedStatus, availability: ObserverAvailability, detail: &str) {
    let bounded_detail: String = detail.chars().take(512).collect();
    *shared.availability.write().await = (availability, bounded_detail.clone());
    let _ = shared.messages.send(RoutedMessage {
        user_id: None,
        message: ObserverMessage::Availability {
            availability,
            detail: bounded_detail,
        },
    });
}

fn classify_bpf_error(error: &str) -> ObserverAvailability {
    let error = error.to_ascii_lowercase();
    if error.contains("lockdown") || error.contains("security") {
        ObserverAvailability::BlockedByPolicy
    } else if error.contains("operation not permitted") || error.contains("permission denied") {
        ObserverAvailability::MissingCapability
    } else if error.contains("tracepoint") || error.contains("not found") {
        ObserverAvailability::UnsupportedKernel
    } else {
        ObserverAvailability::ConnectionFailed
    }
}

async fn process_kernel_events(
    mut incoming: mpsc::Receiver<KernelEvent>,
    shared: Arc<SharedStatus>,
    policy: Option<TrustedBrokerPolicy>,
) {
    let mut candidates = BTreeMap::<CandidateKey, Candidate>::new();
    let mut device_check = tokio::time::interval(std::time::Duration::from_secs(1));
    device_check.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let mut last_timestamp = 0_u64;
    loop {
        let raw = tokio::select! {
            event = incoming.recv() => {
                let Some(event) = event else { break };
                event
            }
            _ = device_check.tick() => {
                remove_missing_devices(
                    &shared,
                    &mut candidates,
                    last_timestamp.saturating_add(1),
                ).await;
                continue;
            }
        };
        last_timestamp = last_timestamp.max(raw.timestamp_ns);
        match raw.operation {
            KernelOperation::StreamStarted => {
                let key = CandidateKey {
                    user_id: raw.user_id,
                    thread_group_id: raw.thread_group_id,
                    file_descriptor: raw.file_descriptor,
                };
                let candidate = build_candidate(raw, policy.as_ref());
                match candidate {
                    Some(candidate) => {
                        publish_candidate(
                            &shared,
                            raw.user_id,
                            &candidate,
                            DirectCaptureOperation::StreamStarted,
                            raw.timestamp_ns,
                            raw.result,
                        )
                        .await;
                        candidates.insert(key, candidate);
                    }
                    None => {
                        increment_suppression(&shared, raw.user_id, CaptureOwner::Unknown).await;
                    }
                }
            }
            KernelOperation::StreamStopped | KernelOperation::DeviceClosed => {
                let key = CandidateKey {
                    user_id: raw.user_id,
                    thread_group_id: raw.thread_group_id,
                    file_descriptor: raw.file_descriptor,
                };
                if let Some(candidate) = candidates.remove(&key) {
                    let operation = if raw.operation == KernelOperation::StreamStopped {
                        DirectCaptureOperation::StreamStopped
                    } else {
                        DirectCaptureOperation::DeviceClosed
                    };
                    publish_candidate(
                        &shared,
                        raw.user_id,
                        &candidate,
                        operation,
                        raw.timestamp_ns,
                        raw.result,
                    )
                    .await;
                }
            }
            KernelOperation::ProcessExited => {
                let keys = candidates
                    .keys()
                    .filter(|key| {
                        key.user_id == raw.user_id && key.thread_group_id == raw.thread_group_id
                    })
                    .copied()
                    .collect::<Vec<_>>();
                for key in keys {
                    if let Some(candidate) = candidates.remove(&key) {
                        publish_candidate(
                            &shared,
                            raw.user_id,
                            &candidate,
                            DirectCaptureOperation::ProcessExited,
                            raw.timestamp_ns,
                            raw.result,
                        )
                        .await;
                    }
                }
            }
        }
    }
}

async fn remove_missing_devices(
    shared: &SharedStatus,
    candidates: &mut BTreeMap<CandidateKey, Candidate>,
    timestamp_monotonic_ns: u64,
) {
    let removed = candidates
        .iter()
        .filter(|(_, candidate)| {
            candidate
                .device
                .udev_syspath
                .as_deref()
                .is_some_and(|path| !Path::new(path).exists())
        })
        .map(|(key, _)| *key)
        .collect::<Vec<_>>();
    for key in removed {
        if let Some(candidate) = candidates.remove(&key) {
            publish_candidate(
                shared,
                key.user_id,
                &candidate,
                DirectCaptureOperation::DeviceRemoved,
                timestamp_monotonic_ns,
                0,
            )
            .await;
        }
    }
}

fn build_candidate(raw: KernelEvent, policy: Option<&TrustedBrokerPolicy>) -> Option<Candidate> {
    let process_start_time_ticks = identity::process_start_time_ticks(raw.thread_group_id).ok()?;
    let device = identity::physical_camera(raw.thread_group_id, raw.file_descriptor).ok()?;
    let owner = policy.map_or(CaptureOwner::Unknown, |policy| {
        policy.classify_process(raw.thread_group_id)
    });
    Some(Candidate {
        process_id: raw.process_id,
        thread_group_id: raw.thread_group_id,
        process_start_time_ticks,
        file_descriptor: raw.file_descriptor,
        device,
        owner,
    })
}

async fn publish_candidate(
    shared: &SharedStatus,
    user_id: u32,
    candidate: &Candidate,
    operation: DirectCaptureOperation,
    timestamp_monotonic_ns: u64,
    result: i64,
) {
    if candidate.owner != CaptureOwner::Direct {
        increment_suppression(shared, user_id, candidate.owner).await;
        return;
    }
    let file_descriptor = if operation == DirectCaptureOperation::ProcessExited {
        -1
    } else {
        candidate.file_descriptor
    };
    let event = DirectCaptureEvent {
        schema_version: OBSERVER_SCHEMA_VERSION,
        timestamp_monotonic_ns,
        process_id: candidate.process_id,
        thread_group_id: candidate.thread_group_id,
        process_start_time_ticks: candidate.process_start_time_ticks,
        file_descriptor,
        device: candidate.device.clone(),
        operation,
        result,
        confidence: CaptureConfidence::Confirmed,
        owner: CaptureOwner::Direct,
    };
    if event.validate().is_ok() {
        let _ = shared.messages.send(RoutedMessage {
            user_id: Some(user_id),
            message: ObserverMessage::Capture {
                event: Box::new(event),
            },
        });
    }
}

async fn increment_suppression(shared: &SharedStatus, user_id: u32, owner: CaptureOwner) {
    let diagnostics = {
        let mut all = shared.diagnostics.write().await;
        let diagnostics = all.entry(user_id).or_default();
        match owner {
            CaptureOwner::BrokerOwned => {
                diagnostics.suppressed_broker_events =
                    diagnostics.suppressed_broker_events.saturating_add(1);
            }
            CaptureOwner::Unknown | CaptureOwner::Direct => {
                diagnostics.suppressed_unknown_events =
                    diagnostics.suppressed_unknown_events.saturating_add(1);
            }
        }
        *diagnostics
    };
    let _ = shared.messages.send(RoutedMessage {
        user_id: Some(user_id),
        message: ObserverMessage::Diagnostics { diagnostics },
    });
}

#[cfg(test)]
mod tests {
    use camera_core::ObserverAvailability;

    use super::classify_bpf_error;

    #[test]
    fn availability_errors_are_actionable() {
        assert_eq!(
            classify_bpf_error("Operation not permitted"),
            ObserverAvailability::MissingCapability
        );
        assert_eq!(
            classify_bpf_error("tracepoint not found"),
            ObserverAvailability::UnsupportedKernel
        );
    }
}
