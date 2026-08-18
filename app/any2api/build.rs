use std::{
    collections::BTreeMap,
    env, fs,
    path::{Component, Path, PathBuf},
};

use serde::Deserialize;
use sha2::{Digest, Sha256};

const ASSET_MANIFEST_ENV: &str = "ANY2API_BUILD_WEB_ASSET_MANIFEST";
const MANIFEST_SCHEMA: u32 = 1;

#[derive(Deserialize)]
struct AssetManifest {
    schema: u32,
    asset_root: String,
    bundle_sha256: String,
    files: Vec<ManifestFile>,
}

#[derive(Deserialize)]
struct ManifestFile {
    path: String,
    size: u64,
    sha256: String,
}

fn main() {
    println!("cargo:rerun-if-env-changed={ASSET_MANIFEST_ENV}");
    let crate_root = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("manifest directory"));
    let assets = match env::var_os(ASSET_MANIFEST_ENV) {
        Some(value) => load_manifest_assets(&PathBuf::from(value)),
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
    fs::write(output, generated).expect("write embedded Web asset manifest");
}

fn load_manifest_assets(manifest_path: &Path) -> BTreeMap<String, PathBuf> {
    assert!(
        !manifest_path.as_os_str().is_empty(),
        "{ASSET_MANIFEST_ENV} cannot be empty"
    );
    assert!(
        manifest_path.is_absolute(),
        "{ASSET_MANIFEST_ENV} must be an absolute path"
    );
    require_regular_file(manifest_path, "Web asset manifest");
    println!("cargo:rerun-if-changed={}", manifest_path.display());
    let bytes = fs::read(manifest_path).unwrap_or_else(|error| {
        panic!(
            "failed to read Web asset manifest {}: {error}",
            manifest_path.display()
        )
    });
    let manifest: AssetManifest = serde_json::from_slice(&bytes).unwrap_or_else(|error| {
        panic!(
            "failed to parse Web asset manifest {}: {error}",
            manifest_path.display()
        )
    });
    assert_eq!(
        manifest.schema, MANIFEST_SCHEMA,
        "unsupported Web asset schema"
    );
    assert_eq!(manifest.asset_root, "root", "unsupported Web asset root");

    let publication = manifest_path
        .parent()
        .expect("Web asset manifest must have a parent directory");
    assert_eq!(
        publication.file_name().and_then(|name| name.to_str()),
        Some(manifest.bundle_sha256.as_str()),
        "Web asset publication directory must equal bundle_sha256"
    );
    let root = publication.join(&manifest.asset_root);
    let assets = collect_assets(&root);
    validate_manifest_files(&assets, &manifest);
    assets
}

fn load_rust_only_assets(root: &Path) -> BTreeMap<String, PathBuf> {
    let assets = collect_assets(root);
    let index = assets
        .get("index.html")
        .expect("Rust-only Web assets must contain index.html");
    assert!(
        fs::metadata(index).expect("Rust-only index metadata").len() > 0,
        "Rust-only index.html cannot be empty"
    );
    assets
}

fn validate_manifest_files(assets: &BTreeMap<String, PathBuf>, manifest: &AssetManifest) {
    assert_eq!(
        assets.len(),
        manifest.files.len(),
        "Web asset manifest file set is stale"
    );
    let mut bundle = Sha256::new();
    let mut previous: Option<&str> = None;
    for expected in &manifest.files {
        validate_relative_path(&expected.path);
        if let Some(previous) = previous {
            assert!(
                previous < expected.path.as_str(),
                "manifest files must be sorted"
            );
        }
        previous = Some(&expected.path);
        let source = assets.get(&expected.path).unwrap_or_else(|| {
            panic!(
                "Web asset manifest references missing file {}",
                expected.path
            )
        });
        let bytes = fs::read(source).unwrap_or_else(|error| {
            panic!("failed to read Web asset {}: {error}", source.display())
        });
        assert_eq!(bytes.len() as u64, expected.size, "Web asset size mismatch");
        assert_eq!(
            hex_sha256(&bytes),
            expected.sha256,
            "Web asset hash mismatch"
        );
        bundle.update(expected.path.as_bytes());
        bundle.update(b"\0");
        bundle.update(expected.size.to_string().as_bytes());
        bundle.update(b"\0");
        bundle.update(expected.sha256.as_bytes());
        bundle.update(b"\n");
    }
    assert_eq!(
        format!("{:x}", bundle.finalize()),
        manifest.bundle_sha256,
        "Web asset bundle digest mismatch"
    );
    let index = assets
        .get("index.html")
        .expect("Web asset manifest must contain index.html");
    assert!(fs::metadata(index).expect("index metadata").len() > 0);
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

fn validate_relative_path(raw: &str) {
    assert!(!raw.is_empty() && !raw.contains('\\'));
    let path = Path::new(raw);
    assert!(
        path.components()
            .all(|component| matches!(component, Component::Normal(_))),
        "invalid Web asset path {raw:?}"
    );
}

fn require_regular_file(path: &Path, label: &str) {
    let metadata = fs::symlink_metadata(path)
        .unwrap_or_else(|error| panic!("failed to inspect {label} {}: {error}", path.display()));
    assert!(
        metadata.file_type().is_file() && !metadata.file_type().is_symlink(),
        "{label} must be a regular file: {}",
        path.display()
    );
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

fn hex_sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn render(assets: &BTreeMap<String, PathBuf>) -> String {
    let mut generated =
        String::from("pub(super) static EMBEDDED_WEB_ASSETS: &[EmbeddedWebAsset] = &[\n");
    for (path, source) in assets {
        let bytes = fs::read(source).expect("read embedded Web asset");
        let etag = format!("\"sha256-{}\"", hex_sha256(&bytes));
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
