use std::collections::BTreeSet;
use std::process::Command;

use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Contributors {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allow: Option<BTreeSet<String>>,
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub deny: BTreeSet<String>,
}

fn login(value: &str) -> Result<String> {
    let value = value.strip_prefix('@').unwrap_or(value);
    let user = value.strip_suffix("[bot]").unwrap_or(value);
    ensure!(
        !user.is_empty()
            && user.len() <= 39
            && !user.starts_with('-')
            && !user.ends_with('-')
            && user
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-'),
        "invalid GitHub login {value:?}; use individual usernames, not emails or teams"
    );
    Ok(value.to_ascii_lowercase())
}

impl Contributors {
    pub fn validate(&self) -> Result<()> {
        for user in self.allow.iter().flatten().chain(&self.deny) {
            login(user)?;
        }
        Ok(())
    }

    fn restricted(&self) -> bool {
        self.allow.is_some() || !self.deny.is_empty()
    }

    fn check(&self, actor: &str) -> Result<()> {
        self.validate()?;
        let actor = login(actor)?;
        ensure!(
            !self
                .deny
                .iter()
                .any(|user| login(user).is_ok_and(|user| user == actor)),
            "contributor @{actor} is blocked"
        );
        if let Some(allow) = &self.allow {
            ensure!(
                allow
                    .iter()
                    .any(|user| login(user).is_ok_and(|user| user == actor)),
                "contributor @{actor} is not in the allowlist"
            );
        }
        Ok(())
    }
}

pub fn check(actor: &str, policies: &[Option<&Contributors>]) -> Result<()> {
    login(actor)?;
    for policy in policies.iter().flatten() {
        policy.check(actor)?;
    }
    Ok(())
}

pub fn check_authenticated(policies: &[Option<&Contributors>]) -> Result<()> {
    if !policies.iter().flatten().any(|policy| policy.restricted()) {
        return Ok(());
    }
    let output = Command::new("gh")
        .args(["api", "--hostname", "github.com", "user", "--jq", ".login"])
        .output()
        .context("contributor policy requires GitHub CLI; install gh and run gh auth login --hostname github.com")?;
    ensure!(
        output.status.success(),
        "could not authenticate contributor; run gh auth login --hostname github.com"
    );
    let actor = std::str::from_utf8(&output.stdout)?.trim();
    check(actor, policies)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn policy(text: &str) -> Contributors {
        toml::from_str(text).unwrap()
    }

    #[test]
    fn defaults_empty_allow_and_deny_precedence() {
        assert!(check("alice", &[Some(&policy(""))]).is_ok());
        assert!(check("alice", &[Some(&policy("allow = []"))]).is_err());
        let rules = policy("allow = ['@Alice']\ndeny = ['alice']");
        assert!(
            check("ALICE", &[Some(&rules)])
                .unwrap_err()
                .to_string()
                .contains("blocked")
        );
        assert!(check("alice", &[Some(&policy("allow = ['@ALICE']"))]).is_ok());
    }

    #[test]
    fn package_rules_cannot_relax_catalog_rules() {
        let catalog = policy("allow = ['alice']");
        let package = policy("allow = ['bob']");
        assert!(check("bob", &[Some(&catalog), Some(&package)]).is_err());
        assert!(check("alice", &[Some(&catalog), Some(&package)]).is_err());
        assert!(check("alice", &[Some(&catalog), None]).is_ok());
    }

    #[test]
    fn identity_syntax_is_explicit() {
        for user in ["", "@", "org/team", "alice@example.com", "*"] {
            assert!(login(user).is_err());
        }
        assert_eq!(login("@dependabot[bot]").unwrap(), "dependabot[bot]");
        assert!(policy("allow = ['org/team']").validate().is_err());
    }
}
