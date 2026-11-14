//! Puts the program icon (icons/napkin-uml.ico) into the Windows .exe, so Explorer, the
//! taskbar and .umlp files show it. Uses the Windows SDK resource compiler (rc.exe) and
//! hands the compiled .res to the linker; no extra crates are needed. If rc.exe can't be
//! found the build still works, just without the embedded icon.

use std::path::{Path, PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=icons/napkin-uml.ico");
    println!("cargo:rerun-if-changed=build.rs");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let Some(rc) = find_rc() else {
        println!("cargo:warning=rc.exe not found: the .exe gets no embedded icon");
        return;
    };
    let dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let ico = dir.join("icons").join("napkin-uml.ico");
    let rc_file = out.join("napkin-uml.rc");
    let res = out.join("napkin-uml.res");
    let ico_text = ico.to_string_lossy().replace('\\', "\\\\");
    std::fs::write(&rc_file, format!("1 ICON \"{ico_text}\"\n")).expect("write .rc");
    let status = std::process::Command::new(&rc).arg("/nologo").arg("/fo").arg(&res).arg(&rc_file).status();
    match status {
        Ok(s) if s.success() => println!("cargo:rustc-link-arg-bins={}", res.display()),
        _ => println!("cargo:warning=rc.exe failed: the .exe gets no embedded icon"),
    }
}

/// rc.exe from RC_EXE, PATH, or the newest Windows 10/11 SDK.
fn find_rc() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("RC_EXE") {
        return Some(PathBuf::from(p));
    }
    if let Ok(path) = std::env::var("PATH") {
        for d in std::env::split_paths(&path) {
            let c = d.join("rc.exe");
            if c.is_file() {
                return Some(c);
            }
        }
    }
    let host = if cfg!(target_arch = "aarch64") { "arm64" } else { "x64" };
    for root in [r"C:\Program Files (x86)\Windows Kits\10\bin", r"C:\Program Files\Windows Kits\10\bin"] {
        let Ok(rd) = std::fs::read_dir(root) else { continue };
        let mut versions: Vec<PathBuf> = rd.flatten().map(|e| e.path()).filter(|p| p.join(host).join("rc.exe").is_file()).collect();
        versions.sort();
        if let Some(v) = versions.last() {
            return Some(v.join(host).join("rc.exe"));
        }
        let flat = Path::new(root).join(host).join("rc.exe");
        if flat.is_file() {
            return Some(flat);
        }
    }
    None
}
