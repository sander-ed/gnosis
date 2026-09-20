use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fs::{self, File};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail, ensure};
use fs2::FileExt;

use crate::git::{self, Store};
use crate::model::{self, Lock, LockedPackage, Manifest, Package, Source, Tree};

#[derive(Clone, PartialEq, Eq)]
enum Entry {
    File(Vec<u8>),
    Directory(Tree),
}

impl Entry {
    fn read(path: &Path) -> Result<Option<Self>> {
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
            Ok(Some(Self::Directory(model::read_tree(path)?)))
        } else {
            ensure!(
                metadata.is_file() && metadata.len() <= model::MAX_FILE as u64,
                "not a bounded regular file: {}",
                path.display()
            );
            Ok(Some(Self::File(fs::read(path)?)))
        }
    }

    fn write(&self, path: &Path) -> Result<()> {
        match self {
            Self::File(bytes) => Ok(fs::write(path, bytes)?),
            Self::Directory(tree) => model::write_tree(path, tree),
        }
    }
}

struct Change {
    path: &'static str,
    before: Option<Entry>,
    after: Entry,
}

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
        model::validate_manifest(&manifest)?;
        Ok(manifest)
    }

    fn read_metadata<T: for<'de> serde::Deserialize<'de>>(&self, name: &str) -> Result<Option<T>> {
        match Entry::read(&self.root.join(name))? {
            None => Ok(None),
            Some(Entry::File(bytes)) => Ok(Some(
                model::decode(&bytes).with_context(|| format!("invalid {name}"))?,
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
            model::name(name)?;
            model::name(&package.source)?;
            model::validate_source(&package.location)?;
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
                model::name(dependency)?;
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
        let mut reachable = BTreeSet::new();
        let mut pending: Vec<_> = lock.requirements.keys().cloned().collect();
        while let Some(name) = pending.pop() {
            if reachable.insert(name.clone()) {
                pending.extend(lock.packages[&name].dependencies.iter().cloned());
            }
        }
        ensure!(
            reachable.len() == lock.packages.len(),
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

    fn change(&self, path: &'static str, after: Entry) -> Result<Change> {
        Ok(Change {
            path,
            before: Entry::read(&self.root.join(path))?,
            after,
        })
    }

    fn apply(&self, changes: Vec<Change>) -> Result<()> {
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

    pub fn init(&self) -> Result<()> {
        ensure!(
            !self.root.join("gnosis.toml").try_exists()?,
            "workspace already initialized"
        );
        ensure!(
            !self.root.join("gnosis").try_exists()?,
            "gnosis directory already exists; refusing to adopt or overwrite it"
        );
        ensure!(
            !self.root.join("gnosis.lock").try_exists()?,
            "gnosis.lock already exists"
        );
        let manifest = Manifest {
            format: 1,
            ..Manifest::default()
        };
        let lock = Lock {
            format: 1,
            ..Lock::default()
        };
        let mut tree = Tree::new();
        model::root_index(&mut tree, &BTreeSet::new())?;
        self.apply(vec![
            self.change("gnosis.toml", Entry::File(model::encode(&manifest)?))?,
            self.change("gnosis.lock", Entry::File(model::encode(&lock)?))?,
            self.change("gnosis", Entry::Directory(tree))?,
        ])?;
        println!("Initialized {}", self.root.display());
        Ok(())
    }

    pub fn package(&self, name: &str, owner: String, description: String) -> Result<()> {
        model::name(name)?;
        ensure!(!owner.trim().is_empty(), "owner cannot be empty");
        let mut manifest = self.manifest()?;
        ensure!(
            !manifest.dependencies.contains_key(name),
            "{name} is imported"
        );
        ensure!(
            !self.root.join("gnosis").join(name).try_exists()?,
            "package directory already exists"
        );
        let original = self.knowledge()?;
        let mut tree = original.clone();
        let package = Package {
            name: name.into(),
            owner,
            description,
            dependencies: BTreeSet::new(),
        };
        let mut content = Tree::from([("package.toml".into(), model::encode(&package)?)]);
        model::indexes(&mut content, name)?;
        model::replace_subtree(&mut tree, name, &content);
        manifest.packages.insert(name.into());
        let names = package_names(&manifest, &self.lock()?);
        model::root_index(&mut tree, &names)?;
        self.apply(vec![
            self.change("gnosis.toml", Entry::File(model::encode(&manifest)?))?,
            Change {
                path: "gnosis",
                before: Some(Entry::Directory(original)),
                after: Entry::Directory(tree),
            },
        ])?;
        println!("Created {name}; publish it through this repository's normal Git review workflow");
        Ok(())
    }

    pub fn source(&self, name: &str, repository: String, reference: String) -> Result<()> {
        model::name(name)?;
        let mut manifest = self.manifest()?;
        let repository = if !repository.contains(':') {
            git::path(
                &fs::canonicalize(self.root.join(repository))
                    .context("local source repository does not exist")?,
            )?
            .to_owned()
        } else {
            repository
        };
        let source = Source {
            repository,
            reference,
        };
        model::validate_source(&source)?;
        manifest.sources.insert(name.into(), source);
        self.apply(vec![
            self.change("gnosis.toml", Entry::File(model::encode(&manifest)?))?,
        ])?;
        println!("Configured source {name}");
        Ok(())
    }

    pub fn list(&self) -> Result<()> {
        let manifest = self.manifest()?;
        let mut store = Store::new()?;
        let catalog = catalog(&manifest, &mut store)?;
        for (name, candidates) in catalog {
            for candidate in candidates {
                let content = store.snapshot(
                    &candidate.location.repository,
                    &candidate.commit,
                    &format!("gnosis/{name}"),
                )?;
                let package = model::package(&content, &name)?;
                println!(
                    "{}/{}\t{}\t{}",
                    candidate.source,
                    name,
                    package.owner,
                    package.description.replace(['\n', '\r'], " ")
                );
            }
        }
        Ok(())
    }

    pub fn add(&self, name: &str, source: &str) -> Result<()> {
        model::name(name)?;
        let mut manifest = self.manifest()?;
        ensure!(
            !manifest.packages.contains(name),
            "{name} is locally authored"
        );
        ensure!(
            manifest.sources.contains_key(source),
            "unknown source {source}"
        );
        manifest.dependencies.insert(name.into(), source.into());
        self.synchronize(manifest, false)
    }

    pub fn sync(&self, update: bool) -> Result<()> {
        self.synchronize(self.manifest()?, update)
    }

    fn synchronize(&self, manifest: Manifest, update: bool) -> Result<()> {
        model::validate_manifest(&manifest)?;
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
        let mut store = Store::new()?;
        let locked = if !update
            && old.requirements == manifest.dependencies
            && old.sources == manifest.sources
        {
            old.clone()
        } else {
            resolve(&manifest, &old, update, &mut store)?
        };
        for (name, entry) in &locked.packages {
            ensure!(
                !manifest.packages.contains(name),
                "dependency {name} collides with a locally authored package"
            );
            let mut upstream = snapshot(&mut store, name, entry)?;
            model::indexes(&mut upstream, name)?;
            let local = model::subtree(&original, name);
            let content = if let Some(previous) = old.packages.get(name) {
                if local.is_empty() {
                    upstream
                } else {
                    let mut base = snapshot(&mut store, name, previous)?;
                    model::indexes(&mut base, name)?;
                    git::merge(
                        &model::without_generated_indexes(&base),
                        &model::without_generated_indexes(&local),
                        &model::without_generated_indexes(&upstream),
                        name,
                    )?
                }
            } else {
                ensure!(
                    local.is_empty(),
                    "refusing to overwrite unmanaged package directory {name}"
                );
                upstream
            };
            let mut content = content;
            model::package(&content, name)?;
            model::indexes(&mut content, name)?;
            model::replace_subtree(&mut tree, name, &content);
        }
        for (name, previous) in &old.packages {
            if locked.packages.contains_key(name) {
                continue;
            }
            let local = model::subtree(&original, name);
            let mut base = snapshot(&mut store, name, previous)?;
            model::indexes(&mut base, name)?;
            ensure!(
                local.is_empty() || local == base,
                "unused package {name} has local changes; preserve them outside gnosis/{name} before removing the dependency"
            );
            model::replace_subtree(&mut tree, name, &Tree::new());
        }
        validate_workspace(&manifest, &locked, &tree)?;
        model::root_index(&mut tree, &package_names(&manifest, &locked))?;
        self.apply(vec![
            Change {
                path: "gnosis",
                before: knowledge_before,
                after: Entry::Directory(tree),
            },
            Change {
                path: "gnosis.lock",
                before: lock_before,
                after: Entry::File(model::encode(&locked)?),
            },
            Change {
                path: "gnosis.toml",
                before: manifest_before,
                after: Entry::File(model::encode(&manifest)?),
            },
        ])?;
        println!(
            "Synced {} imported packages; review the workspace diff with Git",
            locked.packages.len()
        );
        Ok(())
    }

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
            let mut content = model::subtree(&tree, &name);
            model::package(&content, &name)?;
            model::indexes(&mut content, &name)?;
            model::replace_subtree(&mut tree, &name, &content);
        }
        model::root_index(&mut tree, &package_names(&manifest, &lock))?;
        self.apply(vec![Change {
            path: "gnosis",
            before: Some(Entry::Directory(original)),
            after: Entry::Directory(tree),
        }])?;
        println!("Updated generated indexes; hand-authored indexes were preserved");
        Ok(())
    }

    pub fn propose(&self, name: &str, output: &Path) -> Result<()> {
        model::name(name)?;
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
        let local = model::subtree(&original, name);
        model::package(&local, name)?;
        model::validate_okf(&local)?;
        let mut store = Store::new()?;
        let base = snapshot(&mut store, name, pinned)?;
        let base = model::without_generated_indexes(&base);
        let local = model::without_generated_indexes(&local);
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
            &model::without_generated_indexes(&upstream),
            name,
        )?;
        model::package(&merged, name)?;
        model::indexes(&mut merged, name)?;
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

fn package_names(manifest: &Manifest, lock: &Lock) -> BTreeSet<String> {
    manifest
        .packages
        .iter()
        .chain(lock.packages.keys())
        .cloned()
        .collect()
}

fn validate_workspace(manifest: &Manifest, lock: &Lock, tree: &Tree) -> Result<()> {
    let names = package_names(manifest, lock);
    for name in &names {
        ensure!(
            !(manifest.packages.contains(name) && lock.packages.contains_key(name)),
            "{name} is both local and imported"
        );
        let content = model::subtree(tree, name);
        let package = model::package(&content, name).with_context(|| format!("package {name}"))?;
        model::validate_okf(&content).with_context(|| format!("package {name}"))?;
        for dependency in &package.dependencies {
            ensure!(
                names.contains(dependency),
                "{name} needs missing package {dependency}; add it to the workspace"
            );
        }
    }
    Ok(())
}

fn snapshot_content(store: &mut Store, name: &str, entry: &LockedPackage) -> Result<Tree> {
    let tree = store.snapshot(
        &entry.location.repository,
        &entry.commit,
        &format!("gnosis/{name}"),
    )?;
    model::package(&tree, name)?;
    model::validate_okf(&tree)?;
    Ok(tree)
}

fn snapshot(store: &mut Store, name: &str, entry: &LockedPackage) -> Result<Tree> {
    let tree = snapshot_content(store, name, entry)?;
    ensure!(
        model::package(&tree, name)?.dependencies == entry.dependencies,
        "locked dependencies disagree with source snapshot for {name}"
    );
    Ok(tree)
}

fn catalog(manifest: &Manifest, store: &mut Store) -> Result<BTreeMap<String, Vec<LockedPackage>>> {
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
        let published: Manifest = model::decode(&bytes)?;
        model::validate_manifest(&published)?;
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

fn resolve(manifest: &Manifest, old: &Lock, update: bool, store: &mut Store) -> Result<Lock> {
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
        selected.dependencies = model::package(&content, &name)?.dependencies;
        pending.extend(selected.dependencies.iter().cloned());
        lock.packages.insert(name, selected);
    }
    Ok(lock)
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
