use std::fs;

use anyhow::{Context, Result};

use super::Workspace;
use super::dependencies::catalog;
use super::transaction::Entry;
use crate::git::Store;
use crate::metadata::Source;
use crate::{git, metadata};

impl Workspace {
    pub fn source(&self, name: &str, repository: String, reference: String) -> Result<()> {
        metadata::name(name)?;
        let mut manifest = self.manifest()?;
        let repository = if !repository.contains(':') {
            git::path(
                &fs::canonicalize(self.root.join(repository))
                    .context("local source repository does not exist")?,
            )?
            .to_owned()
        } else {
            repository
        };
        let source = Source {
            repository,
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
        let mut store = Store::new()?;
        let catalog = catalog(&manifest, &mut store)?;
        for (name, candidates) in catalog {
            for candidate in candidates {
                let content = store.snapshot(
                    &candidate.location.repository,
                    &candidate.commit,
                    &format!("gnosis/{name}"),
                )?;
                let package = metadata::package(&content, &name)?;
                println!(
                    "{}/{}\t{}\t{}",
                    candidate.source,
                    name,
                    package.owner,
                    package.description.replace(['\n', '\r'], " ")
                );
            }
        }
        Ok(())
    }
}
