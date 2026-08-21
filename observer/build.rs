use std::env;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=bpf/v4l2.bpf.c");
    let output =
        PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR is set by Cargo")).join("v4l2.bpf.o");
    let status = Command::new(env::var_os("CLANG").unwrap_or_else(|| "/usr/bin/clang".into()))
        .args([
            "-g",
            "-O2",
            "-target",
            "bpf",
            "-Wall",
            "-Werror",
            "-c",
            "bpf/v4l2.bpf.c",
            "-o",
        ])
        .arg(&output)
        .status()
        .expect("clang is required to build the LensGuard eBPF program");
    assert!(
        status.success(),
        "failed to compile observer/bpf/v4l2.bpf.c"
    );
}
