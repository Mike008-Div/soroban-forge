//! Resolve the `--source` identity for `stellar` child processes.
//!
//! soroban-forge identities live in `~/.config/soroban-forge/identities.json`,
//! which stellar-cli cannot read. For those we hand stellar-cli the key through
//! the `STELLAR_ACCOUNT` env var of the child process, so the secret never
//! appears in argv or logs. Everything else is passed through unchanged.

use std::fmt;
use std::process::Command;

pub const STELLAR_ACCOUNT_ENV: &str = "STELLAR_ACCOUNT";

#[derive(Clone, PartialEq, Eq)]
pub enum ResolvedSource {
    /// Pass `--source <value>` (stellar-cli name/alias or raw `G…`/`S…` value).
    Arg(String),
    /// soroban-forge-only identity: pass the secret via env, no `--source`.
    Env { name: String, secret: String },
}

impl fmt::Debug for ResolvedSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ResolvedSource::Arg(v) => f.debug_tuple("Arg").field(v).finish(),
            ResolvedSource::Env { name, .. } => f
                .debug_struct("Env")
                .field("name", name)
                .field("secret", &"<redacted>")
                .finish(),
        }
    }
}

/// True for a raw Stellar public (`G…`) or secret (`S…`) strkey.
pub fn is_raw_key(value: &str) -> bool {
    value.len() == 56
        && (value.starts_with('G') || value.starts_with('S'))
        && value
            .chars()
            .all(|c| c.is_ascii_uppercase() || ('2'..='7').contains(&c))
}

/// Decide how to hand `source` to stellar-cli.
///
/// Order: raw key -> soroban-forge identity -> stellar-cli name (fall through).
/// `forge_secret` returns the secret for a soroban-forge identity name, or
/// `None` if no such identity exists.
pub fn resolve_source<F>(source: &str, forge_secret: F) -> ResolvedSource
where
    F: Fn(&str) -> Option<String>,
{
    if is_raw_key(source) {
        return ResolvedSource::Arg(source.to_string());
    }
    match forge_secret(source) {
        Some(secret) => ResolvedSource::Env {
            name: source.to_string(),
            secret,
        },
        None => ResolvedSource::Arg(source.to_string()),
    }
}

impl ResolvedSource {
    /// argv fragment to append to the `stellar` command (never has a secret
    /// for forge identities).
    pub fn args(&self) -> Vec<String> {
        match self {
            ResolvedSource::Arg(v) => vec!["--source".to_string(), v.clone()],
            ResolvedSource::Env { .. } => Vec::new(),
        }
    }

    /// Env vars to set on the child process.
    pub fn envs(&self) -> Vec<(&'static str, String)> {
        match self {
            ResolvedSource::Arg(_) => Vec::new(),
            ResolvedSource::Env { secret, .. } => {
                vec![(STELLAR_ACCOUNT_ENV, secret.clone())]
            }
        }
    }

    /// Convenience: apply args and env to a `Command`.
    pub fn apply(&self, cmd: &mut Command) {
        cmd.args(self.args());
        match self {
            ResolvedSource::Arg(_) => {
                // Don't let an ambient STELLAR_ACCOUNT override the explicit --source.
                cmd.env_remove(STELLAR_ACCOUNT_ENV);
            }
            ResolvedSource::Env { secret, .. } => {
                cmd.env(STELLAR_ACCOUNT_ENV, secret);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const G_KEY: &str = "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";
    const S_KEY: &str = "SBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB";
    const SECRET: &str = "SCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCC";

    fn forge(name: &str) -> Option<String> {
        (name == "alice").then(|| SECRET.to_string())
    }

    #[test]
    fn raw_keys_are_detected() {
        assert!(is_raw_key(G_KEY));
        assert!(is_raw_key(S_KEY));
        assert!(!is_raw_key("alice"));
        assert!(!is_raw_key("GSHORT"));
    }

    #[test]
    fn forge_identity_uses_env_not_argv() {
        let r = resolve_source("alice", forge);
        assert!(r.args().is_empty());
        assert_eq!(r.envs(), vec![(STELLAR_ACCOUNT_ENV, SECRET.to_string())]);
    }

    #[test]
    fn stellar_cli_identity_falls_through() {
        let r = resolve_source("bob", forge);
        assert_eq!(r.args(), vec!["--source", "bob"]);
        assert!(r.envs().is_empty());
    }

    #[test]
    fn raw_keys_pass_through_even_if_lookup_would_match() {
        for key in [G_KEY, S_KEY] {
            let r = resolve_source(key, |_| Some(SECRET.to_string()));
            assert_eq!(r.args(), vec!["--source".to_string(), key.to_string()]);
            assert!(r.envs().is_empty());
        }
    }

    #[test]
    fn debug_never_prints_secret() {
        let r = resolve_source("alice", forge);
        let shown = format!("{r:?}");
        assert!(!shown.contains(SECRET));
        assert!(shown.contains("redacted"));
    }
}