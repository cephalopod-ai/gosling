#[cfg(any(target_os = "linux", all(test, unix)))]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "windows")]
pub use self::windows::WindowsAutomation;

#[cfg(target_os = "macos")]
pub use self::macos::MacOSAutomation;

#[cfg(any(target_os = "linux", target_os = "android"))]
pub use self::linux::LinuxAutomation;

#[allow(dead_code)]
pub trait SystemAutomation: Send + Sync {
    fn execute_system_script(&self, script: &str) -> std::io::Result<String>;
    fn get_shell_command(&self) -> (&'static str, &'static str); // (shell, arg)
    fn get_temp_path(&self) -> std::path::PathBuf;
    fn has_display(&self) -> bool {
        true
    }
}

pub fn create_system_automation() -> Box<dyn SystemAutomation + Send + Sync> {
    #[cfg(target_os = "windows")]
    {
        Box::new(WindowsAutomation)
    }
    #[cfg(target_os = "macos")]
    {
        Box::new(MacOSAutomation)
    }
    #[cfg(not(any(
        target_os = "macos",
        target_os = "windows",
        target_os = "ios",
        target_os = "none"
    )))]
    {
        Box::new(LinuxAutomation::new())
    }
    #[cfg(any(target_os = "ios", target_os = "none"))]
    {
        unimplemented!("Unsupported operating system")
    }
}

#[cfg(any(target_os = "linux", target_os = "windows", test))]
fn checked_output(output: std::process::Output) -> std::io::Result<String> {
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        Err(std::io::Error::other(format!(
            "Automation process exited with {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        )))
    }
}
