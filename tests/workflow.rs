use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use tempfile::TempDir;

fn execute(command: &mut Command) -> Output {
    command.output().expect("run command")
}

fn success(output: Output) -> String {
    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

fn git(root: &Path, args: &[&str]) -> String {
    success(execute(
        Command::new("git")
            .arg("-C")
            .arg(root)
            .args([
                "-c",
                "core.hooksPath=/dev/null",
                "-c",
                "commit.gpgsign=false",
            ])
            .args(args)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_AUTHOR_NAME", "Test Owner")
            .env("GIT_AUTHOR_EMAIL", "owner@example.test")
            .env("GIT_COMMITTER_NAME", "Test Owner")
            .env("GIT_COMMITTER_EMAIL", "owner@example.test"),
    ))
}

fn cli_output(root: &Path, args: &[&str]) -> Output {
    execute(
        Command::new(env!("CARGO_BIN_EXE_gnosis"))
            .arg("-C")
            .arg(root)
            .args(args),
    )
}

fn cli(root: &Path, args: &[&str]) -> String {
    success(cli_output(root, args))
}

fn fails(root: &Path, args: &[&str], expected: &str) {
    let output = cli_output(root, args);
    assert!(!output.status.success(), "unexpected success");
    let error = String::from_utf8(output.stderr).unwrap();
    assert!(
        error.contains(expected),
        "expected {expected:?}, got {error}"
    );
}

fn write(root: &Path, name: &str, content: impl AsRef<[u8]>) {
    let path = root.join(name);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}

fn read(root: &Path, name: &str) -> String {
    fs::read_to_string(root.join(name)).unwrap()
}

fn concept(body: &str) -> String {
    format!("---\ntype: Reference\ncustom_field: preserved\n---\n# Knowledge\n\n{body}\n")
}

fn commit(root: &Path) -> String {
    git(root, &["add", "--all"]);
    git(
        root,
        &[
            "commit",
            "--quiet",
            "--allow-empty",
            "-m",
            "Owner-reviewed knowledge",
        ],
    );
    git(root, &["rev-parse", "HEAD"]).trim().to_owned()
}

fn snapshot(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    fn visit(root: &Path, current: &Path, files: &mut BTreeMap<PathBuf, Vec<u8>>) {
        for entry in fs::read_dir(current).unwrap() {
            let entry = entry.unwrap();
            if entry.file_name() == ".gnosis" || entry.file_name() == ".git" {
                continue;
            }
            if entry.file_type().unwrap().is_dir() {
                visit(root, &entry.path(), files);
            } else {
                files.insert(
                    entry.path().strip_prefix(root).unwrap().to_owned(),
                    fs::read(entry.path()).unwrap(),
                );
            }
        }
    }
    let mut result = BTreeMap::new();
    visit(root, root, &mut result);
    result
}

struct Fixture {
    temporary: TempDir,
    source: PathBuf,
    consumer: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let temporary = tempfile::tempdir().unwrap();
        let source = temporary.path().join("source");
        let consumer = temporary.path().join("consumer");
        fs::create_dir(&source).unwrap();
        fs::create_dir(&consumer).unwrap();
        let source = fs::canonicalize(source).unwrap();
        let consumer = fs::canonicalize(consumer).unwrap();
        git(&source, &["init", "--quiet", "--initial-branch=main"]);
        git(&consumer, &["init", "--quiet", "--initial-branch=main"]);
        cli(&source, &["init"]);
        cli(
            &source,
            &[
                "package",
                "core",
                "--owner",
                "@org/core",
                "--description",
                "Shared knowledge",
            ],
        );
        cli(
            &source,
            &["package", "platform", "--owner", "@org/platform"],
        );
        write(
            &source,
            "gnosis/core/facts.md",
            concept("Original core knowledge."),
        );
        write(
            &source,
            "gnosis/platform/facts.md",
            concept("Original platform knowledge."),
        );
        write(
            &source,
            "gnosis/platform/package.toml",
            "name = 'platform'\nowner = '@org/platform'\ndependencies = ['core']\n",
        );
        cli(&source, &["index"]);
        commit(&source);
        cli(&consumer, &["init"]);
        cli(
            &consumer,
            &["source", "team", source.to_str().unwrap(), "--ref", "main"],
        );
        Self {
            temporary,
            source,
            consumer,
        }
    }

    fn install(&self) {
        cli(&self.consumer, &["add", "platform", "--source", "team"]);
    }

    fn other(&self, name: &str) -> PathBuf {
        let path = self.temporary.path().join(name);
        fs::create_dir(&path).unwrap();
        path
    }
}

#[test]
fn collaborative_round_trip_preserves_edits_and_isolates_proposals() {
    let fixture = Fixture::new();
    let source = &fixture.source;
    let consumer = &fixture.consumer;
    let listed = cli(consumer, &["list"]);
    assert!(listed.contains("team/core"));
    assert!(listed.contains("team/platform"));
    fixture.install();
    cli(consumer, &["check"]);
    let pinned = git(source, &["rev-parse", "HEAD"]);
    assert!(read(consumer, "gnosis.lock").contains(pinned.trim()));
    let head = commit(consumer);

    let restored = fixture.other("restored");
    for name in ["gnosis.toml", "gnosis.lock"] {
        fs::copy(consumer.join(name), restored.join(name)).unwrap();
    }
    cli(&restored, &["sync"]);
    assert_eq!(snapshot(consumer), snapshot(&restored));

    write(
        consumer,
        "gnosis/platform/local.md",
        concept("A locally discovered fact."),
    );
    write(
        consumer,
        "gnosis/core/private.md",
        concept("PRIVATE consumer-only observation."),
    );
    write(
        consumer,
        "application-secret.txt",
        "NEVER contribute this file",
    );
    write(
        source,
        "gnosis/platform/upstream.md",
        concept("An upstream finding."),
    );
    write(
        source,
        "gnosis/core/facts.md",
        concept("Updated core knowledge."),
    );
    write(source, "unrelated.txt", "Source-only data");
    let newer = commit(source);

    cli(consumer, &["sync"]);
    assert!(!consumer.join("gnosis/platform/upstream.md").exists());
    assert!(read(consumer, "gnosis.lock").contains(pinned.trim()));
    cli(consumer, &["sync", "--update"]);
    assert!(consumer.join("gnosis/platform/local.md").exists());
    assert!(consumer.join("gnosis/platform/upstream.md").exists());
    assert!(read(consumer, "gnosis.lock").contains(&newer));
    assert_eq!(git(consumer, &["rev-parse", "HEAD"]).trim(), head);
    assert!(git(consumer, &["diff", "--name-only"]).contains("gnosis/core/facts.md"));
    assert!(git(consumer, &["diff", "--cached", "--name-only"]).is_empty());

    let proposal = fixture.temporary.path().join("proposal");
    cli(
        consumer,
        &[
            "propose",
            "platform",
            "--output",
            proposal.to_str().unwrap(),
        ],
    );
    let staged = git(&proposal, &["diff", "--cached", "--name-only"]);
    assert!(!staged.is_empty());
    assert!(
        staged
            .lines()
            .all(|path| path.starts_with("gnosis/platform/"))
    );
    assert!(!proposal.join("application-secret.txt").exists());
    assert!(!proposal.join("gnosis/core/private.md").exists());
    assert_eq!(git(&proposal, &["rev-parse", "HEAD"]).trim(), newer);
    assert_eq!(git(source, &["rev-parse", "HEAD"]).trim(), newer);
    assert_eq!(
        git(&proposal, &["remote", "get-url", "origin"]).trim(),
        source.to_str().unwrap()
    );
    git(
        &proposal,
        &["commit", "--quiet", "-m", "Approved contribution"],
    );
    git(
        source,
        &[
            "fetch",
            "--quiet",
            proposal.to_str().unwrap(),
            "gnosis/platform",
        ],
    );
    git(source, &["merge", "--quiet", "--ff-only", "FETCH_HEAD"]);
    cli(consumer, &["sync", "--update"]);
    cli(consumer, &["check"]);
    fails(
        consumer,
        &[
            "propose",
            "platform",
            "--output",
            fixture.temporary.path().join("empty").to_str().unwrap(),
        ],
        "no local changes",
    );
    assert!(read(consumer, "gnosis/core/private.md").contains("PRIVATE"));
    assert_eq!(
        read(consumer, "gnosis/platform/local.md")
            .matches("locally discovered")
            .count(),
        1
    );
}

#[test]
fn conflicts_leave_every_package_and_metadata_unchanged() {
    let fixture = Fixture::new();
    fixture.install();
    write(
        &fixture.consumer,
        "gnosis/platform/facts.md",
        concept("Local replacement."),
    );
    write(
        &fixture.source,
        "gnosis/platform/facts.md",
        concept("Different upstream replacement."),
    );
    write(
        &fixture.source,
        "gnosis/core/new.md",
        concept("Should not be partially installed."),
    );
    commit(&fixture.source);
    let before = snapshot(&fixture.consumer);
    fails(
        &fixture.consumer,
        &["sync", "--update"],
        "cannot merge package platform",
    );
    assert_eq!(before, snapshot(&fixture.consumer));
    write(
        &fixture.consumer,
        "gnosis/platform/facts.md",
        concept("Different upstream replacement."),
    );
    cli(&fixture.consumer, &["sync", "--update"]);
    assert!(fixture.consumer.join("gnosis/core/new.md").exists());
}

#[test]
fn multiple_sources_require_explicit_choice_for_ambiguous_dependencies() {
    let fixture = Fixture::new();
    let second = fixture.other("public");
    git(&second, &["init", "--quiet", "--initial-branch=main"]);
    cli(&second, &["init"]);
    cli(&second, &["package", "core", "--owner", "@public"]);
    write(&second, "gnosis/core/facts.md", concept("Public core."));
    commit(&second);
    cli(
        &fixture.consumer,
        &[
            "source",
            "public",
            second.to_str().unwrap(),
            "--ref",
            "main",
        ],
    );
    let before = snapshot(&fixture.consumer);
    fails(
        &fixture.consumer,
        &["add", "platform", "--source", "team"],
        "ambiguous dependency core",
    );
    assert_eq!(before, snapshot(&fixture.consumer));
    cli(&fixture.consumer, &["add", "core", "--source", "public"]);
    fixture.install();
    assert!(read(&fixture.consumer, "gnosis/core/facts.md").contains("Public core"));
    cli(&fixture.consumer, &["sync", "--update"]);
    assert!(read(&fixture.consumer, "gnosis/core/facts.md").contains("Public core"));
}

#[test]
fn only_explicitly_published_packages_are_discovered() {
    let fixture = Fixture::new();
    write(
        &fixture.source,
        "gnosis/unpublished/package.toml",
        "name = 'unpublished'\nowner = '@owner'\n",
    );
    commit(&fixture.source);
    let listing = cli(&fixture.consumer, &["list"]);
    assert!(!listing.contains("unpublished"));
    let before = snapshot(&fixture.consumer);
    fails(
        &fixture.consumer,
        &["add", "unpublished", "--source", "team"],
        "no configured source publishes",
    );
    assert_eq!(before, snapshot(&fixture.consumer));
}

#[test]
fn locked_restore_does_not_require_the_original_branch() {
    let fixture = Fixture::new();
    fixture.install();
    git(&fixture.source, &["tag", "retained-release"]);
    git(&fixture.source, &["branch", "-m", "renamed"]);
    let restored = fixture.other("restored");
    for name in ["gnosis.toml", "gnosis.lock"] {
        fs::copy(fixture.consumer.join(name), restored.join(name)).unwrap();
    }
    cli(&restored, &["sync"]);
    assert_eq!(snapshot(&fixture.consumer), snapshot(&restored));
    let before = snapshot(&restored);
    fails(&restored, &["sync", "--update"], "source team");
    assert_eq!(before, snapshot(&restored));
}

#[test]
fn removing_a_dependency_never_discards_local_knowledge() {
    let fixture = Fixture::new();
    fixture.install();
    write(
        &fixture.consumer,
        "gnosis/core/local.md",
        concept("Unpublished work."),
    );
    write(
        &fixture.source,
        "gnosis/platform/package.toml",
        "name = 'platform'\nowner = '@org/platform'\n",
    );
    commit(&fixture.source);
    let before = snapshot(&fixture.consumer);
    fails(
        &fixture.consumer,
        &["sync", "--update"],
        "unused package core has local changes",
    );
    assert_eq!(before, snapshot(&fixture.consumer));
    fs::rename(
        fixture.consumer.join("gnosis/core/local.md"),
        fixture.consumer.join("saved.md"),
    )
    .unwrap();
    cli(&fixture.consumer, &["sync", "--update"]);
    assert!(!fixture.consumer.join("gnosis/core").exists());
    assert!(fixture.consumer.join("saved.md").exists());
}

#[test]
fn indexes_preserve_concepts_extensions_and_hand_authored_indexes() {
    let fixture = Fixture::new();
    fixture.install();
    let document = "---\r\ntype: My Custom Type\r\ntitle: '[name](evil)'\r\ndescription: '<script>no</script>'\r\ncustom: {a: 3}\r\n---\r\nCustom body.\r\n";
    write(
        &fixture.consumer,
        "gnosis/platform/group/a file.md",
        document,
    );
    write(
        &fixture.consumer,
        "gnosis/platform/manual/index.md",
        "# A human index\n",
    );
    write(
        &fixture.consumer,
        "gnosis/platform/manual/fact.md",
        concept("Other."),
    );
    write(
        &fixture.consumer,
        "gnosis/platform/schema.json",
        "{\"custom\":true}",
    );
    cli(&fixture.consumer, &["index"]);
    let index = read(&fixture.consumer, "gnosis/platform/group/index.md");
    assert!(index.contains("a%20file.md"));
    assert!(index.contains("\\[name\\]"));
    assert!(index.contains("&lt;script&gt;"));
    assert_eq!(
        read(&fixture.consumer, "gnosis/platform/group/a file.md"),
        document
    );
    assert_eq!(
        read(&fixture.consumer, "gnosis/platform/manual/index.md"),
        "# A human index\n"
    );
    assert_eq!(
        read(&fixture.consumer, "gnosis/platform/schema.json"),
        "{\"custom\":true}"
    );
    assert!(read(&fixture.consumer, "gnosis/platform/index.md").contains("group/index.md"));
    cli(&fixture.consumer, &["check"]);
    let before = snapshot(&fixture.consumer);
    cli(&fixture.consumer, &["index"]);
    assert_eq!(before, snapshot(&fixture.consumer));
}

#[test]
fn invalid_okf_is_rejected_before_installing_anything() {
    let fixture = Fixture::new();
    write(
        &fixture.source,
        "gnosis/platform/facts.md",
        "---\ntitle: No type\n---\nBad concept.\n",
    );
    commit(&fixture.source);
    let before = snapshot(&fixture.consumer);
    fails(
        &fixture.consumer,
        &["add", "platform", "--source", "team"],
        "nonempty string type",
    );
    assert_eq!(before, snapshot(&fixture.consumer));
}

#[test]
fn cyclic_knowledge_dependencies_are_a_finite_closure() {
    let fixture = Fixture::new();
    write(
        &fixture.source,
        "gnosis/core/package.toml",
        "name = 'core'\nowner = '@core'\ndependencies = ['platform']\n",
    );
    commit(&fixture.source);
    fixture.install();
    cli(&fixture.consumer, &["check"]);
    assert!(fixture.consumer.join("gnosis/core/facts.md").exists());
}

#[cfg(unix)]
#[test]
fn source_symlinks_are_rejected_without_touching_the_consumer() {
    use std::os::unix::fs::symlink;
    let fixture = Fixture::new();
    symlink(
        "../../outside.md",
        fixture.source.join("gnosis/platform/link.md"),
    )
    .unwrap();
    commit(&fixture.source);
    let before = snapshot(&fixture.consumer);
    fails(
        &fixture.consumer,
        &["add", "platform", "--source", "team"],
        "regular package files",
    );
    assert_eq!(before, snapshot(&fixture.consumer));
}

#[test]
fn interrupted_transactions_and_concurrent_operations_are_explicit_errors() {
    use fs2::FileExt;
    let fixture = Fixture::new();
    let lock = fs::File::options()
        .read(true)
        .write(true)
        .open(fixture.consumer.join(".gnosis/operation.lock"))
        .unwrap();
    lock.try_lock_exclusive().unwrap();
    fails(&fixture.consumer, &["sync"], "another gnosis operation");
    FileExt::unlock(&lock).unwrap();
    fs::create_dir(fixture.consumer.join(".gnosis/transaction-interrupted")).unwrap();
    fails(&fixture.consumer, &["sync"], "interrupted transaction");
}
