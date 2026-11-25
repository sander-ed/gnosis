use std::fs;

use anyhow::{Context, Result, bail, ensure};

use super::Workspace;
use super::dependencies::catalog;
use super::transaction::Entry;
use crate::git::Store;
use crate::metadata::{Manifest, Package, Source};
use crate::{git, metadata};

impl Workspace {
    pub fn source(&self, name: &str, repository: String, reference: String) -> Result<()> {
        metadata::name(name)?;
        let mut manifest = self.manifest()?;
        let source = Source {
            repository: self.location(repository)?,
            reference,
        };
        metadata::validate_source(&source)?;
        manifest.sources.insert(name.into(), source);
        self.apply(vec![self.change(
            "gnosis.toml",
            Entry::File(metadata::encode(&manifest)?),
        )?])?;
        println!("Configured source {name}");
        Ok(())
    }

    pub fn list(&self) -> Result<()> {
        let manifest = self.manifest()?;
        for (source, name, package) in published(&manifest, &mut Store::new()?)? {
            println!(
                "{source}/{name}\t{}\t{}",
                package.owner,
                package.description.replace(['\n', '\r'], " ")
            );
        }
        Ok(())
    }

    /// Resolve local repository paths against the workspace root; keep URLs as given.
    fn location(&self, repository: String) -> Result<String> {
        if repository.contains(':') {
            return Ok(repository);
        }
        Ok(git::path(
            &fs::canonicalize(self.root.join(repository))
                .context("local source repository does not exist")?,
        )?
        .to_owned())
    }

    /// Add `repository` to `manifest` at the HEAD ref unless a source (named `alias`,
    /// when given) already points to it. Returns the alias and whether it is new.
    pub(super) fn register(
        &self,
        manifest: &mut Manifest,
        repository: String,
        alias: Option<String>,
    ) -> Result<(String, bool)> {
        let repository = self.location(repository)?;
        if let Some(existing) = manifest.sources.iter().find_map(|(name, source)| {
            (source.repository == repository && alias.as_ref().is_none_or(|alias| alias == name))
                .then(|| name.clone())
        }) {
            return Ok((existing, false));
        }
        let alias = match alias {
            Some(alias) => alias,
            None => derive_alias(&repository)?,
        };
        metadata::name(&alias)?;
        if let Some(other) = manifest.sources.get(&alias) {
            bail!(
                "source {alias} already points to {}; choose another alias with --source ALIAS",
                other.repository
            );
        }
        let source = Source {
            repository,
            reference: "HEAD".into(),
        };
        metadata::validate_source(&source)?;
        manifest.sources.insert(alias.clone(), source);
        Ok((alias, true))
    }
}

/// Every package the manifest's sources publish, as (source, name, package).
pub(super) fn published(
    manifest: &Manifest,
    store: &mut Store,
) -> Result<Vec<(String, String, Package)>> {
    let mut packages = Vec::new();
    for (name, candidates) in catalog(manifest, store)? {
        for candidate in candidates {
            let content = store.snapshot(
                &candidate.location.repository,
                &candidate.commit,
                &format!("gnosis/{name}"),
            )?;
            let package = metadata::package(&content, &name)?;
            packages.push((candidate.source, name.clone(), package));
        }
    }
    Ok(packages)
}

/// Derive a source alias from a repository's last path segment, such as
/// `knowledge` for `git@github.com:my-org/knowledge.git`.
pub(super) fn derive_alias(repository: &str) -> Result<String> {
    let base = repository
        .trim_end_matches(['/', '\\'])
        .rsplit(['/', '\\', ':'])
        .next()
        .unwrap_or_default();
    let base = base.strip_suffix(".git").unwrap_or(base);
    let alias = base
        .to_ascii_lowercase()
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    ensure!(
        metadata::name(&alias).is_ok(),
        "cannot derive a source alias from {repository}; pass one with --source ALIAS"
    );
    Ok(alias)
}

#[cfg(test)]
mod tests {
    use super::derive_alias;

    #[test]
    fn aliases_derive_from_the_repository_name() {
        for (repository, alias) in [
            ("git@github.com:my-org/knowledge.git", "knowledge"),
            (
                "https://github.com/my-org/Team_Knowledge/",
                "team-knowledge",
            ),
            ("/srv/repos/shared.knowledge.git", "shared-knowledge"),
            (r"C:\repos\Docs", "docs"),
        ] {
            assert_eq!(derive_alias(repository).unwrap(), alias);
        }
        assert!(derive_alias("/srv/知识/").is_err());
    }
}
