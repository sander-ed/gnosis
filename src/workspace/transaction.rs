use std::fs;
use std::path::Path;

use anyhow::{Result, bail, ensure};

use super::Workspace;
use crate::tree::{self, Tree};

#[derive(Clone, PartialEq, Eq)]
pub(super) enum Entry {
    File(Vec<u8>),
    Directory(Tree),
}

impl Entry {
    pub(super) fn read(path: &Path) -> Result<Option<Self>> {
        let metadata = match fs::symlink_metadata(path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error.into()),
        };
        ensure!(
            !metadata.file_type().is_symlink(),
            "refusing symlink {}",
            path.display()
        );
        if metadata.is_dir() {
            Ok(Some(Self::Directory(tree::read_tree(path)?)))
        } else {
            ensure!(
                metadata.is_file() && metadata.len() <= tree::MAX_FILE as u64,
                "not a bounded regular file: {}",
                path.display()
            );
            Ok(Some(Self::File(fs::read(path)?)))
        }
    }

    fn write(&self, path: &Path) -> Result<()> {
        match self {
            Self::File(bytes) => Ok(fs::write(path, bytes)?),
            Self::Directory(tree) => tree::write_tree(path, tree),
        }
    }
}

pub(super) struct Change {
    pub(super) path: &'static str,
    pub(super) before: Option<Entry>,
    pub(super) after: Entry,
}

impl Workspace {
    pub(super) fn change(&self, path: &'static str, after: Entry) -> Result<Change> {
        Ok(Change {
            path,
            before: Entry::read(&self.root.join(path))?,
            after,
        })
    }

    pub(super) fn apply(&self, changes: Vec<Change>) -> Result<()> {
        let staging = tempfile::Builder::new()
            .prefix("transaction-")
            .tempdir_in(self.root.join(".gnosis"))?;
        let paths = changes
            .iter()
            .enumerate()
            .map(|(index, change)| format!("{index}\t{}\n", change.path))
            .collect::<String>();
        fs::write(staging.path().join("paths.txt"), paths)?;
        for (index, change) in changes.iter().enumerate() {
            change
                .after
                .write(&staging.path().join(format!("new-{index}")))?;
        }
        for change in &changes {
            ensure!(
                Entry::read(&self.root.join(change.path))? == change.before,
                "{} changed during the operation; retry",
                change.path
            );
        }
        let mut installed = Vec::new();
        let result = (|| -> Result<()> {
            for (index, change) in changes.iter().enumerate() {
                let destination = self.root.join(change.path);
                let backup = staging.path().join(format!("old-{index}"));
                if change.before.is_some() {
                    fs::rename(&destination, &backup)?;
                }
                installed.push(index);
                fs::rename(staging.path().join(format!("new-{index}")), destination)?;
            }
            Ok(())
        })();
        if let Err(error) = result {
            let recovery = (|| -> Result<()> {
                for index in installed.into_iter().rev() {
                    let change = &changes[index];
                    let destination = self.root.join(change.path);
                    if destination.try_exists()? {
                        fs::rename(&destination, staging.path().join(format!("failed-{index}")))?;
                    }
                    if change.before.is_some() {
                        fs::rename(staging.path().join(format!("old-{index}")), destination)?;
                    }
                }
                Ok(())
            })();
            if let Err(recovery) = recovery {
                let retained = staging.keep();
                bail!(
                    "write failed: {error:#}; rollback failed: {recovery:#}; recovery files retained at {}",
                    retained.display()
                );
            }
            return Err(error.context("write failed; previous workspace restored"));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failed_write_rolls_back_prior_replacements() {
        let directory = tempfile::tempdir().unwrap();
        fs::write(directory.path().join("first"), "original").unwrap();
        let workspace = Workspace::open(directory.path()).unwrap();
        let result = workspace.apply(vec![
            workspace
                .change("first", Entry::File(b"replacement".to_vec()))
                .unwrap(),
            workspace
                .change("missing/second", Entry::File(b"new".to_vec()))
                .unwrap(),
        ]);
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("previous workspace restored")
        );
        assert_eq!(
            fs::read(directory.path().join("first")).unwrap(),
            b"original"
        );
        assert!(!directory.path().join("missing").exists());
    }

    #[test]
    fn edits_during_preparation_are_not_overwritten() {
        let directory = tempfile::tempdir().unwrap();
        fs::write(directory.path().join("first"), "original").unwrap();
        let workspace = Workspace::open(directory.path()).unwrap();
        let change = workspace
            .change("first", Entry::File(b"replacement".to_vec()))
            .unwrap();
        fs::write(directory.path().join("first"), "concurrent edit").unwrap();
        assert!(
            workspace
                .apply(vec![change])
                .unwrap_err()
                .to_string()
                .contains("changed during")
        );
        assert_eq!(
            fs::read(directory.path().join("first")).unwrap(),
            b"concurrent edit"
        );
    }
}
