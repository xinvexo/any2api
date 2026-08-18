use std::{env, path::Path};

#[test]
#[ignore = "run explicitly through the Node application lifecycle"]
fn export_admin_bindings() {
    let output = env::var_os("TS_RS_EXPORT_DIR")
        .filter(|value| !value.is_empty())
        .expect("TS_RS_EXPORT_DIR must be set by the Node application lifecycle");
    assert!(
        Path::new(&output).is_absolute(),
        "TS_RS_EXPORT_DIR must be an absolute path"
    );

    let config = ts_rs::Config::from_env().with_large_int("number");
    super::request_usage::export_bindings(&config).expect("export request usage bindings");
    super::gateway_api_key::export_bindings(&config).expect("export gateway API Key bindings");
    super::overview::export_bindings(&config).expect("export overview bindings");
}
