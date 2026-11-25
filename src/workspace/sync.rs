use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result, bail, ensure};

use super::dependencies::{Update, catalog, prune, resolve, snapshot};
use super::inspection::validate_workspace;
use super::sources::published;
use super::transaction::{Change, Entry};
use super::{Workspace, package_names};
use crate::git::Store;
use crate::metadata::Manifest;
use crate::tree::Tree;
use crate::{git, metadata, navigation, tree};

/// A package a configured source publishes, as offered by `Workspace::add_selected`.
pub struct Available {
    pub source: String,
    pub name: String,
    pub description: String,
    pub installed: bool,
}

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
        self.synchronize(manifest, Update::None, false, &mut store)
    }

    /// Offer published packages to `choose`, then install the chosen ones and update
    /// those already installed. With `repository`, offers only that repository's
    /// packages, registering it under `alias` (or a name derived from its URL) unless
    /// a configured source already points to it. Nothing is written unless packages
    /// are chosen.
    pub fn add_selected(
        &self,
        repository: Option<(String, Option<String>)>,
        choose: impl FnOnce(&[Available]) -> Result<Option<Vec<usize>>>,
    ) -> Result<()> {
        let mut manifest = self.manifest()?;
        let registered = match repository {
            Some((repository, alias)) => Some(self.register(&mut manifest, repository, alias)?),
            None => None,
        };
        ensure!(
            !manifest.sources.is_empty(),
            "no source repositories configured; run gnosis add REPOSITORY or register one with gnosis source ALIAS REPOSITORY"
        );
        let lock = self.lock()?;
        let mut store = Store::new()?;
        let available: Vec<_> = published(&manifest, &mut store)?
            .into_iter()
            .filter(|(source, name, _)| {
                registered.as_ref().is_none_or(|(alias, _)| alias == source)
                    && !manifest.packages.contains(name)
            })
            .map(|(source, name, package)| Available {
                installed: lock
                    .packages
                    .get(&name)
                    .is_some_and(|locked| locked.source == source),
                source,
                name,
                description: package.description,
            })
            .collect();
        ensure!(!available.is_empty(), "no installable packages published");
        let Some(chosen) = choose(&available)? else {
            println!("Cancelled; nothing changed");
            return Ok(());
        };
        if chosen.is_empty() {
            println!("No packages selected; nothing changed");
            return Ok(());
        }
        let mut sources = BTreeMap::new();
        for package in chosen.iter().map(|&index| &available[index]) {
            if let Some(other) = sources.insert(&package.name, &package.source) {
                bail!(
                    "selected {0} from both {other} and {1}; choose one source for {0}",
                    package.name,
                    package.source
                );
            }
            manifest
                .dependencies
                .insert(package.name.clone(), package.source.clone());
        }
        let update = Update::Packages(sources.into_keys().cloned().collect());
        self.synchronize(manifest, update, false, &mut store)?;
        if let Some((alias, true)) = registered {
            println!("Configured source {alias}");
        }
        Ok(())
    }

    pub fn sync(&self, update: bool, merge: bool) -> Result<()> {
        let update = if update { Update::All } else { Update::None };
        self.synchronize(self.manifest()?, update, merge, &mut Store::new()?)
    }

    fn synchronize(
        &self,
        manifest: Manifest,
        update: Update,
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
        let locked = if matches!(update, Update::None)
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

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;
    use std::process::Command;

    use super::*;

    fn git(root: &Path, args: &[&str]) -> String {
        let output = Command::new("git")
            .arg("-C")
            .arg(root)
            .args([
                "-c",
                "user.name=Owner",
                "-c",
                "user.email=owner@example.test",
            ])
            .args([
                "-c",
                "commit.gpgsign=false",
                "-c",
                "core.hooksPath=/dev/null",
            ])
            .args(args)
            .output()
            .unwrap();
        assert!(output.status.success(), "{output:?}");
        String::from_utf8(output.stdout).unwrap().trim().to_owned()
    }

    fn publish(root: &Path, fact: &str) -> String {
        fs::write(
            root.join("gnosis/core/facts.md"),
            format!("---\ntype: Reference\n---\n# Facts\n\n{fact}\n"),
        )
        .unwrap();
        Workspace::open(root).unwrap().index().unwrap();
        git(root, &["add", "--all"]);
        git(root, &["commit", "--quiet", "-m", "Publish"]);
        git(root, &["rev-parse", "HEAD"])
    }

    fn locked_commit(root: &Path, name: &str) -> String {
        Workspace::open(root).unwrap().lock().unwrap().packages[name]
            .commit
            .clone()
    }

    #[test]
    fn selected_packages_install_and_update_with_their_dependencies() {
        let temporary = tempfile::tempdir().unwrap();
        let source = temporary.path().join("team-knowledge");
        let consumer = temporary.path().join("consumer");
        fs::create_dir(&source).unwrap();
        fs::create_dir(&consumer).unwrap();
        git(&source, &["init", "--quiet"]);
        {
            let workspace = Workspace::open(&source).unwrap();
            workspace.init().unwrap();
            workspace
                .package("core", "@org/core".into(), "Core".into())
                .unwrap();
            workspace
                .package("platform", "@org/platform".into(), String::new())
                .unwrap();
        }
        fs::write(
            source.join("gnosis/platform/package.toml"),
            "name = 'platform'\nowner = '@org/platform'\ndependencies = ['core']\n",
        )
        .unwrap();
        let first = publish(&source, "Original.");
        Workspace::open(&consumer).unwrap().init().unwrap();
        let repository = source.to_str().unwrap().to_owned();

        Workspace::open(&consumer)
            .unwrap()
            .add_selected(Some((repository.clone(), None)), |available| {
                let offered: Vec<_> = available
                    .iter()
                    .map(|p| (p.source.as_str(), p.name.as_str(), p.installed))
                    .collect();
                assert_eq!(
                    offered,
                    [
                        ("team-knowledge", "core", false),
                        ("team-knowledge", "platform", false)
                    ]
                );
                Ok(Some(vec![1]))
            })
            .unwrap();
        let manifest = Workspace::open(&consumer).unwrap().manifest().unwrap();
        assert_eq!(manifest.sources["team-knowledge"].reference, "HEAD");
        assert_eq!(manifest.dependencies["platform"], "team-knowledge");
        assert_eq!(locked_commit(&consumer, "core"), first);

        // Cancelling writes nothing, even for a repository that is not yet registered.
        let before = fs::read(consumer.join("gnosis.toml")).unwrap();
        let other = temporary.path().join("other");
        fs::create_dir(&other).unwrap();
        git(&other, &["init", "--quiet"]);
        Workspace::open(&other).unwrap().init().unwrap();
        Workspace::open(&other)
            .unwrap()
            .package("docs", "@org/docs".into(), String::new())
            .unwrap();
        git(&other, &["add", "--all"]);
        git(&other, &["commit", "--quiet", "-m", "Publish"]);
        Workspace::open(&consumer)
            .unwrap()
            .add_selected(Some((other.to_str().unwrap().into(), None)), |_| Ok(None))
            .unwrap();
        assert_eq!(fs::read(consumer.join("gnosis.toml")).unwrap(), before);

        // Selecting an installed package updates it and its dependencies.
        let second = publish(&source, "Updated.");
        Workspace::open(&consumer)
            .unwrap()
            .add_selected(None, |available| {
                assert!(available.iter().all(|p| p.installed));
                Ok(Some(vec![1]))
            })
            .unwrap();
        assert_eq!(locked_commit(&consumer, "platform"), second);
        assert_eq!(locked_commit(&consumer, "core"), second);
        assert!(
            fs::read_to_string(consumer.join("gnosis/core/facts.md"))
                .unwrap()
                .contains("Updated.")
        );
    }
}
