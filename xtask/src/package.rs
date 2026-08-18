use std::{
    env,
    ffi::{OsStr, OsString},
    path::{Path, PathBuf},
    process::Command,
};

use anyhow::{Context, Result, bail};

const USAGE: &str = "usage: cargo xtask package [--target <triple>]";

#[derive(Debug, PartialEq, Eq)]
struct Options {
    target: Option<String>,
}

enum Action {
    Build(Options),
    Help,
}

pub(super) fn run(args: impl IntoIterator<Item = String>) -> Result<()> {
    let Action::Build(options) = parse_args(args)? else {
        println!("{USAGE}");
        return Ok(());
    };
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .context("xtask must live directly under the workspace root")?;
    let pnpm = if cfg!(windows) { "pnpm.cmd" } else { "pnpm" };

    run_command(root, pnpm, ["--dir", "web", "build:embedded"])?;
    run_command(root, pnpm, ["--dir", "web", "check:embedded"])?;

    let cargo = env::var_os("CARGO").unwrap_or_else(|| OsString::from("cargo"));
    let mut cargo_args = vec![
        OsString::from("build"),
        OsString::from("--locked"),
        OsString::from("--release"),
        OsString::from("--package"),
        OsString::from("any2api"),
    ];
    if let Some(target) = options.target.as_deref() {
        cargo_args.extend([OsString::from("--target"), OsString::from(target)]);
    }
    run_command(root, cargo, cargo_args)?;

    println!("packaged {}", executable_path(root, &options).display());
    Ok(())
}

fn parse_args(args: impl IntoIterator<Item = String>) -> Result<Action> {
    let mut target = None;
    let mut args = args.into_iter();
    while let Some(argument) = args.next() {
        match argument.as_str() {
            "--target" => {
                let value = args.next().context("--target requires a target triple")?;
                set_target(&mut target, value)?;
            }
            "-h" | "--help" => return Ok(Action::Help),
            _ if argument.starts_with("--target=") => {
                set_target(&mut target, argument["--target=".len()..].to_owned())?;
            }
            _ => bail!("unknown package argument {argument:?}; {USAGE}"),
        }
    }
    Ok(Action::Build(Options { target }))
}

fn set_target(target: &mut Option<String>, value: String) -> Result<()> {
    if value.is_empty() || value.starts_with('-') {
        bail!("--target requires a non-empty target triple");
    }
    if target.replace(value).is_some() {
        bail!("--target may only be specified once");
    }
    Ok(())
}

fn run_command(
    root: &Path,
    program: impl AsRef<OsStr>,
    args: impl IntoIterator<Item = impl AsRef<OsStr>>,
) -> Result<()> {
    let mut command = Command::new(program.as_ref());
    command.current_dir(root);
    command.args(args);
    println!("==> {command:?}");
    let status = command
        .status()
        .with_context(|| format!("failed to start {:?}", program.as_ref()))?;
    if !status.success() {
        bail!("command {:?} failed with {status}", program.as_ref());
    }
    Ok(())
}

fn executable_path(root: &Path, options: &Options) -> PathBuf {
    let target_root = env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .map(|path| {
            if path.is_absolute() {
                path
            } else {
                root.join(path)
            }
        })
        .unwrap_or_else(|| root.join("target"));
    let directory = options
        .target
        .as_deref()
        .map_or(target_root.clone(), |target| target_root.join(target));
    let windows = options
        .target
        .as_deref()
        .map_or(cfg!(windows), |target| target.contains("-windows-"));
    directory
        .join("release")
        .join(if windows { "any2api.exe" } else { "any2api" })
}

#[cfg(test)]
mod tests {
    use super::{Action, Options, parse_args};

    fn build(args: &[&str]) -> Options {
        match parse_args(args.iter().map(|value| (*value).to_owned())).expect("valid arguments") {
            Action::Build(options) => options,
            Action::Help => panic!("expected build action"),
        }
    }

    #[test]
    fn parses_default_and_target_modes() {
        assert_eq!(build(&[]), Options { target: None });
        assert_eq!(
            build(&["--target", "x86_64-unknown-linux-gnu"]),
            Options {
                target: Some("x86_64-unknown-linux-gnu".to_owned()),
            }
        );
    }

    #[test]
    fn parses_help_without_building() {
        assert!(matches!(
            parse_args(["--help".to_owned()]).expect("help arguments"),
            Action::Help
        ));
    }

    #[test]
    fn rejects_unknown_or_duplicate_arguments() {
        for args in [
            vec!["--check-assets"],
            vec!["--target", "linux", "--target=windows"],
            vec!["--target", ""],
        ] {
            assert!(
                parse_args(args.into_iter().map(str::to_owned)).is_err(),
                "accepted invalid arguments"
            );
        }
    }
}
