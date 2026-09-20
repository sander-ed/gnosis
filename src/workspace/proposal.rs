use std::path::Path;

use anyhow::{Context, Result, ensure};

use super::Workspace;
use super::dependencies::{snapshot, snapshot_content};
use crate::git::Store;
use crate::metadata::{LockedPackage, Manifest};
use crate::{contributors, git, metadata, navigation, okf, tree};

impl Workspace {
    pub fn propose(&self, name: &str, source: Option<&str>, output: &Path) -> Result<()> {
        metadata::name(name)?;
        let manifest = self.manifest()?;
        ensure!(
            !manifest.packages.contains(name),
            "{name} is locally authored here; create a Git branch in this repository, commit your package changes, and open a pull request. Run gnosis propose from a consuming workspace for imported packages."
        );
        let lock = self.lock()?;
        let pinned = lock.packages.get(name).context(
            "package is not installed; use gnosis add SOURCE/NAME in a consuming workspace first",
        )?;
        if let Some(source) = source {
            metadata::name(source)?;
            ensure!(
                source == pinned.source,
                "{name} is installed from {}; requested source {source} differs",
                pinned.source
            );
        }
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
        let repository = store.repository(&pinned.location.repository)?;
        let published: Manifest = metadata::decode(
            &git::file_at(&repository, &current, "gnosis.toml")?
                .context("source is missing gnosis.toml")?,
        )?;
        metadata::validate_manifest(&published)?;
        ensure!(
            published.packages.contains(name),
            "source no longer publishes {name}"
        );
        let package = metadata::package(&upstream, name)?;
        contributors::check_authenticated(&[
            published.contributors.as_ref(),
            package.contributors.as_ref(),
        ])?;
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
