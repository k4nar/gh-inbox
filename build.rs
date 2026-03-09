use std::process::Command;

fn main() {
    // Only build the frontend in release mode.
    // In debug mode the Vite dev server handles it.
    if std::env::var("PROFILE").unwrap_or_default() != "release" {
        return;
    }

    println!("cargo:rerun-if-changed=frontend/src");
    println!("cargo:rerun-if-changed=frontend/index.html");
    println!("cargo:rerun-if-changed=frontend/package.json");

    let ci_status = Command::new("npm")
        .args(["ci"])
        .current_dir("frontend")
        .status()
        .expect("failed to run `npm ci` — is npm installed?");

    if !ci_status.success() {
        panic!("`npm ci` failed — check frontend/package-lock.json");
    }

    let build_status = Command::new("npm")
        .args(["run", "build"])
        .current_dir("frontend")
        .status()
        .expect("failed to run `npm run build`");

    if !build_status.success() {
        panic!("frontend build failed");
    }
}
