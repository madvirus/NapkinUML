//! Lets Windows open .umlp project files with this program (double-click in Explorer).
//! Registered per user under HKEY_CURRENT_USER\Software\Classes, so no administrator
//! rights are needed; it is refreshed at each start so it follows the program if moved.

#[cfg(target_os = "windows")]
pub fn register_file_type() {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let Ok(exe) = std::env::current_exe() else { return };
    let exe = exe.to_string_lossy().to_string();
    let open = format!("\"{exe}\" \"%1\"");
    let icon = format!("\"{exe}\",0");
    let entries: [(&str, Option<&str>, String); 4] = [
        (r"HKCU\Software\Classes\.umlp", None, "NapkinUML.Project".to_string()),
        (r"HKCU\Software\Classes\NapkinUML.Project", None, "NapkinUML Project".to_string()),
        (r"HKCU\Software\Classes\NapkinUML.Project\DefaultIcon", None, icon),
        (r"HKCU\Software\Classes\NapkinUML.Project\shell\open\command", None, open),
    ];
    std::thread::spawn(move || {
        for (key, _name, value) in entries {
            let _ = std::process::Command::new("reg")
                .args(["add", key, "/ve", "/d", &value, "/f"])
                .creation_flags(CREATE_NO_WINDOW)
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status();
        }
    });
}

#[cfg(not(target_os = "windows"))]
pub fn register_file_type() {}
