use std::collections::{BTreeMap, BTreeSet, VecDeque};

use anyhow::{Context, Result, ensure};

use crate::git::Store;
use crate::metadata::{Lock, LockedPackage, Manifest};
use crate::tree::Tree;
use crate::{git, metadata, okf};

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

pub(super) fn resolve(
    manifest: &Manifest,
    old: &Lock,
    update: bool,
    store: &mut Store,
) -> Result<Lock> {
    let catalog = catalog(manifest, store)?;
    let mut lock = Lock {
        format: 1,
        requirements: manifest.dependencies.clone(),
        sources: manifest.sources.clone(),
        packages: BTreeMap::new(),
    };
    let mut pending: VecDeque<_> = manifest.dependencies.keys().cloned().collect();
    while let Some(name) = pending.pop_front() {
        if lock.packages.contains_key(&name) {
            continue;
        }
        let preferred = manifest.dependencies.get(&name);
        let retained = old.packages.get(&name).filter(|previous| {
            !update
                && manifest.sources.get(&previous.source) == Some(&previous.location)
                && preferred.is_none_or(|source| source == &previous.source)
        });
        let mut selected = if let Some(previous) = retained {
            previous.clone()
        } else {
            let choices = catalog
                .get(&name)
                .with_context(|| format!("no configured source publishes {name}"))?;
            let preferred =
                preferred.or_else(|| old.packages.get(&name).map(|package| &package.source));
            if let Some(source) = preferred {
                choices.iter().find(|choice| &choice.source == source).with_context(|| format!("source {source} no longer publishes {name}; select its source explicitly"))?.clone()
            } else {
                ensure!(
                    choices.len() == 1,
                    "ambiguous dependency {name}; select a source with gnosis add {name} --source NAME"
                );
                choices[0].clone()
            }
        };
        let content = snapshot_content(store, &name, &selected)?;
        selected.dependencies = metadata::package(&content, &name)?.dependencies;
        pending.extend(selected.dependencies.iter().cloned());
        lock.packages.insert(name, selected);
    }
    Ok(lock)
}
