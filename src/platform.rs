//! Operating-system specific helpers.

use std::process::Command;

/// A `Command` that does not flash a console window on Windows.
pub fn hidden_command(program: &str) -> Command {
    #[allow(unused_mut)]
    let mut cmd = Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}

pub fn open_browser(url: &str) {
    #[cfg(windows)]
    let _ = hidden_command("cmd").args(["/C", "start", "", url]).spawn();
    #[cfg(target_os = "macos")]
    let _ = Command::new("open").arg(url).spawn();
    #[cfg(all(unix, not(target_os = "macos")))]
    let _ = Command::new("xdg-open").arg(url).spawn();
}

/// Keeps the console open so a double-clicked executable can show its error.
pub fn wait_for_enter() {
    eprintln!("Press Enter to exit...");
    let mut line = String::new();
    let _ = std::io::stdin().read_line(&mut line);
}

/// Current UTC time of day as `HH:MM:SS` (no time-zone database needed).
pub fn utc_clock(now_ms: u128) -> String {
    let secs = (now_ms / 1000) % 86_400;
    format!("{:02}:{:02}:{:02}", secs / 3600, (secs / 60) % 60, secs % 60)
}
