use std::fs;
use std::os::unix::fs::{FileTypeExt, MetadataExt};
use std::path::{Path, PathBuf};

use camera_core::PhysicalCameraId;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum IdentityError {
    #[error("capture process or file descriptor disappeared")]
    Missing,
    #[error("capture file descriptor is not a V4L2 character device")]
    NotV4l2,
}

pub fn process_start_time_ticks(process_id: u32) -> Result<u64, IdentityError> {
    let stat = fs::read_to_string(format!("/proc/{process_id}/stat"))
        .map_err(|_| IdentityError::Missing)?;
    let fields = stat
        .rsplit_once(')')
        .ok_or(IdentityError::Missing)?
        .1
        .split_whitespace()
        .collect::<Vec<_>>();
    fields
        .get(19)
        .and_then(|value| value.parse().ok())
        .filter(|value| *value > 0)
        .ok_or(IdentityError::Missing)
}

pub fn physical_camera(process_id: u32, fd: i32) -> Result<PhysicalCameraId, IdentityError> {
    let fd_path = PathBuf::from(format!("/proc/{process_id}/fd/{fd}"));
    let node = fs::read_link(&fd_path).map_err(|_| IdentityError::Missing)?;
    let metadata = fs::metadata(&fd_path).map_err(|_| IdentityError::Missing)?;
    if !metadata.file_type().is_char_device() {
        return Err(IdentityError::NotV4l2);
    }
    let major = linux_major(metadata.rdev());
    let minor = linux_minor(metadata.rdev());
    let sys_char = PathBuf::from(format!("/sys/dev/char/{major}:{minor}"));
    let uevent = fs::read_to_string(sys_char.join("uevent"))
        .or_else(|_| fs::read_to_string(sys_char.join("device/uevent")))
        .map_err(|_| IdentityError::NotV4l2)?;
    let devname = uevent
        .lines()
        .find_map(|line| line.strip_prefix("DEVNAME="))
        .ok_or(IdentityError::NotV4l2)?;
    if !devname.starts_with("video") {
        return Err(IdentityError::NotV4l2);
    }

    let device_path = fs::canonicalize(sys_char.join("device")).ok();
    let class_path = PathBuf::from("/sys/class/video4linux").join(devname);
    Ok(PhysicalCameraId {
        major,
        minor,
        udev_syspath: device_path.as_ref().map(|path| path.display().to_string()),
        media_device: None,
        serial: find_parent_value(device_path.as_deref(), "serial"),
        hardware_path: device_path.as_ref().map(|path| path.display().to_string()),
        vendor_id: find_parent_value(device_path.as_deref(), "idVendor"),
        product_id: find_parent_value(device_path.as_deref(), "idProduct"),
        bus_info: node.to_str().map(str::to_owned),
        driver: find_driver(device_path.as_deref()),
        display_name: read_trimmed(&class_path.join("name")),
    })
}

fn find_parent_value(start: Option<&Path>, name: &str) -> Option<String> {
    start?
        .ancestors()
        .take(10)
        .find_map(|path| read_trimmed(&path.join(name)))
}

fn find_driver(start: Option<&Path>) -> Option<String> {
    start?.ancestors().take(10).find_map(|path| {
        fs::read_link(path.join("driver")).ok().and_then(|driver| {
            driver
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
        })
    })
}

fn read_trimmed(path: &Path) -> Option<String> {
    fs::read_to_string(path)
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn linux_major(device: u64) -> u32 {
    let value = ((device >> 8) & 0xfff) | ((device >> 32) & 0xffff_f000);
    u32::try_from(value).unwrap_or_default()
}

fn linux_minor(device: u64) -> u32 {
    let value = (device & 0xff) | ((device >> 12) & 0xffff_ff00);
    u32::try_from(value).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::{linux_major, linux_minor};

    #[test]
    fn linux_device_number_round_trip_known_values() {
        let encoded = (81_u64 << 8) | 3;
        assert_eq!(linux_major(encoded), 81);
        assert_eq!(linux_minor(encoded), 3);
    }
}
