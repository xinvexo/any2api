use std::{
    collections::BTreeMap,
    env, fs,
    path::{Path, PathBuf},
};

fn main() {
    let manifest = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("manifest directory"));
    let root = manifest.join("web-assets");
    println!("cargo:rerun-if-changed={}", root.display());
    let root_type = fs::symlink_metadata(&root)
        .expect("embedded web asset directory")
        .file_type();
    assert!(
        root_type.is_dir() && !root_type.is_symlink(),
        "embedded web asset root must be a regular directory"
    );

    let mut assets = BTreeMap::new();
    collect(&root, &root, &mut assets);
    assert!(
        assets.contains_key("index.html"),
        "embedded web assets must contain index.html; run `pnpm build:embedded` in web"
    );

    let generated = render(&assets);
    let output = PathBuf::from(env::var_os("OUT_DIR").expect("build output directory"))
        .join("embedded_web_assets.rs");
    fs::write(output, generated).expect("write embedded web asset manifest");
}

fn collect(root: &Path, directory: &Path, assets: &mut BTreeMap<String, PathBuf>) {
    let entries = fs::read_dir(directory).unwrap_or_else(|error| {
        panic!(
            "failed to read embedded web assets {}: {error}; run `pnpm build:embedded` in web",
            directory.display()
        )
    });
    for entry in entries {
        let entry = entry.expect("read embedded web asset entry");
        let path = entry.path();
        let file_type = entry.file_type().expect("embedded asset file type");
        assert!(
            !file_type.is_symlink(),
            "embedded web assets cannot contain symbolic links: {}",
            path.display()
        );
        if file_type.is_dir() {
            collect(root, &path, assets);
            continue;
        }
        assert!(
            file_type.is_file(),
            "embedded web assets must be regular files: {}",
            path.display()
        );
        let relative = path
            .strip_prefix(root)
            .expect("embedded asset stays under its root")
            .to_string_lossy()
            .replace('\\', "/");
        assert!(
            assets.insert(relative.clone(), path.clone()).is_none(),
            "duplicate embedded web asset path {relative}"
        );
        println!("cargo:rerun-if-changed={}", path.display());
    }
}

fn render(assets: &BTreeMap<String, PathBuf>) -> String {
    let mut generated =
        String::from("pub(super) static EMBEDDED_WEB_ASSETS: &[EmbeddedWebAsset] = &[\n");
    for (path, source) in assets {
        generated.push_str("    EmbeddedWebAsset::new(");
        generated.push_str(&format!(
            "{path:?}, include_bytes!({:?})",
            source.to_string_lossy()
        ));
        generated.push_str("),\n");
    }
    generated.push_str("];\n");
    generated
}
