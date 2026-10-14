#![cfg(unix)]

use gosling_mcp::subprocess::{merged_path, user_login_path};
use std::os::unix::fs::PermissionsExt;
use std::process::Command;

#[test]
fn concurrent_consumers_cache_success_and_failure() {
    if let Ok(marker) = std::env::var("GOSLING_TEST_LOGIN_PROBES") {
        let success = std::env::var("GOSLING_TEST_LOGIN_MODE").unwrap() != "failure";
        let barrier = std::sync::Barrier::new(8);
        std::thread::scope(|scope| {
            for _ in 0..8 {
                scope.spawn(|| {
                    barrier.wait();
                    assert_eq!(user_login_path(), success.then_some("/usr/bin:/bin"));
                    assert_eq!(merged_path().is_some(), success);
                });
            }
        });
        assert_eq!(user_login_path(), success.then_some("/usr/bin:/bin"));
        assert_eq!(std::fs::read_to_string(marker).unwrap(), "probe\n");
        return;
    }

    let fixture = tempfile::tempdir().unwrap();
    let shell = fixture.path().join("login-shell");
    std::fs::write(&shell, "#!/bin/sh\nprintf 'probe\\n' >> \"$GOSLING_TEST_LOGIN_PROBES\"\n[ \"$4\" = \"$GOSLING_TEST_EXPECTED_PROBE\" ] || exit 2\nprintf '/usr/bin:/bin\\n'\n[ \"$GOSLING_TEST_LOGIN_MODE\" != failure ]\n").unwrap();
    std::fs::set_permissions(&shell, std::fs::Permissions::from_mode(0o755)).unwrap();
    for mode in ["success", "failure", "fish", "nu"] {
        let mut command = Command::new(std::env::current_exe().unwrap());
        command
            .args([
                "--exact",
                "concurrent_consumers_cache_success_and_failure",
                "--nocapture",
            ])
            .env(
                "GOSLING_TEST_LOGIN_PROBES",
                fixture.path().join(format!("probes-{mode}")),
            )
            .env("GOSLING_TEST_LOGIN_MODE", mode);
        if mode == "fish" || mode == "nu" {
            let login = fixture.path().join(mode);
            std::fs::copy(&shell, &login).unwrap();
            command.env_remove("GOSLING_SHELL").env("SHELL", login).env(
                "GOSLING_TEST_EXPECTED_PROBE",
                if mode == "fish" {
                    "printf '%s\\n' \"$PATH\""
                } else {
                    "print ($env.PATH | str join (char esep))"
                },
            );
        } else {
            command
                .env("GOSLING_SHELL", &shell)
                .env("SHELL", "/bin/sh")
                .env("GOSLING_TEST_EXPECTED_PROBE", "echo $PATH");
        }
        let output = command.output().unwrap();
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
