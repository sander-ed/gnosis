use anyhow::{Result, bail, ensure};

use super::dependencies::{resolve, snapshot};
use super::inspection::validate_workspace;
use super::transaction::{Change, Entry};
use super::{Workspace, package_names};
use crate::git::Store;
use crate::metadata::Manifest;
use crate::tree::Tree;
use crate::{git, metadata, navigation, tree};

impl Workspace {
    pub fn add(&self, name: &str, source: &str) -> Result<()> {
        metadata::name(name)?;
        let mut manifest = self.manifest()?;
        ensure!(
            !manifest.packages.contains(name),
            "{name} is locally authored"
        );
        ensure!(
            manifest.sources.contains_key(source),
            "unknown source {source}"
        );
        manifest.dependencies.insert(name.into(), source.into());
        self.synchronize(manifest, false)
    }

    pub fn sync(&self, update: bool) -> Result<()> {
        self.synchronize(self.manifest()?, update)
    }

    fn synchronize(&self, manifest: Manifest, update: bool) -> Result<()> {
        metadata::validate_manifest(&manifest)?;
        let manifest_before = Entry::read(&self.root.join("gnosis.toml"))?;
        let lock_before = Entry::read(&self.root.join("gnosis.lock"))?;
        let old = self.lock()?;
        let knowledge_before = Entry::read(&self.root.join("gnosis"))?;
        let original = match &knowledge_before {
            Some(Entry::Directory(tree)) => tree.clone(),
            None => Tree::new(),
            _ => bail!("gnosis must be a directory"),
        };
        let mut tree = original.clone();
        let mut store = Store::new()?;
        let locked = if !update
            && old.requirements == manifest.dependencies
            && old.sources == manifest.sources
        {
            old.clone()
        } else {
            resolve(&manifest, &old, update, &mut store)?
        };
        for (name, entry) in &locked.packages {
            ensure!(
                !manifest.packages.contains(name),
                "dependency {name} collides with a locally authored package"
            );
            let mut upstream = snapshot(&mut store, name, entry)?;
            navigation::indexes(&mut upstream, name)?;
            let local = tree::subtree(&original, name);
            let content = if let Some(previous) = old.packages.get(name) {
                if local.is_empty() {
                    upstream
                } else {
                    let mut base = snapshot(&mut store, name, previous)?;
                    navigation::indexes(&mut base, name)?;
                    git::merge(
                        &navigation::without_generated_indexes(&base),
                        &navigation::without_generated_indexes(&local),
                        &navigation::without_generated_indexes(&upstream),
                        name,
                    )?
                }
            } else {
                ensure!(
                    local.is_empty(),
                    "refusing to overwrite unmanaged package directory {name}"
                );
                upstream
            };
            let mut content = content;
            metadata::package(&content, name)?;
            navigation::indexes(&mut content, name)?;
            tree::replace_subtree(&mut tree, name, &content);
        }
        for (name, previous) in &old.packages {
            if locked.packages.contains_key(name) {
                continue;
            }
            let local = tree::subtree(&original, name);
            let mut base = snapshot(&mut store, name, previous)?;
            navigation::indexes(&mut base, name)?;
            ensure!(
                local.is_empty() || local == base,
                "unused package {name} has local changes; preserve them outside gnosis/{name} before removing the dependency"
            );
            tree::replace_subtree(&mut tree, name, &Tree::new());
        }
        validate_workspace(&manifest, &locked, &tree)?;
        navigation::root_index(&mut tree, &package_names(&manifest, &locked))?;
        self.apply(vec![
            Change {
                path: "gnosis",
                before: knowledge_before,
                after: Entry::Directory(tree),
            },
            Change {
                path: "gnosis.lock",
                before: lock_before,
                after: Entry::File(metadata::encode(&locked)?),
            },
            Change {
                path: "gnosis.toml",
                before: manifest_before,
                after: Entry::File(metadata::encode(&manifest)?),
            },
        ])?;
        println!(
            "Synced {} imported packages; review the workspace diff with Git",
            locked.packages.len()
        );
        Ok(())
    }
}
