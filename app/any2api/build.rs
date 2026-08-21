use std::{
    collections::BTreeMap,
    env, fs,
    path::{Path, PathBuf},
};

use sha2::{Digest, Sha256};

const WEB_ASSET_DIR_ENV: &str = "ANY2API_BUILD_WEB_DIR";

fn main() {
    println!("cargo:rerun-if-env-changed={WEB_ASSET_DIR_ENV}");
    let crate_root = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("manifest directory"));
    let assets = match env::var_os(WEB_ASSET_DIR_ENV) {
        Some(value) => load_build_assets(&PathBuf::from(value)),
        None => {
            println!(
                "cargo:warning=building any2api without the complete Web UI; run `pnpm build` or `pnpm package` for an application build"
            );
            load_rust_only_assets(&crate_root.join("rust-only-web"))
        }
    };

    let generated = render(&assets);
    let output = PathBuf::from(env::var_os("OUT_DIR").expect("build output directory"))
        .join("embedded_web_assets.rs");
    fs::write(output, generated).expect("write embedded Web asset table");
}

fn load_build_assets(root: &Path) -> BTreeMap<String, PathBuf> {
    assert!(
        !root.as_os_str().is_empty(),
        "{WEB_ASSET_DIR_ENV} cannot be empty"
    );
    assert!(
        root.is_absolute(),
        "{WEB_ASSET_DIR_ENV} must be an absolute path"
    );
    let assets = collect_assets(root);
    require_non_empty_index(&assets, "Web asset");
    assets
}

fn load_rust_only_assets(root: &Path) -> BTreeMap<String, PathBuf> {
    let assets = collect_assets(root);
    require_non_empty_index(&assets, "Rust-only Web asset");
    assets
}

fn require_non_empty_index(assets: &BTreeMap<String, PathBuf>, label: &str) {
    let index = assets
        .get("index.html")
        .unwrap_or_else(|| panic!("{label} assets must contain index.html"));
    assert!(
        fs::metadata(index).expect("index metadata").len() > 0,
        "{label} index.html cannot be empty"
    );
}

fn collect_assets(root: &Path) -> BTreeMap<String, PathBuf> {
    require_regular_directory(root, "Web asset root");
    println!("cargo:rerun-if-changed={}", root.display());
    let mut assets = BTreeMap::new();
    collect_directory(root, root, &mut assets);
    assets
}

fn collect_directory(root: &Path, directory: &Path, assets: &mut BTreeMap<String, PathBuf>) {
    let entries = fs::read_dir(directory).unwrap_or_else(|error| {
        panic!("failed to read Web assets {}: {error}", directory.display())
    });
    for entry in entries {
        let entry = entry.expect("read Web asset entry");
        let path = entry.path();
        let file_type = entry.file_type().expect("Web asset file type");
        assert!(
            !file_type.is_symlink(),
            "Web assets cannot contain symbolic links: {}",
            path.display()
        );
        if file_type.is_dir() {
            collect_directory(root, &path, assets);
        } else {
            assert!(file_type.is_file(), "Web assets must be regular files");
            let relative = path
                .strip_prefix(root)
                .expect("Web asset stays under its root")
                .to_string_lossy()
                .replace('\\', "/");
            assert!(
                assets.insert(relative.clone(), path.clone()).is_none(),
                "duplicate Web asset path {relative}"
            );
            println!("cargo:rerun-if-changed={}", path.display());
        }
    }
}

fn require_regular_directory(path: &Path, label: &str) {
    let metadata = fs::symlink_metadata(path)
        .unwrap_or_else(|error| panic!("failed to inspect {label} {}: {error}", path.display()));
    assert!(
        metadata.file_type().is_dir() && !metadata.file_type().is_symlink(),
        "{label} must be a regular directory: {}",
        path.display()
    );
}

fn render(assets: &BTreeMap<String, PathBuf>) -> String {
    let mut generated =
        String::from("pub(super) static EMBEDDED_WEB_ASSETS: &[EmbeddedWebAsset] = &[\n");
    for (path, source) in assets {
        let bytes = fs::read(source).expect("read embedded Web asset");
        let etag = format!(
            "\"sha256-{}\"",
            Sha256::digest(&bytes)
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>()
        );
        generated.push_str("    EmbeddedWebAsset::new(");
        generated.push_str(&format!(
            "{path:?}, include_bytes!({:?}), {etag:?}",
            source.to_string_lossy(),
        ));
        generated.push_str("),\n");
    }
    generated.push_str("];\n");
    generated
}
