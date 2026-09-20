use std::collections::BTreeSet;

use anyhow::{Context, Result};

use super::Workspace;
use crate::metadata::{Manifest, Package};
use crate::{contributors, git, metadata};

impl Workspace {
    pub fn check_contribution(&self, actor: &str, base: &str, head: &str) -> Result<()> {
        let commit = |revision: &str| -> Result<String> {
            let bytes = git::run(
                &self.root,
                &[
                    "rev-parse",
                    "--verify",
                    "--end-of-options",
                    &format!("{revision}^{{commit}}"),
                ],
                true,
            )?;
            Ok(String::from_utf8(bytes)?.trim().to_owned())
        };
        let base = commit(base)?;
        let head = commit(head)?;
        let manifest: Manifest = metadata::decode(
            &git::file_at(&self.root, &base, "gnosis.toml")?
                .context("trusted base is missing gnosis.toml; establish catalog policy on the base branch first")?,
        )?;
        metadata::validate_manifest(&manifest)?;
        let paths = git::run(
            &self.root,
            &[
                "diff",
                "--name-only",
                "--no-renames",
                "-z",
                &format!("{base}...{head}"),
                "--",
                "gnosis.toml",
                "gnosis/",
            ],
            true,
        )?;
        if paths.is_empty() {
            println!("No catalog changes to authorize");
            return Ok(());
        }
        contributors::check(actor, &[manifest.contributors.as_ref()])
            .context("catalog contributor policy")?;
        let mut packages = BTreeSet::new();
        for path in paths
            .split(|byte| *byte == 0)
            .filter(|path| !path.is_empty())
        {
            let path = std::str::from_utf8(path)?;
            if let Some((name, _)) = path
                .strip_prefix("gnosis/")
                .and_then(|path| path.split_once('/'))
            {
                metadata::name(name)?;
                packages.insert(name.to_owned());
            }
        }
        for name in &packages {
            if let Some(bytes) =
                git::file_at(&self.root, &base, &format!("gnosis/{name}/package.toml"))?
            {
                let package: Package = metadata::decode(&bytes)?;
                contributors::check(actor, &[package.contributors.as_ref()])
                    .with_context(|| format!("package {name} contributor policy"))?;
            }
        }
        println!(
            "Contributor {actor} is allowed by the base policy for {} changed packages",
            packages.len()
        );
        Ok(())
    }
}
