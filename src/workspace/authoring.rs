use std::collections::BTreeSet;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail, ensure};

use super::transaction::{Change, Entry};
use super::{Workspace, package_names};
use crate::metadata::{Lock, Manifest, Package};
use crate::tree::Tree;
use crate::{git, metadata, navigation, okf, tree};

pub struct NewEntry {
    pub directory: bool,
    pub metadata: okf::ConceptMetadata,
    pub body: Option<String>,
    pub body_file: Option<PathBuf>,
}

fn read_body(reader: impl Read) -> Result<String> {
    let mut bytes = Vec::new();
    reader
        .take(tree::MAX_FILE as u64 + 1)
        .read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() <= tree::MAX_FILE,
        "Markdown body exceeds 16 MiB"
    );
    String::from_utf8(bytes).context("Markdown body must be UTF-8")
}

impl Workspace {
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
        navigation::root_index(&mut tree, &BTreeSet::new())?;
        self.apply(vec![
            self.change("gnosis.toml", Entry::File(metadata::encode(&manifest)?))?,
            self.change("gnosis.lock", Entry::File(metadata::encode(&lock)?))?,
            self.change("gnosis", Entry::Directory(tree))?,
        ])?;
        println!("Initialized {}", self.root.display());
        Ok(())
    }

    pub fn package(&self, name: &str, owner: String, description: String) -> Result<()> {
        metadata::name(name)?;
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
        let mut content = Tree::from([("package.toml".into(), metadata::encode(&package)?)]);
        navigation::indexes(&mut content, name)?;
        tree::replace_subtree(&mut tree, name, &content);
        manifest.packages.insert(name.into());
        let names = package_names(&manifest, &self.lock()?);
        navigation::root_index(&mut tree, &names)?;
        self.apply(vec![
            self.change("gnosis.toml", Entry::File(metadata::encode(&manifest)?))?,
            Change {
                path: "gnosis",
                before: Some(Entry::Directory(original)),
                after: Entry::Directory(tree),
            },
        ])?;
        println!("Created {name}; publish it through this repository's normal Git review workflow");
        Ok(())
    }

    pub fn new_entry(&self, name: &str, path: &str, options: NewEntry) -> Result<()> {
        metadata::name(name)?;
        let manifest = self.manifest()?;
        let lock = self.lock()?;
        let names = package_names(&manifest, &lock);
        ensure!(
            names.contains(name),
            "unknown package {name}; create it with gnosis package or install it with gnosis add"
        );
        let directory = options.directory || path.ends_with('/');
        let mut relative = if directory {
            path.trim_end_matches('/').to_owned()
        } else {
            path.to_owned()
        };
        tree::safe_path(&relative)?;
        if directory {
            ensure!(
                options.body.is_none() && options.body_file.is_none(),
                "directory bodies are generated from metadata; use --description or create a .md file inside the directory"
            );
            ensure!(
                !relative.ends_with(".md"),
                "a navigation directory must not end in .md"
            );
        } else {
            match Path::new(&relative)
                .extension()
                .and_then(|extension| extension.to_str())
            {
                None => relative.push_str(".md"),
                Some("md") => {}
                _ => bail!("concept files must use the .md extension"),
            }
            let filename = relative.rsplit('/').next().context("missing filename")?;
            ensure!(
                !filename.eq_ignore_ascii_case("index.md")
                    && !filename.eq_ignore_ascii_case("log.md"),
                "index.md and log.md are reserved; use --dir for navigation or choose a concept name"
            );
        }
        let original = self.knowledge()?;
        let mut content = tree::subtree(&original, name);
        metadata::package(&content, name)?;
        let destination = self.root.join("gnosis").join(name).join(&relative);
        ensure!(
            !destination.try_exists()?,
            "{} already exists; edit it directly or choose a different path",
            destination.display()
        );
        let created = if directory {
            content.insert(
                format!("{relative}/index.yml"),
                options.metadata.render_directory(&relative)?,
            );
            format!("{relative}/index.md")
        } else {
            relative
        };
        let mut manual_indexes = Vec::new();
        let mut parent = Path::new(&created).parent();
        while let Some(directory) = parent {
            let index = directory.join("index.md");
            let index = git::path(&index)?.replace(std::path::MAIN_SEPARATOR, "/");
            if content
                .get(&index)
                .is_some_and(|bytes| !navigation::is_generated_index(bytes))
            {
                manual_indexes.push(index);
            }
            parent = directory.parent();
        }
        let bytes = if directory {
            format!("{}\n", navigation::MARKER).into_bytes()
        } else {
            let body = match options.body_file {
                Some(path) if path == Path::new("-") => Some(read_body(std::io::stdin().lock())?),
                Some(path) => Some(read_body(
                    File::open(self.root.join(&path))
                        .with_context(|| format!("cannot read body file {}", path.display()))?,
                )?),
                None => options.body,
            };
            options.metadata.render(&created, body.as_deref())?
        };
        content.insert(created.clone(), bytes);
        navigation::indexes(&mut content, name)?;
        let mut tree = original.clone();
        tree::replace_subtree(&mut tree, name, &content);
        navigation::root_index(&mut tree, &names)?;
        ensure!(
            tree.values().all(|bytes| bytes.len() <= tree::MAX_FILE),
            "generated file exceeds 16 MiB"
        );
        ensure!(
            tree.values().map(Vec::len).sum::<usize>() <= tree::MAX_TREE,
            "knowledge directory exceeds 256 MiB"
        );
        self.apply(vec![Change {
            path: "gnosis",
            before: Some(Entry::Directory(original)),
            after: Entry::Directory(tree),
        }])?;
        println!("Created gnosis/{name}/{created}");
        for index in manual_indexes {
            eprintln!(
                "note: preserved hand-authored gnosis/{name}/{index}; add a navigation link there if needed"
            );
        }
        Ok(())
    }
}
