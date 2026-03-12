use std::process::Command;

fn main() {
    // Only build the frontend in release mode.
    // In debug mode the Vite dev server handles it.
    if std::env::var("PROFILE").unwrap_or_default() != "release" {
        return;
    }

    println!("cargo:rerun-if-changed=frontend/src");
    println!("cargo:rerun-if-changed=frontend/index.html");
    println!("cargo:rerun-if-changed=package.json");
    println!("cargo:rerun-if-changed=package-lock.json");

    let ci_status = Command::new("npm")
        .args(["ci"])
        .status()
        .expect("failed to run `npm ci` — is npm installed?");

    if !ci_status.success() {
        panic!("`npm ci` failed — check package-lock.json");
    }

    let build_status = Command::new("npm")
        .args(["run", "build"])
        .status()
        .expect("failed to run `npm run build`");

    if !build_status.success() {
        panic!("frontend build failed");
    }
}
