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
    // Using std::env::var for runtime evaluation (more reliable in build scripts)
    println!("cargo:rerun-if-env-changed=GIT_REV");
    let git_rev = std::env::var("GIT_REV").ok().unwrap_or_else(|| {
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

    // Git release name: use env var if set, otherwise use Cargo.toml version
    // This ensures verification builds match CI builds (both use the same version)
    println!("cargo:rerun-if-env-changed=GIT_RELEASE");
    let git_release = std::env::var("GIT_RELEASE").ok().unwrap_or_else(|| {
        // Use CARGO_PKG_VERSION from Cargo.toml as the source of truth
        // This is deterministic and matches the version bumped before building
        let version = std::env::var("CARGO_PKG_VERSION").unwrap_or_else(|_| "0.0.0".to_string());
        format!("v{}", version)
    });
    println!("cargo:rustc-env=GIT_RELEASE={}", git_release);
    println!("cargo:warning=GIT_RELEASE={}", git_release);

    // Rebuild if git HEAD changes
    println!("cargo:rerun-if-changed=.git/HEAD");
}
