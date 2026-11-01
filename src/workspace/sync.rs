use std::collections::{BTreeMap, BTreeSet};

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
        self.synchronize(manifest, false, false, &mut store)
    }

    pub fn sync(&self, update: bool, merge: bool) -> Result<()> {
        self.synchronize(self.manifest()?, update, merge, &mut Store::new()?)
    }

    fn synchronize(
        &self,
        manifest: Manifest,
        update: bool,
        merge: bool,
        store: &mut Store,
    ) -> Result<()> {
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
        let mut discarded = Vec::new();
        let mut conflicts = Vec::new();
        let mut conflicted = BTreeMap::new();
        for (name, entry) in &locked.packages {
            ensure!(
                !manifest.packages.contains(name),
                "dependency {name} collides with a locally authored package"
            );
            let upstream = snapshot(store, name, entry)?;
            let local = tree::subtree(&original, name);
            let mut content = match old.packages.get(name) {
                Some(_) if local.is_empty() => upstream,
                Some(previous) => {
                    let base =
                        navigation::without_generated_indexes(&snapshot(store, name, previous)?);
                    let mine = navigation::without_generated_indexes(&local);
                    let theirs = navigation::without_generated_indexes(&upstream);
                    if merge {
                        let merged = git::merge(&base, &mine, &theirs)?;
                        if !merged.conflicts.is_empty() {
                            conflicts.extend(
                                merged
                                    .conflicts
                                    .iter()
                                    .map(|path| format!("gnosis/{name}/{path}")),
                            );
                            // Conflict markers may break OKF parsing, so keep the previous
                            // navigation until the user resolves them and runs gnosis index.
                            let mut content = merged.tree;
                            for (path, bytes) in &local {
                                if !content.contains_key(path)
                                    && navigation::is_generated_index(bytes)
                                {
                                    content.insert(path.clone(), bytes.clone());
                                }
                            }
                            tree::replace_subtree(&mut tree, name, &content);
                            conflicted.insert(name.clone(), upstream);
                            continue;
                        }
                        merged.tree
                    } else if base == theirs {
                        mine
                    } else {
                        let paths: BTreeSet<_> = mine.keys().chain(base.keys()).collect();
                        discarded.extend(
                            paths
                                .into_iter()
                                .filter(|path| {
                                    mine.get(*path) != base.get(*path)
                                        && mine.get(*path) != theirs.get(*path)
                                })
                                .map(|path| format!("gnosis/{name}/{path}")),
                        );
                        theirs
                    }
                }
                None => {
                    ensure!(
                        local.is_empty(),
                        "refusing to overwrite unmanaged package directory {name}"
                    );
                    upstream
                }
            };
            metadata::package(&content, name)?;
            navigation::indexes(&mut content, name)?;
            tree::replace_subtree(&mut tree, name, &content);
        }
        prune(&mut tree, &old, &locked, false, store)?;
        // Validate conflicted packages against their upstream content instead.
        let mut validated = tree.clone();
        for (name, upstream) in &conflicted {
            tree::replace_subtree(&mut validated, name, upstream);
        }
        validate_workspace(&manifest, &locked, &validated)?;
        navigation::root_index(&mut validated, &package_names(&manifest, &locked))?;
        if let Some(index) = validated.remove("index.md") {
            tree.insert("index.md".into(), index);
        }
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
        if !discarded.is_empty() {
            println!(
                "Replaced local edits with upstream (restore them from Git and rerun with --merge to keep them):"
            );
            for path in &discarded {
                println!("  {path}");
            }
        }
        if !conflicts.is_empty() {
            println!(
                "Merge conflicts need resolution; edit the markers, then run gnosis index and gnosis check:"
            );
            for path in &conflicts {
                println!("  {path}");
            }
        }
        println!(
            "Synced {} imported packages; review the workspace diff with Git",
            locked.packages.len()
        );
        Ok(())
    }
}
