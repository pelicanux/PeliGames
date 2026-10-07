// Keep package selection and installer arguments independent of privileged execution.
pub fn distro_format(content: &str) -> Option<&'static str> {
    let mut id = String::new();
    let mut like = String::new();
    for line in content.lines() {
        let Some((key, value)) = line.split_once('=') else { continue; };
        let value = value.trim().trim_matches(|c| c == '"' || c == '\'').to_ascii_lowercase();
        match key.trim() { "ID" => id = value, "ID_LIKE" => like = value, _ => {} }
    }
    for family in std::iter::once(id.as_str()).chain(like.split_whitespace()) {
        match family {
            "debian" | "ubuntu" => return Some(".deb"),
            "nobara" | "fedora" | "rhel" | "centos" | "opensuse" | "opensuse-tumbleweed" | "opensuse-leap" | "suse" | "sles" => return Some(".rpm"),
            _ => {}
        }
    }
    None
}
pub fn package_matches_format(name: &str, format: &str) -> bool {
    matches!(format, ".rpm" | ".deb" | ".appimage") && name.to_ascii_lowercase().ends_with(format)
}
pub fn installer_plan(format: &str, exists: impl Fn(&str) -> bool) -> Option<(&'static str, &'static [&'static str])> {
    let plans: &[(&str, &[&str])] = match format {
        ".deb" => &[("/usr/bin/apt-get", &["install", "-y", "--"]), ("/usr/bin/dpkg", &["-i", "--"])],
        ".rpm" => &[("/usr/bin/dnf", &["install", "-y", "--"]), ("/usr/bin/zypper", &["--non-interactive", "install", "--allow-unsigned-rpm", "--"]), ("/usr/bin/rpm", &["-Uvh", "--"])],
        _ => return None,
    };
    plans.iter().copied().find(|(program, _)| exists(program))
}
pub fn installation_result(code: Option<i32>) -> Result<(), String> {
    match code { Some(0) => Ok(()), Some(126) => Err("authCancelled".into()), Some(127) => Err("pkexecFailed".into()), _ => Err("packageInstallFailed".into()) }
}
