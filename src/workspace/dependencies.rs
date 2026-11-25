use std::collections::{BTreeMap, BTreeSet, VecDeque};

use anyhow::{Context, Result, ensure};

use super::sources::derive_alias;
use crate::git::Store;
use crate::metadata::{Lock, LockedPackage, Manifest, Source};
use crate::tree::Tree;
use crate::{git, metadata, navigation, okf, tree};

pub(super) fn prune(
    tree: &mut Tree,
    old: &Lock,
    retained: &Lock,
    force: bool,
    store: &mut Store,
) -> Result<Vec<String>> {
    let mut removed = Vec::new();
    for (name, previous) in &old.packages {
        if retained.packages.contains_key(name) {
            continue;
        }
        let local = tree::subtree(tree, name);
        if !force && !local.is_empty() {
            let base = snapshot(store, name, previous)
                .with_context(|| format!("cannot verify local changes in unused package {name}"))?;
            ensure!(
                navigation::without_generated_indexes(&local)
                    == navigation::without_generated_indexes(&base),
                "unused package {name} has local changes; preserve them outside gnosis/{name} before removal, or use --force when removing its direct requirement"
            );
        }
        tree::replace_subtree(tree, name, &Tree::new());
        removed.push(name.clone());
    }
    Ok(removed)
}

pub(super) fn snapshot_content(
    store: &mut Store,
    name: &str,
    entry: &LockedPackage,
) -> Result<Tree> {
    let tree = store.snapshot(
        &entry.location.repository,
        &entry.commit,
        &format!("gnosis/{name}"),
    )?;
    metadata::package(&tree, name)?;
    okf::validate_okf(&tree)?;
    Ok(tree)
}

pub(super) fn snapshot(store: &mut Store, name: &str, entry: &LockedPackage) -> Result<Tree> {
    let tree = snapshot_content(store, name, entry)?;
    ensure!(
        metadata::package(&tree, name)?.dependencies == entry.dependencies,
        "locked dependencies disagree with source snapshot for {name}"
    );
    Ok(tree)
}

pub(super) fn catalog(
    manifest: &Manifest,
    store: &mut Store,
) -> Result<BTreeMap<String, Vec<LockedPackage>>> {
    let mut catalog: BTreeMap<String, Vec<LockedPackage>> = BTreeMap::new();
    for (alias, source) in &manifest.sources {
        let commit = store
            .revision(source)
            .with_context(|| format!("source {alias}"))?;
        let repository = store.repository(&source.repository)?;
        let bytes = git::run(
            &repository,
            &["show", &format!("{commit}:gnosis.toml")],
            true,
        )?;
        let published: Manifest = metadata::decode(&bytes)?;
        metadata::validate_manifest(&published)?;
        for name in published.packages {
            catalog.entry(name).or_default().push(LockedPackage {
                source: alias.clone(),
                location: source.clone(),
                commit: commit.clone(),
                dependencies: BTreeSet::new(),
            });
        }
    }
    Ok(catalog)
}

/// Which locked packages resolution may move to their source's current commit.
pub(super) enum Update {
    None,
    All,
    /// These packages and everything they depend on.
    Packages(BTreeSet<String>),
}

/// A repository's published metadata at one commit.
struct Published {
    manifest: Manifest,
    lock: Option<Lock>,
}

/// The repository that authors a package.
enum Owner {
    /// This workspace.
    Local,
    /// `location`, which the repository that pointed to it called `alias`.
    Remote { location: Source, alias: String },
}

struct Resolver<'a> {
    store: &'a mut Store,
    workspace: &'a BTreeSet<String>,
    revisions: BTreeMap<(String, String), String>,
    published: BTreeMap<(String, String), Published>,
}

impl Resolver<'_> {
    fn revision(&mut self, source: &Source) -> Result<String> {
        let key = (source.repository.clone(), source.reference.clone());
        if let Some(commit) = self.revisions.get(&key) {
            return Ok(commit.clone());
        }
        let commit = self
            .store
            .revision(source)
            .with_context(|| format!("cannot read {}", source.repository))?;
        self.revisions.insert(key, commit.clone());
        Ok(commit)
    }

    fn published(&mut self, repository: &str, commit: &str) -> Result<&Published> {
        let key = (repository.to_owned(), commit.to_owned());
        if !self.published.contains_key(&key) {
            let path = self
                .store
                .fetch(repository, commit)
                .with_context(|| format!("cannot read {repository}"))?;
            let bytes = git::file_at(&path, commit, "gnosis.toml")?
                .with_context(|| format!("{repository} has no gnosis.toml"))?;
            let manifest: Manifest = metadata::decode(&bytes)
                .with_context(|| format!("invalid gnosis.toml in {repository}"))?;
            metadata::validate_manifest(&manifest)?;
            // The lock only helps to locate imports, so an unreadable one is ignored.
            let lock = git::file_at(&path, commit, "gnosis.lock")
                .ok()
                .flatten()
                .and_then(|bytes| metadata::decode(&bytes).ok());
            self.published
                .insert(key.clone(), Published { manifest, lock });
        }
        Ok(&self.published[&key])
    }

    /// Follow `name` from the repository at `location` through the repositories
    /// that import it to the one that authors it.
    fn owner(
        &mut self,
        name: &str,
        mut location: Source,
        mut commit: Option<String>,
        mut alias: String,
    ) -> Result<Owner> {
        let mut visited = BTreeSet::new();
        loop {
            let identity = metadata::repository_identity(&location.repository);
            if self.workspace.contains(&identity) {
                return Ok(Owner::Local);
            }
            ensure!(
                visited.insert(identity),
                "{name} is imported in a cycle through {} and no repository authors it",
                location.repository
            );
            let commit_value = match commit {
                Some(commit) => commit,
                None => self.revision(&location)?,
            };
            let published = self.published(&location.repository, &commit_value)?;
            if published.manifest.packages.contains(name) {
                return Ok(Owner::Remote { location, alias });
            }
            (location, commit, alias) = match published
                .lock
                .as_ref()
                .and_then(|lock| lock.packages.get(name))
            {
                Some(locked) => (
                    locked.location.clone(),
                    Some(locked.commit.clone()),
                    locked.source.clone(),
                ),
                None => published
                    .manifest
                    .dependencies
                    .get(name)
                    .and_then(|alias| {
                        let source = published.manifest.sources.get(alias)?;
                        Some((source.clone(), None, alias.clone()))
                    })
                    .with_context(|| {
                        format!("{} neither authors nor imports {name}", location.repository)
                    })?,
            };
        }
    }
}

/// The configured source for the repository at `location`, registering it under
/// the alias its importer used (or one derived from its URL) when none exists.
fn owner_source(
    manifest: &mut Manifest,
    old: &Lock,
    name: &str,
    via: &str,
    location: &Source,
    alias: &str,
) -> Result<String> {
    let identity = metadata::repository_identity(&location.repository);
    let matches = |alias: &str| {
        manifest
            .sources
            .get(alias)
            .is_some_and(|source| metadata::repository_identity(&source.repository) == identity)
    };
    let preferred = [
        Some(via),
        manifest.dependencies.get(name).map(String::as_str),
        old.packages
            .get(name)
            .map(|package| package.source.as_str()),
    ];
    if let Some(existing) = preferred
        .into_iter()
        .flatten()
        .find(|alias| matches(alias))
        .or_else(|| {
            manifest
                .sources
                .keys()
                .map(String::as_str)
                .find(|alias| matches(alias))
        })
    {
        return Ok(existing.to_owned());
    }
    let alias = if metadata::name(alias).is_ok() && !manifest.sources.contains_key(alias) {
        alias.to_owned()
    } else {
        derive_alias(&location.repository)?
    };
    ensure!(
        !manifest.sources.contains_key(&alias),
        "{name} is owned by {0}, which no configured source points to; register it with gnosis source ALIAS {0}",
        location.repository
    );
    metadata::validate_source(location)?;
    manifest.sources.insert(alias.clone(), location.clone());
    Ok(alias)
}

/// Lock the manifest's requirements and their dependencies. Every package resolves
/// to the repository that authors it, whichever repository imported it on the
/// way; this may add sources to `manifest` and move requirements to their owner.
/// Packages this workspace (whose repository identities are `workspace`) authors
/// satisfy dependencies locally and are not locked.
pub(super) fn resolve(
    manifest: &mut Manifest,
    old: &Lock,
    mut update: Update,
    workspace: &BTreeSet<String>,
    store: &mut Store,
) -> Result<Lock> {
    if let Update::Packages(names) = &mut update {
        let mut pending: Vec<_> = names.iter().cloned().collect();
        while let Some(name) = pending.pop() {
            for dependency in old
                .packages
                .get(&name)
                .into_iter()
                .flat_map(|p| &p.dependencies)
            {
                if names.insert(dependency.clone()) {
                    pending.push(dependency.clone());
                }
            }
        }
    }
    let refreshes = |update: &Update, name: &str| match update {
        Update::None => false,
        Update::All => true,
        Update::Packages(names) => names.contains(name),
    };
    let mut resolver = Resolver {
        store,
        workspace,
        revisions: BTreeMap::new(),
        published: BTreeMap::new(),
    };
    let mut packages: BTreeMap<String, LockedPackage> = BTreeMap::new();
    // (package, source it was reached through, that source's commit, dependent package)
    let mut pending: VecDeque<_> = manifest
        .dependencies
        .iter()
        .map(|(name, via)| {
            let pinned = old
                .packages
                .get(name)
                .filter(|previous| {
                    !refreshes(&update, name)
                        && &previous.source == via
                        && manifest.sources.get(via) == Some(&previous.location)
                })
                .map(|previous| previous.commit.clone());
            (name.clone(), via.clone(), pinned, None::<String>)
        })
        .collect();
    while let Some((name, via, commit, dependent)) = pending.pop_front() {
        let needed = match &dependent {
            Some(dependent) => format!("{dependent} depends on {name}"),
            None => format!("requirement {via}/{name}"),
        };
        let location = manifest
            .sources
            .get(&via)
            .with_context(|| format!("unknown source {via}"))?
            .clone();
        let owner = resolver
            .owner(&name, location, commit, via.clone())
            .with_context(|| match &dependent {
                Some(dependent) => format!("cannot resolve {name}, which {dependent} depends on"),
                None => format!("cannot resolve {name} from source {via}"),
            })?;
        let (location, alias) = match owner {
            Owner::Local => {
                ensure!(
                    dependent.is_some() && manifest.packages.contains(&name),
                    "{needed}, which this workspace owns, but gnosis/{name} is not one of its packages"
                );
                continue;
            }
            Owner::Remote { location, alias } => (location, alias),
        };
        ensure!(
            !manifest.packages.contains(&name),
            "{needed}, which {} owns, but this workspace authors a different {name}",
            location.repository
        );
        if let Some(existing) = packages.get(&name) {
            ensure!(
                metadata::repository_identity(&existing.location.repository)
                    == metadata::repository_identity(&location.repository),
                "{needed} from its owner {}, but this workspace installs {name} from {}",
                location.repository,
                existing.location.repository
            );
            continue;
        }
        let alias = owner_source(manifest, old, &name, &via, &location, &alias)?;
        if dependent.is_none() {
            manifest.dependencies.insert(name.clone(), alias.clone());
        }
        let source = manifest.sources[&alias].clone();
        let retained = old.packages.get(&name).filter(|previous| {
            !refreshes(&update, &name) && previous.source == alias && previous.location == source
        });
        let mut selected = match retained {
            Some(previous) => previous.clone(),
            None => {
                let commit = resolver
                    .revision(&source)
                    .with_context(|| format!("cannot install {name} from its owner {alias}"))?;
                ensure!(
                    resolver
                        .published(&source.repository, &commit)?
                        .manifest
                        .packages
                        .contains(&name),
                    "source {alias} no longer publishes {name}"
                );
                LockedPackage {
                    source: alias.clone(),
                    location: source,
                    commit,
                    dependencies: BTreeSet::new(),
                }
            }
        };
        let content = snapshot_content(resolver.store, &name, &selected)?;
        selected.dependencies = metadata::package(&content, &name)?.dependencies;
        if let (true, Update::Packages(names)) = (refreshes(&update, &name), &mut update) {
            names.extend(selected.dependencies.iter().cloned());
        }
        for dependency in &selected.dependencies {
            pending.push_back((
                dependency.clone(),
                alias.clone(),
                Some(selected.commit.clone()),
                Some(name.clone()),
            ));
        }
        packages.insert(name, selected);
    }
    Ok(Lock {
        format: 1,
        requirements: manifest.dependencies.clone(),
        sources: manifest.sources.clone(),
        packages,
    })
}
