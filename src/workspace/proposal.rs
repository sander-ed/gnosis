use std::path::Path;

use anyhow::{Context, Result, ensure};

use super::Workspace;
use super::dependencies::{snapshot, snapshot_content};
use crate::git::Store;
use crate::metadata::LockedPackage;
use crate::{git, metadata, navigation, okf, tree};

impl Workspace {
    pub fn propose(&self, name: &str, output: &Path) -> Result<()> {
        metadata::name(name)?;
        let manifest = self.manifest()?;
        let lock = self.lock()?;
        let pinned = lock.packages.get(name).context(
            "only imported packages need proposals; use ordinary Git for local packages",
        )?;
        ensure!(
            manifest.sources.get(&pinned.source) == Some(&pinned.location),
            "source configuration changed; sync before proposing"
        );
        let original = self.knowledge()?;
        let local = tree::subtree(&original, name);
        metadata::package(&local, name)?;
        okf::validate_okf(&local)?;
        let mut store = Store::new()?;
        let base = snapshot(&mut store, name, pinned)?;
        let base = navigation::without_generated_indexes(&base);
        let local = navigation::without_generated_indexes(&local);
        ensure!(base != local, "no local changes to propose for {name}");
        let current = store.revision(&pinned.location)?;
        let latest = LockedPackage {
            commit: current.clone(),
            ..pinned.clone()
        };
        let upstream = snapshot_content(&mut store, name, &latest)?;
        let mut merged = git::merge(
            &base,
            &local,
            &navigation::without_generated_indexes(&upstream),
            name,
        )?;
        metadata::package(&merged, name)?;
        navigation::indexes(&mut merged, name)?;
        let output = self.root.join(output);
        ensure!(
            !output.starts_with(self.root.join("gnosis")),
            "proposal checkout must be outside the knowledge directory"
        );
        git::prepare(
            &mut store,
            &pinned.location,
            &current,
            name,
            &merged,
            &output,
        )?;
        println!(
            "Prepared {}\nOnly {name} changes are staged. Nothing was committed or pushed.\n\
             Review the staged diff for confidentiality, then commit and open a source-repository PR.\n\
             Owner approval must be enforced by the source repository.",
            output.display()
        );
        Ok(())
    }
}
