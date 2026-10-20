use std::fs;
use std::path::PathBuf;

fn main() {
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("manifest dir"));
    let dist = manifest.join("../web/dist");
    let index = dist.join("index.html");
    if !index.exists() {
        fs::create_dir_all(&dist).expect("create web dist");
        fs::write(
            &index,
            "<!doctype html><title>Status</title><p>The UI has not been built yet. From apps/web, run <code>bun install && bun run build</code>, then rebuild the server.</p>",
        )
        .expect("write placeholder index");
    }
    println!("cargo:rerun-if-changed={}", dist.display());
}
