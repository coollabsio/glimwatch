use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("manifest dir"));
    placeholder_ui(&manifest);
    migration_catalog(&manifest);
}

fn placeholder_ui(manifest: &Path) {
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

/// Writes `migrations.rs` to `OUT_DIR` from the `<version>_<name>.sql` files in
/// `migrations/`. Versions are UTC timestamps (`YYYYMMDDHHMMSS`), so parallel
/// branches do not collide. Make a new one with `just new-migration <name>`.
fn migration_catalog(manifest: &Path) {
    let root = manifest.join("migrations");
    println!("cargo:rerun-if-changed={}", root.display());

    let mut catalog = BTreeMap::new();
    for entry in fs::read_dir(&root).expect("read migrations directory") {
        let path = entry.expect("read migrations entry").path();
        let file = path.file_name().unwrap().to_string_lossy().into_owned();
        let Some(stem) = file.strip_suffix(".sql") else {
            continue;
        };
        let (version, name) = stem
            .split_once('_')
            .unwrap_or_else(|| panic!("migration {file}: expected `<version>_<name>.sql`"));
        assert!(
            !name.is_empty()
                && name
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_'),
            "migration {file}: use lowercase letters, digits and `_` in the name"
        );
        let number: i64 = version
            .parse()
            .unwrap_or_else(|_| panic!("migration {file}: the version must be a number"));
        assert!(
            version.len() == 14 && version.starts_with("20"),
            "migration {file}: a migration needs a UTC timestamp version (YYYYMMDDHHMMSS). \
             Make it with `just new-migration <name>`"
        );
        if let Some((_, other)) = catalog.insert(number, (name.to_owned(), path.clone())) {
            panic!(
                "migration version {number} appears more than once: {} and {file}",
                other.display()
            );
        }
    }

    let mut out = String::from("pub(crate) const MIGRATIONS: &[(i64, &str, &str)] = &[\n");
    for (version, (name, path)) in &catalog {
        writeln!(
            out,
            "    ({version}, {name:?}, include_str!({:?})),",
            path.display().to_string()
        )
        .unwrap();
    }
    out.push_str("];\n");
    let out_path = PathBuf::from(std::env::var("OUT_DIR").expect("out dir")).join("migrations.rs");
    fs::write(out_path, out).expect("write migration catalog");
}
