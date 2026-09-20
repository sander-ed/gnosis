use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use anyhow::{Context, Result, bail, ensure};
use tempfile::TempDir;

use crate::model::{self, MAX_FILE, MAX_TREE, Source, Tree};

fn command(directory: &Path, arguments: &[&str], isolated: bool) -> Result<Output> {
    let mut command = Command::new("git");
    if directory.join("HEAD").is_file() && directory.join("objects").is_dir() {
        command.arg("--git-dir").arg(directory);
    }
    command
        .arg("-C")
        .arg(directory)
        .args([
            "-c",
            "core.hooksPath=/dev/null",
            "-c",
            "core.fsmonitor=false",
        ])
        .args([
            "-c",
            "protocol.ext.allow=never",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(arguments)
        .env("GIT_TERMINAL_PROMPT", "0");
    for variable in [
        "GIT_DIR",
        "GIT_WORK_TREE",
        "GIT_INDEX_FILE",
        "GIT_OBJECT_DIRECTORY",
        "GIT_ALTERNATE_OBJECT_DIRECTORIES",
    ] {
        command.env_remove(variable);
    }
    if isolated {
        command
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_AUTHOR_NAME", "Gnosis local merge")
            .env("GIT_AUTHOR_EMAIL", "gnosis@localhost")
            .env("GIT_COMMITTER_NAME", "Gnosis local merge")
            .env("GIT_COMMITTER_EMAIL", "gnosis@localhost");
    }
    command.output().context("could not run Git")
}

pub fn run(directory: &Path, arguments: &[&str], isolated: bool) -> Result<Vec<u8>> {
    let output = command(directory, arguments, isolated)?;
    ensure!(
        output.status.success(),
        "Git {} failed:\n{}{}",
        arguments.first().unwrap_or(&""),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(output.stdout)
}

pub fn path(path: &Path) -> Result<&str> {
    path.to_str().context("non-UTF-8 filesystem path")
}

pub struct Store {
    directory: TempDir,
    repositories: BTreeMap<String, PathBuf>,
}

impl Store {
    pub fn new() -> Result<Self> {
        Ok(Self {
            directory: tempfile::tempdir()?,
            repositories: BTreeMap::new(),
        })
    }

    pub fn repository(&mut self, url: &str) -> Result<PathBuf> {
        if let Some(repository) = self.repositories.get(url) {
            return Ok(repository.clone());
        }
        let destination = self
            .directory
            .path()
            .join(self.repositories.len().to_string());
        run(
            self.directory.path(),
            &[
                "clone",
                "--quiet",
                "--bare",
                "--no-hardlinks",
                "--",
                url,
                path(&destination)?,
            ],
            false,
        )?;
        self.repositories
            .insert(url.to_owned(), destination.clone());
        Ok(destination)
    }

    pub fn revision(&mut self, source: &Source) -> Result<String> {
        model::validate_source(source)?;
        let repository = self.repository(&source.repository)?;
        let revision = format!("{}^{{commit}}", source.reference);
        let bytes = run(
            &repository,
            &["rev-parse", "--verify", "--end-of-options", &revision],
            true,
        )?;
        Ok(String::from_utf8(bytes)?.trim().to_owned())
    }

    pub fn snapshot(&mut self, url: &str, commit: &str, directory: &str) -> Result<Tree> {
        ensure!(
            matches!(commit.len(), 40 | 64) && commit.bytes().all(|c| c.is_ascii_hexdigit()),
            "invalid locked Git commit {commit:?}"
        );
        let repository = self.repository(url)?;
        if !command(
            &repository,
            &["cat-file", "-e", &format!("{commit}^{{commit}}")],
            true,
        )?
        .status
        .success()
        {
            run(
                &repository,
                &["fetch", "--quiet", "--no-tags", "--", url, commit],
                false,
            )?;
        }
        tree_at(&repository, commit, directory)
    }
}

pub fn tree_at(repository: &Path, revision: &str, directory: &str) -> Result<Tree> {
    let object = if directory.is_empty() {
        revision.to_owned()
    } else {
        format!("{revision}:{directory}")
    };
    let listing = run(repository, &["ls-tree", "-r", "-z", &object], true)?;
    let mut result = Tree::new();
    let mut total = 0;
    for entry in listing
        .split(|byte| *byte == 0)
        .filter(|entry| !entry.is_empty())
    {
        let entry = std::str::from_utf8(entry)?;
        let (metadata, name) = entry.split_once('\t').context("invalid Git tree entry")?;
        model::safe_path(name)?;
        let fields: Vec<_> = metadata.split(' ').collect();
        ensure!(
            fields.len() == 3 && fields[0] == "100644" && fields[1] == "blob",
            "only non-executable regular package files are supported: {name}"
        );
        let size = run(repository, &["cat-file", "-s", fields[2]], true)?;
        let size: usize = std::str::from_utf8(&size)?.trim().parse()?;
        ensure!(size <= MAX_FILE, "{name} exceeds 16 MiB");
        total += size;
        ensure!(total <= MAX_TREE, "package exceeds 256 MiB");
        result.insert(
            name.to_owned(),
            run(repository, &["cat-file", "blob", fields[2]], true)?,
        );
    }
    Ok(result)
}

fn commit_snapshot(repository: &Path, tree: &Tree, message: &str) -> Result<()> {
    for entry in fs::read_dir(repository)? {
        let entry = entry?;
        if entry.file_name() == ".git" {
            continue;
        }
        if entry.file_type()?.is_dir() {
            fs::remove_dir_all(entry.path())?;
        } else {
            fs::remove_file(entry.path())?;
        }
    }
    model::write_tree(repository, tree)?;
    run(repository, &["add", "--all", "--force"], true)?;
    run(
        repository,
        &["commit", "--quiet", "--allow-empty", "-m", message],
        true,
    )?;
    Ok(())
}

pub fn merge(base: &Tree, local: &Tree, upstream: &Tree, package: &str) -> Result<Tree> {
    if base == local || local == upstream {
        return Ok(upstream.clone());
    }
    if base == upstream {
        return Ok(local.clone());
    }
    let temporary = tempfile::tempdir()?;
    let directory = temporary.path();
    run(
        directory,
        &["init", "--quiet", "--initial-branch=local"],
        true,
    )?;
    commit_snapshot(directory, base, "base")?;
    run(directory, &["branch", "upstream"], true)?;
    commit_snapshot(directory, local, "local")?;
    run(directory, &["checkout", "--quiet", "upstream"], true)?;
    commit_snapshot(directory, upstream, "upstream")?;
    let output = command(
        directory,
        &["merge-tree", "--write-tree", "local", "upstream"],
        true,
    )?;
    if !output.status.success() {
        bail!(
            "cannot merge package {package}; workspace and lock are unchanged.\n\
             Reconcile the reported files against upstream, then retry.\n{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let output = String::from_utf8(output.stdout)?;
    let tree = output
        .lines()
        .next()
        .context("Git did not return a merged tree")?;
    tree_at(directory, tree, "")
}

pub fn prepare(
    store: &mut Store,
    source: &Source,
    revision: &str,
    package: &str,
    content: &Tree,
    output: &Path,
) -> Result<()> {
    ensure!(
        !output.try_exists()?,
        "proposal output already exists: {}",
        output.display()
    );
    let repository = store.repository(&source.repository)?;
    let parent = output.parent().context("proposal output needs a parent")?;
    fs::create_dir_all(parent)?;
    run(
        parent,
        &[
            "clone",
            "--quiet",
            "--no-checkout",
            "--no-hardlinks",
            "--",
            path(&repository)?,
            path(output)?,
        ],
        true,
    )?;
    let result = || -> Result<()> {
        run(
            output,
            &["remote", "set-url", "origin", &source.repository],
            true,
        )?;
        run(
            output,
            &[
                "checkout",
                "--quiet",
                "-b",
                &format!("gnosis/{package}"),
                revision,
            ],
            true,
        )?;
        let destination = output.join("gnosis").join(package);
        let metadata = fs::symlink_metadata(&destination)?;
        ensure!(
            metadata.is_dir() && !metadata.file_type().is_symlink(),
            "source package is not a regular directory"
        );
        fs::remove_dir_all(&destination)?;
        model::write_tree(&destination, content)?;
        run(
            output,
            &["add", "--force", "--", &format!("gnosis/{package}")],
            true,
        )?;
        Ok(())
    };
    result().with_context(|| {
        format!(
            "proposal preparation stopped; inspect retained checkout {}",
            output.display()
        )
    })
}
