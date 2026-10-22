#![cfg(unix)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::Command;

use soroban_forge_core::source::{resolve_source, STELLAR_ACCOUNT_ENV};

const G_KEY: &str = "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";
const SECRET: &str = "SCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCC";

fn fake_stellar(tag: &str) -> (PathBuf, PathBuf) {
    let dir = std::env::temp_dir().join(format!("forge-src-{}-{}", std::process::id(), tag));
    fs::create_dir_all(&dir).unwrap();
    let bin = dir.join("stellar");
    fs::write(
        &bin,
        "#!/bin/sh\nprintf '%s\\n' \"$@\" > \"$FAKE_OUT/args\"\nprintf '%s' \"${STELLAR_ACCOUNT:-}\" > \"$FAKE_OUT/env\"\n",
    )
    .unwrap();
    fs::set_permissions(&bin, fs::Permissions::from_mode(0o755)).unwrap();
    (bin, dir)
}

fn run(source: &str, tag: &str) -> (String, String) {
    let (bin, dir) = fake_stellar(tag);
    let resolved = resolve_source(source, |n| (n == "alice").then(|| SECRET.to_string()));
    let mut cmd = Command::new(bin);
    cmd.arg("contract").arg("deploy").env("FAKE_OUT", &dir);
    resolved.apply(&mut cmd);
    assert!(cmd.status().unwrap().success());
    (
        fs::read_to_string(dir.join("args")).unwrap(),
        fs::read_to_string(dir.join("env")).unwrap(),
    )
}

#[test]
fn forge_identity_secret_goes_via_env_only() {
    let (args, env) = run("alice", "forge");
    assert!(!args.contains(SECRET));
    assert!(!args.contains("--source"));
    assert_eq!(env, SECRET);
    let _ = STELLAR_ACCOUNT_ENV;
}

#[test]
fn stellar_cli_identity_is_passed_as_source() {
    let (args, env) = run("bob", "cli");
    assert!(args.contains("--source\nbob"));
    assert_eq!(env, "");
}

#[test]
fn raw_key_is_passed_as_source() {
    let (args, env) = run(G_KEY, "raw");
    assert!(args.contains(&format!("--source\n{G_KEY}")));
    assert_eq!(env, "");
}