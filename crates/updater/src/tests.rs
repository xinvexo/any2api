use std::{fs, io::Write, time::Duration};

use flate2::{Compression, write::GzEncoder};
use serde_json::json;
use tar::{Builder, Header};
use tempfile::tempdir;

use crate::{
    api::UpdateErrorKind,
    github::{MAX_ARCHIVE_BYTES, parse_release_for_test},
    install::{extract_and_replace_for_test, verify_checksum_for_test},
    smoke::verify_staged_version_for_test,
};

mod task_lifecycle;

const TARGET_VERSION: &str = "1.2.3";
const RELEASE_BINARY: &[u8] = b"#!/bin/sh\n\
if [ \"$1\" = \"--version\" ]; then\n\
  printf 'any2api 1.2.3\\n'\n\
else\n\
  exit 2\n\
fi\n";

#[test]
fn compiled_build_version_uses_the_release_override_or_dev_default() {
    let version = semver::Version::parse(crate::BUILD_VERSION).expect("build version is SemVer");
    if let Some(expected) = option_env!("ANY2API_BUILD_VERSION") {
        assert_eq!(crate::BUILD_VERSION, expected);
        assert!(version.pre.is_empty());
        assert!(version.build.is_empty());
    } else {
        assert_eq!(crate::BUILD_VERSION, "0.0.0-dev");
    }
}

#[test]
fn release_metadata_requires_exact_stable_assets() {
    let archive = "any2api-v1.2.3-linux-amd64.tar.gz";
    let document = json!({
        "tag_name": "v1.2.3",
        "draft": false,
        "prerelease": false,
        "published_at": "2026-07-29T00:00:00Z",
        "assets": [
            { "name": archive, "size": 1234 },
            { "name": format!("{archive}.sha256"), "size": 100 }
        ]
    });
    let release = parse_release_for_test(&serde_json::to_vec(&document).expect("metadata"))
        .expect("valid release");
    assert_eq!(release.version.to_string(), "1.2.3");
    assert_eq!(release.archive_name, archive);

    let mut oversized = document.clone();
    oversized["assets"][0]["size"] = json!(MAX_ARCHIVE_BYTES + 1);
    let error = parse_release_for_test(&serde_json::to_vec(&oversized).expect("large metadata"))
        .expect_err("oversized archive must fail");
    assert_eq!(error.kind(), UpdateErrorKind::InvalidRelease);

    let mut missing_checksum = document;
    missing_checksum["assets"] = json!([{ "name": archive, "size": 1234 }]);
    let error =
        parse_release_for_test(&serde_json::to_vec(&missing_checksum).expect("invalid metadata"))
            .expect_err("checksum is required");
    assert_eq!(error.kind(), UpdateErrorKind::InvalidRelease);
}

#[test]
fn checksum_must_name_and_match_the_archive() {
    let name = "any2api-v1.2.3-linux-amd64.tar.gz";
    let digest = "a".repeat(64);
    verify_checksum_for_test(format!("{digest}  {name}\n").as_bytes(), name, &digest)
        .expect("valid checksum");
    let error = verify_checksum_for_test(
        format!("{}  {name}\n", "b".repeat(64)).as_bytes(),
        name,
        &digest,
    )
    .expect_err("mismatch must fail");
    assert_eq!(error.kind(), UpdateErrorKind::VerificationFailed);
}

#[test]
fn verified_archive_atomically_replaces_the_binary() {
    let directory = tempdir().expect("temporary directory");
    let executable = directory.path().join("any2api");
    write_executable(&executable, b"old binary", 0o755);
    let archive = directory.path().join("release.tar.gz");
    write_archive(&archive, &[("any2api", RELEASE_BINARY)]);

    extract_and_replace_for_test(
        &archive,
        &directory.path().join("staged"),
        &executable,
        &target_version(),
    )
    .expect("replace binary");
    assert_eq!(fs::read(&executable).expect("new binary"), RELEASE_BINARY);
    assert_eq!(
        fs::read(directory.path().join("any2api.previous")).expect("previous binary"),
        b"old binary"
    );
    assert!(directory.path().join("any2api.update-pending").is_file());
}

#[cfg(unix)]
#[test]
fn archive_with_wrong_embedded_version_never_changes_the_current_binary() {
    let directory = tempdir().expect("temporary directory");
    let executable = directory.path().join("any2api");
    write_executable(&executable, b"old binary", 0o755);
    let archive = directory.path().join("release.tar.gz");
    write_archive(
        &archive,
        &[("any2api", b"#!/bin/sh\nprintf 'any2api 9.9.9\\n'\n")],
    );

    let error = extract_and_replace_for_test(
        &archive,
        &directory.path().join("staged"),
        &executable,
        &target_version(),
    )
    .expect_err("wrong candidate version must fail");

    assert_eq!(error.kind(), UpdateErrorKind::VerificationFailed);
    assert_eq!(fs::read(&executable).expect("old binary"), b"old binary");
    assert!(!directory.path().join("any2api.previous").exists());
    assert!(!directory.path().join("any2api.update-pending").exists());
}

#[cfg(unix)]
#[test]
fn replacement_rejects_a_symlink_current_executable() {
    use std::os::unix::fs::symlink;

    let directory = tempdir().expect("temporary directory");
    let actual = directory.path().join("actual");
    write_executable(&actual, b"old binary", 0o755);
    let executable = directory.path().join("any2api");
    symlink(&actual, &executable).expect("current executable symlink");
    let archive = directory.path().join("release.tar.gz");
    write_archive(&archive, &[("any2api", RELEASE_BINARY)]);

    let error = extract_and_replace_for_test(
        &archive,
        &directory.path().join("staged"),
        &executable,
        &target_version(),
    )
    .expect_err("symlink current executable must fail");

    assert_eq!(error.kind(), UpdateErrorKind::InstallFailed);
    assert_eq!(
        fs::read_link(&executable).expect("symlink preserved"),
        actual
    );
    assert!(!directory.path().join("any2api.previous").exists());
    assert!(!directory.path().join("any2api.update-pending").exists());
}

#[cfg(unix)]
#[test]
fn replacement_preserves_restrictive_executable_permissions() {
    use std::os::unix::fs::PermissionsExt;

    for mode in [0o700, 0o555] {
        let directory = tempdir().expect("temporary directory");
        let executable = directory.path().join("any2api");
        write_executable(&executable, b"old binary", mode);
        let archive = directory.path().join("release.tar.gz");
        write_archive(&archive, &[("any2api", RELEASE_BINARY)]);

        extract_and_replace_for_test(
            &archive,
            &directory.path().join("staged"),
            &executable,
            &target_version(),
        )
        .expect("replace binary");
        let actual = fs::metadata(executable)
            .expect("new binary")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(actual, mode);
    }
}

#[test]
fn archive_with_extra_entries_is_rejected_before_replacement() {
    let directory = tempdir().expect("temporary directory");
    let executable = directory.path().join("any2api");
    fs::write(&executable, b"old binary").expect("old binary");
    let archive = directory.path().join("release.tar.gz");
    write_archive(&archive, &[("any2api", b"new binary"), ("extra", b"bad")]);

    let error = extract_and_replace_for_test(
        &archive,
        &directory.path().join("staged"),
        &executable,
        &target_version(),
    )
    .expect_err("extra archive member must fail");
    assert_eq!(error.kind(), UpdateErrorKind::VerificationFailed);
    assert_eq!(fs::read(executable).expect("old binary"), b"old binary");
}

#[cfg(unix)]
#[test]
fn staged_version_must_match_and_finish_before_the_deadline() {
    let directory = tempdir().expect("temporary directory");
    let wrong = directory.path().join("wrong");
    write_executable(&wrong, b"#!/bin/sh\nprintf 'any2api 9.9.9\\n'\n", 0o755);
    let error = verify_staged_version_for_test(&wrong, &target_version(), Duration::from_secs(1))
        .expect_err("wrong version must fail");
    assert_eq!(error.kind(), UpdateErrorKind::VerificationFailed);

    let noisy = directory.path().join("noisy");
    let noisy_script = format!("#!/bin/sh\nprintf '{}\\n'\n", "x".repeat(300));
    write_executable(&noisy, noisy_script.as_bytes(), 0o755);
    let error = verify_staged_version_for_test(&noisy, &target_version(), Duration::from_secs(1))
        .expect_err("oversized version output must fail");
    assert_eq!(error.kind(), UpdateErrorKind::VerificationFailed);

    let failing = directory.path().join("failing");
    write_executable(&failing, b"#!/bin/sh\nexit 7\n", 0o755);
    let error = verify_staged_version_for_test(&failing, &target_version(), Duration::from_secs(1))
        .expect_err("non-zero version command must fail");
    assert_eq!(error.kind(), UpdateErrorKind::VerificationFailed);

    let hanging = directory.path().join("hanging");
    write_executable(&hanging, b"#!/bin/sh\nwhile :; do :; done\n", 0o755);
    let started = std::time::Instant::now();
    let error =
        verify_staged_version_for_test(&hanging, &target_version(), Duration::from_millis(50))
            .expect_err("hanging version command must fail");
    assert_eq!(error.kind(), UpdateErrorKind::VerificationFailed);
    assert!(started.elapsed() < Duration::from_secs(2));
}

fn write_archive(path: &std::path::Path, entries: &[(&str, &[u8])]) {
    let file = fs::File::create(path).expect("archive file");
    let encoder = GzEncoder::new(file, Compression::default());
    let mut builder = Builder::new(encoder);
    for (name, contents) in entries {
        let mut header = Header::new_gnu();
        header.set_size(u64::try_from(contents.len()).expect("content size"));
        header.set_mode(0o755);
        header.set_cksum();
        builder
            .append_data(&mut header, name, *contents)
            .expect("archive entry");
    }
    let encoder = builder.into_inner().expect("tar finish");
    encoder
        .finish()
        .expect("gzip finish")
        .flush()
        .expect("flush");
}

fn target_version() -> semver::Version {
    semver::Version::parse(TARGET_VERSION).expect("target version")
}

fn write_executable(path: &std::path::Path, contents: &[u8], mode: u32) {
    fs::write(path, contents).expect("write executable");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        fs::set_permissions(path, fs::Permissions::from_mode(mode)).expect("set executable mode");
    }
    #[cfg(not(unix))]
    let _ = mode;
}
