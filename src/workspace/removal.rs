use anyhow::{Context, Result, bail, ensure};

use super::dependencies::prune;
use super::inspection::validate_workspace;
use super::transaction::{Change, Entry};
use super::{Workspace, package_names};
use crate::git::Store;
use crate::tree::Tree;
use crate::{contributors, metadata, navigation, tree};

impl Workspace {
    pub fn remove(&self, name: &str, source: Option<&str>, force: bool) -> Result<()> {
        metadata::name(name)?;
        if let Some(source) = source {
            metadata::name(source)?;
        }
        let manifest_before = Entry::read(&self.root.join("gnosis.toml"))?;
        let lock_before = Entry::read(&self.root.join("gnosis.lock"))?;
        let knowledge_before = Entry::read(&self.root.join("gnosis"))?;
        let mut manifest = self.manifest()?;
        let old = self.lock()?;
        let mut lock = old.clone();
        let mut tree = match &knowledge_before {
            Some(Entry::Directory(tree)) => tree.clone(),
            None => Tree::new(),
            _ => bail!("gnosis must be a directory"),
        };
        let removed = if manifest.packages.remove(name) {
            ensure!(source.is_none(), "{name} is locally authored, not imported");
            ensure!(
                force,
                "{name} is locally authored; preserve its knowledge before using --force to delete it"
            );
            let content = tree::subtree(&tree, name);
            if !content.is_empty() {
                let package = metadata::package(&content, name)?;
                contributors::check_authenticated(package.contributors.as_ref())?;
            }
            tree::replace_subtree(&mut tree, name, &Tree::new());
            vec![name.to_owned()]
        } else {
            let installed = old
                .packages
                .get(name)
                .with_context(|| format!("unknown installed package {name}"))?;
            if let Some(source) = source {
                ensure!(
                    source == installed.source,
                    "{name} is installed from {}; requested source {source} differs",
                    installed.source
                );
            }
            ensure!(
                manifest.dependencies == old.requirements && manifest.sources == old.sources,
                "manifest and lock differ; run gnosis sync before removing imported packages"
            );
            ensure!(
                manifest.dependencies.remove(name).is_some(),
                "{name} is a transitive dependency; remove the package requiring it instead"
            );
            lock.requirements.remove(name);
            let reachable = lock.reachable();
            lock.packages.retain(|name, _| reachable.contains(name));
            prune(&mut tree, &old, &lock, force, &mut Store::new()?)?
        };
        validate_workspace(&manifest, &lock, &tree)?;
        let manual_index = tree
            .get("index.md")
            .is_some_and(|bytes| !navigation::is_generated_index(bytes));
        navigation::root_index(&mut tree, &package_names(&manifest, &lock))?;
        self.apply(vec![
            Change {
                path: "gnosis",
                before: knowledge_before,
                after: Entry::Directory(tree),
            },
            Change {
                path: "gnosis.lock",
                before: lock_before,
                after: Entry::File(metadata::encode(&lock)?),
            },
            Change {
                path: "gnosis.toml",
                before: manifest_before,
                after: Entry::File(metadata::encode(&manifest)?),
            },
        ])?;
        if removed.is_empty() {
            println!(
                "Removed direct requirement {name}; package retained because another import needs it"
            );
        } else {
            println!(
                "Removed packages: {}; review the workspace diff with Git",
                removed.join(", ")
            );
        }
        if manual_index {
            eprintln!(
                "note: preserved hand-authored gnosis/index.md; remove obsolete package links manually"
            );
        }
        Ok(())
    }
}
