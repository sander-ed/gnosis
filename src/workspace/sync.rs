use anyhow::{Context, Result, bail, ensure};

use super::dependencies::{catalog, prune, resolve, snapshot};
use super::inspection::validate_workspace;
use super::transaction::{Change, Entry};
use super::{Workspace, package_names};
use crate::git::Store;
use crate::metadata::Manifest;
use crate::tree::Tree;
use crate::{git, metadata, navigation, tree};

impl Workspace {
    pub fn add(&self, name: &str, source: Option<&str>) -> Result<()> {
        metadata::name(name)?;
        if let Some(source) = source {
            metadata::name(source)?;
        }
        let mut manifest = self.manifest()?;
        ensure!(
            !manifest.packages.contains(name),
            "{name} is locally authored and already available in this workspace at gnosis/{name}. \
             To install it elsewhere, run gnosis add from the consuming project."
        );
        let mut store = Store::new()?;
        let source = match source {
            Some(source) => source.to_owned(),
            None => {
                ensure!(
                    !manifest.sources.is_empty(),
                    "no source repositories configured; register the Git repository that publishes this package with gnosis source ALIAS REPOSITORY"
                );
                let lock = self.lock()?;
                if let Some(source) = manifest
                    .dependencies
                    .get(name)
                    .or_else(|| lock.packages.get(name).map(|package| &package.source))
                {
                    source.clone()
                } else {
                    let catalog = catalog(&manifest, &mut store)?;
                    let choices = catalog.get(name)
                        .with_context(|| format!("no configured source publishes {name}; run gnosis list to see available packages"))?;
                    ensure!(
                        choices.len() == 1,
                        "multiple sources publish {name}: {}. Select one with gnosis add SOURCE/{name} or --source SOURCE",
                        choices
                            .iter()
                            .map(|choice| format!("{}/{name}", choice.source))
                            .collect::<Vec<_>>()
                            .join(", ")
                    );
                    choices[0].source.clone()
                }
            }
        };
        ensure!(
            manifest.sources.contains_key(&source),
            "unknown source {source}; register a Git repository with gnosis source {source} REPOSITORY"
        );
        manifest.dependencies.insert(name.into(), source);
        self.synchronize(manifest, false, &mut store)
    }

    pub fn sync(&self, update: bool) -> Result<()> {
        self.synchronize(self.manifest()?, update, &mut Store::new()?)
    }

    fn synchronize(&self, manifest: Manifest, update: bool, store: &mut Store) -> Result<()> {
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
        let locked = if !update
            && old.requirements == manifest.dependencies
            && old.sources == manifest.sources
        {
            old.clone()
        } else {
            resolve(&manifest, &old, update, store)?
        };
        for (name, entry) in &locked.packages {
            ensure!(
                !manifest.packages.contains(name),
                "dependency {name} collides with a locally authored package"
            );
            let upstream = snapshot(store, name, entry)?;
            let local = tree::subtree(&original, name);
            let content = if let Some(previous) = old.packages.get(name) {
                if local.is_empty() {
                    upstream
                } else {
                    let base = snapshot(store, name, previous)?;
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
        prune(&mut tree, &old, &locked, false, store)?;
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
