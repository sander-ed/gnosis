use anyhow::{Context, Result, ensure};

use super::transaction::{Change, Entry};
use super::{Workspace, package_names};
use crate::metadata::{Lock, Manifest};
use crate::tree::Tree;
use crate::{metadata, navigation, okf, tree};

impl Workspace {
    pub fn check(&self) -> Result<()> {
        let manifest = self.manifest()?;
        let lock = self.lock()?;
        ensure!(
            manifest.dependencies == lock.requirements && manifest.sources == lock.sources,
            "manifest and lock differ; run gnosis sync"
        );
        validate_workspace(&manifest, &lock, &self.knowledge()?)?;
        println!(
            "Basic OKF and package checks passed (content accuracy and owner approval are not verified)"
        );
        Ok(())
    }

    pub fn index(&self) -> Result<()> {
        let manifest = self.manifest()?;
        let lock = self.lock()?;
        let original = self.knowledge()?;
        let mut tree = original.clone();
        for name in package_names(&manifest, &lock) {
            let mut content = tree::subtree(&tree, &name);
            metadata::package(&content, &name)?;
            navigation::indexes(&mut content, &name)?;
            tree::replace_subtree(&mut tree, &name, &content);
        }
        navigation::root_index(&mut tree, &package_names(&manifest, &lock))?;
        self.apply(vec![Change {
            path: "gnosis",
            before: Some(Entry::Directory(original)),
            after: Entry::Directory(tree),
        }])?;
        println!("Updated generated indexes; hand-authored indexes were preserved");
        Ok(())
    }
}

pub(super) fn validate_workspace(manifest: &Manifest, lock: &Lock, tree: &Tree) -> Result<()> {
    let names = package_names(manifest, lock);
    for name in &names {
        ensure!(
            !(manifest.packages.contains(name) && lock.packages.contains_key(name)),
            "{name} is both local and imported"
        );
        let content = tree::subtree(tree, name);
        let package =
            metadata::package(&content, name).with_context(|| format!("package {name}"))?;
        okf::validate_okf(&content).with_context(|| format!("package {name}"))?;
        for dependency in &package.dependencies {
            ensure!(
                names.contains(dependency),
                "{name} needs missing package {dependency}; add it to the workspace"
            );
        }
    }
    Ok(())
}
