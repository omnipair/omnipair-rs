use std::process::Command;

fn main() {
    // We require overflow checks for the program to function correctly
    // Note: In debug builds, overflow checks are enabled by default.
    // For release builds, ensure `overflow-checks = true` in Cargo.toml [profile.release]
    match std::panic::catch_unwind(|| {
        #[allow(arithmetic_overflow)]
        let _ = 255_u8 + 1;
    }) {
        Ok(_) => {
            panic!("overflow checks are required for the program to function correctly");
        }
        Err(_) => {
            // Overflow checks are enabled - good!
        }
    }

    // Git revision: use env var if set, otherwise run git command
    println!("cargo:rerun-if-env-changed=GIT_REV");
    let git_rev = std::option_env!("GIT_REV")
        .map(String::from)
        .unwrap_or_else(|| {
            Command::new("git")
                .args(["rev-parse", "HEAD"])
                .output()
                .ok()
                .and_then(|o| String::from_utf8(o.stdout).ok())
                .map(|s| s.trim().to_string())
                .unwrap_or_else(|| "GIT_REV_MISSING".to_string())
        });
    println!("cargo:rustc-env=GIT_REV={}", git_rev);
    println!("cargo:warning=GIT_REV={}", git_rev);

    // Git release name: use env var if set, otherwise try to get exact tag, fallback to describe
    println!("cargo:rerun-if-env-changed=GIT_RELEASE");
    let git_release = std::option_env!("GIT_RELEASE")
        .map(String::from)
        .unwrap_or_else(|| {
            // First try to get exact tag (for release builds)
            Command::new("git")
                .args(["describe", "--tags", "--exact-match"])
                .output()
                .ok()
                .filter(|o| o.status.success())
                .and_then(|o| String::from_utf8(o.stdout).ok())
                .map(|s| s.trim().to_string())
                // Fallback to describe with distance from tag (for dev builds)
                .or_else(|| {
                    Command::new("git")
                        .args(["describe", "--tags", "--always"])
                        .output()
                        .ok()
                        .and_then(|o| String::from_utf8(o.stdout).ok())
                        .map(|s| s.trim().to_string())
                })
                .unwrap_or_else(|| "GIT_RELEASE_MISSING".to_string())
        });
    println!("cargo:rustc-env=GIT_RELEASE={}", git_release);
    println!("cargo:warning=GIT_RELEASE={}", git_release);

    // Rebuild if git HEAD changes
    println!("cargo:rerun-if-changed=.git/HEAD");
}
