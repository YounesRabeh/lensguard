use aya::maps::AsyncPerfEventArray;
use aya::programs::TracePoint;
use aya::util::online_cpus;
use aya::{Ebpf, include_bytes_aligned};
use bytes::BytesMut;
use thiserror::Error;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

#[derive(Clone, Copy, Debug)]
pub struct KernelEvent {
    pub timestamp_ns: u64,
    pub process_id: u32,
    pub thread_group_id: u32,
    pub user_id: u32,
    pub file_descriptor: i32,
    pub operation: KernelOperation,
    pub result: i64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KernelOperation {
    StreamStarted,
    StreamStopped,
    DeviceClosed,
    ProcessExited,
}

#[derive(Debug, Error)]
pub enum BpfError {
    #[error("failed to load eBPF object: {0}")]
    Load(#[from] aya::EbpfError),
    #[error("eBPF program or map '{0}' is missing")]
    Missing(&'static str),
    #[error("failed to load or attach eBPF program: {0}")]
    Program(#[from] aya::programs::ProgramError),
    #[error("failed to open eBPF event map: {0}")]
    Map(#[from] aya::maps::MapError),
    #[error("failed to open eBPF perf buffer: {0}")]
    Perf(#[from] aya::maps::perf::PerfBufferError),
    #[error("failed to enumerate online CPUs: {0}")]
    Cpu(std::io::Error),
}

pub struct AttachedBpf {
    _ebpf: Ebpf,
    readers: Vec<JoinHandle<()>>,
}

impl Drop for AttachedBpf {
    fn drop(&mut self) {
        for reader in &self.readers {
            reader.abort();
        }
    }
}

pub fn attach(sender: &mpsc::Sender<KernelEvent>) -> Result<AttachedBpf, BpfError> {
    let mut ebpf = Ebpf::load(include_bytes_aligned!(concat!(
        env!("OUT_DIR"),
        "/v4l2.bpf.o"
    )))?;
    attach_tracepoint(&mut ebpf, "enter_ioctl", "syscalls", "sys_enter_ioctl")?;
    attach_tracepoint(&mut ebpf, "exit_ioctl", "syscalls", "sys_exit_ioctl")?;
    attach_tracepoint(&mut ebpf, "enter_close", "syscalls", "sys_enter_close")?;
    attach_tracepoint(&mut ebpf, "exit_close", "syscalls", "sys_exit_close")?;
    attach_tracepoint(&mut ebpf, "process_exit", "sched", "sched_process_exit")?;

    let map = ebpf.take_map("EVENTS").ok_or(BpfError::Missing("EVENTS"))?;
    let mut events = AsyncPerfEventArray::try_from(map)?;
    let cpus = online_cpus().map_err(|(_, error)| BpfError::Cpu(error))?;
    let mut readers = Vec::with_capacity(cpus.len());
    for cpu in cpus {
        let mut buffer = events.open(cpu, None)?;
        let sender = sender.clone();
        readers.push(tokio::spawn(async move {
            let mut buffers = (0..16)
                .map(|_| BytesMut::with_capacity(64))
                .collect::<Vec<_>>();
            loop {
                match buffer.read_events(&mut buffers).await {
                    Ok(read) => {
                        for payload in buffers.iter().take(read.read) {
                            if let Some(event) = parse_kernel_event(payload) {
                                if sender.send(event).await.is_err() {
                                    return;
                                }
                            }
                        }
                    }
                    Err(error) => {
                        tracing::error!(%error, cpu, "eBPF event reader stopped");
                        return;
                    }
                }
            }
        }));
    }
    Ok(AttachedBpf {
        _ebpf: ebpf,
        readers,
    })
}

fn attach_tracepoint(
    ebpf: &mut Ebpf,
    program_name: &'static str,
    category: &str,
    tracepoint: &str,
) -> Result<(), BpfError> {
    let program: &mut TracePoint = ebpf
        .program_mut(program_name)
        .ok_or(BpfError::Missing(program_name))?
        .try_into()
        .map_err(BpfError::Program)?;
    program.load()?;
    program.attach(category, tracepoint)?;
    Ok(())
}

fn parse_kernel_event(payload: &[u8]) -> Option<KernelEvent> {
    if payload.len() < 40 {
        return None;
    }
    let timestamp_ns = u64::from_ne_bytes(payload[0..8].try_into().ok()?);
    let pid_tgid = u64::from_ne_bytes(payload[8..16].try_into().ok()?);
    let uid_gid = u64::from_ne_bytes(payload[16..24].try_into().ok()?);
    let file_descriptor = i32::from_ne_bytes(payload[24..28].try_into().ok()?);
    let operation = match u32::from_ne_bytes(payload[28..32].try_into().ok()?) {
        1 => KernelOperation::StreamStarted,
        2 => KernelOperation::StreamStopped,
        3 => KernelOperation::DeviceClosed,
        4 => KernelOperation::ProcessExited,
        _ => return None,
    };
    Some(KernelEvent {
        timestamp_ns,
        process_id: u32::try_from(pid_tgid & u64::from(u32::MAX)).ok()?,
        thread_group_id: u32::try_from(pid_tgid >> 32).ok()?,
        user_id: u32::try_from(uid_gid & u64::from(u32::MAX)).ok()?,
        file_descriptor,
        operation,
        result: i64::from_ne_bytes(payload[32..40].try_into().ok()?),
    })
}

#[cfg(test)]
mod tests {
    use super::{KernelOperation, parse_kernel_event};

    #[test]
    fn rejects_short_and_unknown_kernel_events() {
        assert!(parse_kernel_event(&[0; 39]).is_none());
        let mut unknown = [0_u8; 40];
        unknown[28..32].copy_from_slice(&99_u32.to_ne_bytes());
        assert!(parse_kernel_event(&unknown).is_none());
    }

    #[test]
    fn parses_metadata_without_payload_data() {
        let mut raw = [0_u8; 40];
        raw[0..8].copy_from_slice(&42_u64.to_ne_bytes());
        raw[8..16].copy_from_slice(&((7_u64 << 32) | 9).to_ne_bytes());
        raw[16..24].copy_from_slice(&1000_u64.to_ne_bytes());
        raw[24..28].copy_from_slice(&3_i32.to_ne_bytes());
        raw[28..32].copy_from_slice(&1_u32.to_ne_bytes());
        let event = parse_kernel_event(&raw).unwrap();
        assert_eq!(event.thread_group_id, 7);
        assert_eq!(event.process_id, 9);
        assert_eq!(event.operation, KernelOperation::StreamStarted);
    }
}
