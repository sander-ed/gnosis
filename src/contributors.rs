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
        "invalid GitHub login {value:?}; use an individual username"
    );
    Ok(value.to_ascii_lowercase())
}

fn principal(value: &str) -> Result<String> {
    let value = value.strip_prefix('@').unwrap_or(value);
    if let Some((org, team)) = value.split_once('/') {
        login(org)?;
        ensure!(
            !org.ends_with("[bot]")
                && !team.is_empty()
                && team
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || b"-_".contains(&byte)),
            "invalid GitHub team {value:?}; use @org/team-slug"
        );
        Ok(value.to_ascii_lowercase())
    } else {
        login(value)
    }
}

fn github(arguments: &[&str]) -> Result<String> {
    let output = Command::new("gh")
        .args(["api", "--hostname", "github.com"])
        .args(arguments)
        .output()
        .context("contributor policy requires GitHub CLI; install gh and run gh auth login --hostname github.com")?;
    ensure!(
        output.status.success(),
        "GitHub request failed for {}; check gh authentication and organization Members read permission for team checks",
        arguments[0]
    );
    Ok(String::from_utf8(output.stdout)?)
}

fn team_member(org: &str, team: &str, actor: &str) -> Result<bool> {
    // A successful complete listing distinguishes non-membership from an inaccessible team.
    let members = github(&[
        &format!("orgs/{org}/teams/{team}/members?per_page=100"),
        "--paginate",
        "--jq",
        ".[].login",
    ])
    .with_context(|| format!("cannot verify membership of @{org}/{team}"))?;
    for member in members.lines() {
        if login(member)? == actor {
            return Ok(true);
        }
    }
    Ok(false)
}

impl Contributors {
    pub fn validate(&self) -> Result<()> {
        for entry in self.allow.iter().flatten().chain(&self.deny) {
            principal(entry)?;
        }
        Ok(())
    }

    pub fn check(&self, actor: &str) -> Result<()> {
        self.check_with(actor, team_member)
    }

    fn check_with(
        &self,
        actor: &str,
        mut member: impl FnMut(&str, &str, &str) -> Result<bool>,
    ) -> Result<()> {
        self.validate()?;
        let actor = login(actor)?;
        let mut matches = |entry: &str| -> Result<bool> {
            let entry = principal(entry)?;
            match entry.split_once('/') {
                Some((org, team)) => member(org, team, &actor),
                None => Ok(entry == actor),
            }
        };
        for entry in &self.deny {
            ensure!(
                !matches(entry)?,
                "contributor @{actor} is blocked by {entry}"
            );
        }
        if let Some(allow) = &self.allow {
            for entry in allow {
                if matches(entry)? {
                    return Ok(());
                }
            }
            anyhow::bail!("contributor @{actor} is not in the allowlist");
        }
        Ok(())
    }
}

pub fn check_authenticated(policy: Option<&Contributors>) -> Result<()> {
    let Some(policy) = policy else {
        return Ok(());
    };
    if policy.allow.is_none() && policy.deny.is_empty() {
        return Ok(());
    }
    let actor = github(&["user", "--jq", ".login"])?;
    policy.check(actor.trim())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn policy(text: &str) -> Contributors {
        toml::from_str(text).unwrap()
    }

    #[test]
    fn defaults_empty_allow_and_deny_precedence() {
        assert!(policy("").check("alice").is_ok());
        assert!(policy("allow = []").check("alice").is_err());
        let rules = policy("allow = ['@Alice']\ndeny = ['alice']");
        assert!(
            rules
                .check("ALICE")
                .unwrap_err()
                .to_string()
                .contains("blocked")
        );
        assert!(policy("allow = ['@ALICE']").check("alice").is_ok());
    }

    #[test]
    fn team_rules_match_members_and_deny_overrides_user_allow() {
        let rules = policy("allow = ['@ORG/Engineering']");
        assert!(
            rules
                .check_with("@Alice", |org, team, actor| {
                    assert_eq!((org, team, actor), ("org", "engineering", "alice"));
                    Ok(true)
                })
                .is_ok()
        );
        assert!(rules.check_with("bob", |_, _, _| Ok(false)).is_err());
        let rules = policy("allow = ['alice']\ndeny = ['@org/blocked']");
        assert!(
            rules
                .check_with("alice", |_, _, _| Ok(true))
                .unwrap_err()
                .to_string()
                .contains("blocked")
        );
        assert!(rules.check_with("alice", |_, _, _| Ok(false)).is_ok());
    }

    #[test]
    fn unreadable_teams_cannot_bypass_a_deny() {
        let rules = policy("allow = ['alice']\ndeny = ['@org/private']");
        assert!(
            rules
                .check_with("alice", |_, _, _| anyhow::bail!("not visible"))
                .is_err()
        );
    }

    #[test]
    fn identity_syntax_is_explicit() {
        for entry in [
            "",
            "@",
            "org/",
            "/team",
            "org/team/extra",
            "alice@example.com",
            "*",
            "org/team?x=1",
        ] {
            assert!(principal(entry).is_err());
        }
        assert_eq!(login("@dependabot[bot]").unwrap(), "dependabot[bot]");
        assert!(policy("allow = ['@org/data-team']").validate().is_ok());
        assert!(login("org/team").is_err());
    }
}
