/// Extracts a Flatpak application ID from `.flatpak-info` content.
pub(crate) fn app_id_from_flatpak_info(contents: &str) -> Option<String> {
    let mut in_application = false;
    for raw_line in contents.lines() {
        let line = raw_line.trim();
        if line.starts_with('[') && line.ends_with(']') {
            in_application = line == "[Application]";
            continue;
        }
        if in_application
            && let Some(value) = line.strip_prefix("name=")
            && !value.trim().is_empty()
        {
            return Some(value.trim().to_owned());
        }
    }
    None
}

/// Extracts the best-effort Flatpak application ID encoded in a systemd cgroup path.
pub(crate) fn app_id_from_cgroup(contents: &str) -> Option<String> {
    contents.lines().find_map(|line| {
        let path = line.rsplit_once(':').map_or(line, |(_, path)| path);
        let unit = path.rsplit('/').next()?;
        let encoded = unit
            .strip_prefix("app-flatpak-")
            .or_else(|| unit.strip_prefix("flatpak-"))?
            .strip_suffix(".scope")?;
        let (app_id, instance) = encoded.rsplit_once('-')?;
        (!app_id.is_empty() && !instance.is_empty()).then(|| unescape_systemd(app_id))
    })
}

fn unescape_systemd(value: &str) -> String {
    value.replace("\\x2d", "-").replace("\\x2e", ".")
}

#[cfg(test)]
mod tests {
    use super::{app_id_from_cgroup, app_id_from_flatpak_info};

    #[test]
    fn extracts_flatpak_ids_from_supported_metadata() {
        assert_eq!(
            app_id_from_flatpak_info("[Application]\nname=org.example.Camera\nruntime=x"),
            Some(String::from("org.example.Camera"))
        );
        assert_eq!(
            app_id_from_cgroup(
                "0::/user.slice/app.slice/app-flatpak-org.example.Camera-abc123.scope"
            ),
            Some(String::from("org.example.Camera"))
        );
    }
}
