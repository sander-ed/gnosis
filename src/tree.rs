use std::collections::BTreeMap;
use std::fs;
use std::path::{Component, Path};

use anyhow::{Context, Result, ensure};

pub const MAX_FILE: usize = 16 * 1024 * 1024;
pub const MAX_TREE: usize = 256 * 1024 * 1024;
pub type Tree = BTreeMap<String, Vec<u8>>;

pub fn safe_path(value: &str) -> Result<()> {
    ensure!(
        !value.is_empty()
            && !value.contains(['\\', '\0', '\n', '\r', ':'])
            && value.split('/').all(|part| !part.is_empty()
                && part != "."
                && part != ".."
                && !part.eq_ignore_ascii_case(".git"))
            && Path::new(value)
                .components()
                .all(|part| matches!(part, Component::Normal(_))),
        "unsafe package path {value:?}"
    );
    Ok(())
}

pub fn read_tree(root: &Path) -> Result<Tree> {
    fn visit(root: &Path, path: &Path, tree: &mut Tree, size: &mut usize) -> Result<()> {
        let metadata = fs::symlink_metadata(path)?;
        ensure!(
            !metadata.file_type().is_symlink(),
            "symlinks are not supported: {}",
            path.display()
        );
        if metadata.is_dir() {
            for entry in fs::read_dir(path)? {
                visit(root, &entry?.path(), tree, size)?;
            }
        } else {
            ensure!(metadata.is_file(), "not a regular file: {}", path.display());
            let relative = path
                .strip_prefix(root)?
                .to_str()
                .context("non-UTF-8 path")?
                .replace(std::path::MAIN_SEPARATOR, "/");
            safe_path(&relative)?;
            ensure!(
                metadata.len() <= MAX_FILE as u64,
                "{} exceeds 16 MiB",
                path.display()
            );
            *size += metadata.len() as usize;
            ensure!(*size <= MAX_TREE, "knowledge directory exceeds 256 MiB");
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                ensure!(
                    metadata.permissions().mode() & 0o111 == 0,
                    "executable package files are not supported: {}",
                    path.display()
                );
            }
            tree.insert(relative, fs::read(path)?);
        }
        Ok(())
    }
    let mut tree = Tree::new();
    visit(root, root, &mut tree, &mut 0)?;
    Ok(tree)
}

pub fn write_tree(root: &Path, tree: &Tree) -> Result<()> {
    let mut paths = BTreeMap::new();
    for path in tree.keys() {
        safe_path(path)?;
        let mut prefix = String::new();
        let parts: Vec<_> = path.split('/').collect();
        for (index, part) in parts.iter().enumerate() {
            if !prefix.is_empty() {
                prefix.push('/');
            }
            prefix.push_str(part);
            let entry = (prefix.clone(), index + 1 == parts.len());
            if let Some(previous) = paths.insert(prefix.to_lowercase(), entry.clone()) {
                ensure!(
                    previous == entry,
                    "package paths collide on a case-insensitive filesystem: {}",
                    prefix
                );
            }
        }
    }
    fs::create_dir_all(root)?;
    for (path, bytes) in tree {
        safe_path(path)?;
        let target = root.join(path);
        fs::create_dir_all(target.parent().context("missing parent")?)?;
        fs::write(target, bytes)?;
    }
    Ok(())
}

pub fn subtree(tree: &Tree, prefix: &str) -> Tree {
    let prefix = format!("{prefix}/");
    tree.iter()
        .filter_map(|(path, bytes)| {
            path.strip_prefix(&prefix)
                .map(|relative| (relative.to_owned(), bytes.clone()))
        })
        .collect()
}

pub fn replace_subtree(tree: &mut Tree, prefix: &str, replacement: &Tree) {
    let prefix = format!("{prefix}/");
    tree.retain(|path, _| !path.starts_with(&prefix));
    tree.extend(
        replacement
            .iter()
            .map(|(path, bytes)| (format!("{prefix}{path}"), bytes.clone())),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_trees_reject_case_and_directory_collisions_before_writing() {
        for names in [["Facts.md", "facts.md"], ["Folder", "folder/fact.md"]] {
            let directory = tempfile::tempdir().unwrap();
            let tree = names
                .into_iter()
                .map(|name| (name.to_owned(), b"content".to_vec()))
                .collect();
            assert!(write_tree(directory.path(), &tree).is_err());
            assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 0);
        }
    }
}
