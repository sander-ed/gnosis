mod authoring;
mod contributions;
mod dependencies;
mod inspection;
mod proposal;
mod removal;
mod sources;
mod sync;
mod transaction;

pub use authoring::NewEntry;

use std::collections::BTreeSet;
use std::fs::{self, File};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail, ensure};
use fs2::FileExt;

use crate::metadata::{self, Lock, Manifest};
use crate::tree::Tree;
use transaction::Entry;

pub struct Workspace {
    root: PathBuf,
    _operation_lock: File,
}

impl Workspace {
    pub fn open(directory: &Path) -> Result<Self> {
        let root = fs::canonicalize(directory).context("workspace directory must exist")?;
        let state = root.join(".gnosis");
        if let Ok(metadata) = fs::symlink_metadata(&state) {
            ensure!(
                metadata.is_dir() && !metadata.file_type().is_symlink(),
                ".gnosis must be a regular directory"
            );
        }
        fs::create_dir_all(&state)?;
        let ignore = state.join(".gitignore");
        if !ignore.try_exists()? {
            fs::write(ignore, "*\n")?;
        }
        let lock_path = state.join("operation.lock");
        if let Ok(metadata) = fs::symlink_metadata(&lock_path) {
            ensure!(
                metadata.is_file() && !metadata.file_type().is_symlink(),
                "invalid operation lock"
            );
        }
        let lock = File::options()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(lock_path)?;
        lock.try_lock_exclusive()
            .context("another gnosis operation is running")?;
        for entry in fs::read_dir(&state)? {
            let entry = entry?;
            ensure!(
                !entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with("transaction-"),
                "interrupted transaction at {}; inspect its old-* backups and new-* files before moving that directory aside and retrying",
                entry.path().display()
            );
        }
        Ok(Self {
            root,
            _operation_lock: lock,
        })
    }

    fn manifest(&self) -> Result<Manifest> {
        let manifest = self
            .read_metadata("gnosis.toml")?
            .context("not initialized; run gnosis init")?;
        metadata::validate_manifest(&manifest)?;
        Ok(manifest)
    }

    fn read_metadata<T: for<'de> serde::Deserialize<'de>>(&self, name: &str) -> Result<Option<T>> {
        match Entry::read(&self.root.join(name))? {
            None => Ok(None),
            Some(Entry::File(bytes)) => Ok(Some(
                metadata::decode(&bytes).with_context(|| format!("invalid {name}"))?,
            )),
            _ => bail!("{name} must be a file"),
        }
    }

    fn lock(&self) -> Result<Lock> {
        let lock: Lock = self.read_metadata("gnosis.lock")?.unwrap_or(Lock {
            format: 1,
            ..Lock::default()
        });
        ensure!(lock.format == 1, "unsupported lock format");
        for (name, package) in &lock.packages {
            metadata::name(name)?;
            metadata::name(&package.source)?;
            metadata::validate_source(&package.location)?;
            ensure!(
                lock.sources.get(&package.source) == Some(&package.location),
                "locked source differs for {name}"
            );
            ensure!(
                matches!(package.commit.len(), 40 | 64)
                    && package.commit.bytes().all(|c| c.is_ascii_hexdigit()),
                "invalid commit for {name}"
            );
            for dependency in &package.dependencies {
                metadata::name(dependency)?;
                ensure!(
                    lock.packages.contains_key(dependency),
                    "lock is missing dependency {dependency} of {name}"
                );
            }
        }
        for (name, source) in &lock.requirements {
            ensure!(
                lock.packages
                    .get(name)
                    .is_some_and(|package| &package.source == source),
                "lock is missing root requirement {source}/{name}"
            );
        }
        ensure!(
            lock.reachable().len() == lock.packages.len(),
            "lock contains packages outside the requested dependency graph"
        );
        Ok(lock)
    }

    fn knowledge(&self) -> Result<Tree> {
        match Entry::read(&self.root.join("gnosis"))? {
            Some(Entry::Directory(tree)) => Ok(tree),
            None => Ok(Tree::new()),
            _ => bail!("gnosis must be a directory"),
        }
    }
}

fn package_names(manifest: &Manifest, lock: &Lock) -> BTreeSet<String> {
    manifest
        .packages
        .iter()
        .chain(lock.packages.keys())
        .cloned()
        .collect()
}
