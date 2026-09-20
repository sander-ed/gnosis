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

fn authoring_workspace() -> TempDir {
    let directory = tempfile::tempdir().unwrap();
    git(
        directory.path(),
        &["init", "--quiet", "--initial-branch=main"],
    );
    cli(directory.path(), &["init"]);
    cli(
        directory.path(),
        &["package", "notes", "--owner", "@owner", "-d", "Local notes"],
    );
    directory
}

fn document(root: &Path, path: &str) -> (serde_yaml_ng::Value, String) {
    let text = read(root, path);
    let (metadata, body) = text
        .strip_prefix("---\n")
        .unwrap()
        .split_once("\n---\n")
        .unwrap();
    (serde_yaml_ng::from_str(metadata).unwrap(), body.to_owned())
}

#[test]
fn new_passes_okf_metadata_and_refreshes_nested_navigation_without_committing() {
    let directory = authoring_workspace();
    let root = directory.path();
    let head = commit(root);
    let manifest = read(root, "gnosis.toml");
    let lock = read(root, "gnosis.lock");
    let description = "Migration: preparation\nThen cutover.";
    let output = cli(
        root,
        &[
            "new",
            "notes",
            "plans/cutover.md",
            "--type",
            "Custom Playbook",
            "--title",
            "Cutover: \"production\"",
            "--description",
            description,
            "--resource",
            "https://example.test/service",
            "--tag",
            "migration",
            "--tag",
            "production",
            "--source",
            "https://example.test/runbook",
            "--source",
            "/evidence.md",
            "--field",
            "priority=2",
            "--field",
            "custom={enabled: true}",
        ],
    );
    assert!(output.contains("Created gnosis/notes/plans/cutover.md"));
    let (metadata, body) = document(root, "gnosis/notes/plans/cutover.md");
    assert_eq!(metadata["type"], "Custom Playbook");
    assert_eq!(metadata["title"], "Cutover: \"production\"");
    assert_eq!(metadata["description"], description);
    assert_eq!(metadata["resource"], "https://example.test/service");
    assert_eq!(
        metadata["tags"],
        serde_yaml_ng::to_value(["migration", "production"]).unwrap()
    );
    assert_eq!(
        metadata["sources"][0]["resource"],
        "https://example.test/runbook"
    );
    assert_eq!(metadata["sources"][1]["resource"], "/evidence.md");
    assert_eq!(metadata["priority"].as_u64(), Some(2));
    assert_eq!(metadata["custom"]["enabled"].as_bool(), Some(true));
    assert!(metadata.get("generated").is_none());
    assert!(metadata.get("verified").is_none());
    assert_eq!(body, "# Cutover: \"production\"\n");
    assert!(read(root, "gnosis/notes/index.md").contains("plans/index.md"));
    assert!(read(root, "gnosis/notes/plans/index.md").contains("(cutover.md)"));
    assert!(!root.join("gnosis/notes/plans/cutover.md.md").exists());
    assert_eq!(read(root, "gnosis.toml"), manifest);
    assert_eq!(read(root, "gnosis.lock"), lock);
    assert_eq!(git(root, &["rev-parse", "HEAD"]).trim(), head);
    assert!(git(root, &["diff", "--cached", "--name-only"]).is_empty());
    cli(root, &["check"]);
}

#[test]
fn new_defaults_and_manual_authoring_produce_ordinary_okf_files() {
    let directory = authoring_workspace();
    let root = directory.path();
    cli(root, &["new", "notes", "first-note"]);
    let (metadata, body) = document(root, "gnosis/notes/first-note.md");
    assert_eq!(metadata["type"], "Reference");
    assert_eq!(metadata["title"], "First note");
    assert_eq!(metadata.as_mapping().unwrap().len(), 2);
    assert_eq!(body, "# First note\n");
    cli(
        root,
        &[
            "new",
            "notes",
            "nested/second_note.md",
            "--field",
            "type=Decision",
        ],
    );
    let (metadata, _) = document(root, "gnosis/notes/nested/second_note.md");
    assert_eq!(metadata["type"], "Decision");
    assert_eq!(metadata["title"], "Second note");
    let manual = concept("Manually authored knowledge.");
    write(root, "gnosis/notes/manual.md", &manual);
    cli(root, &["index"]);
    cli(root, &["check"]);
    assert_eq!(read(root, "gnosis/notes/manual.md"), manual);
    assert!(read(root, "gnosis/notes/index.md").contains("(manual.md)"));
}

#[test]
fn new_directories_store_metadata_and_accept_trailing_slashes() {
    let directory = authoring_workspace();
    let root = directory.path();
    cli(root, &["new", "notes", "guides", "--dir"]);
    cli(root, &["new", "notes", "guides/nested/"]);
    let index = read(root, "gnosis/notes/guides/index.md");
    let nested = read(root, "gnosis/notes/guides/nested/index.md");
    assert!(!index.starts_with("---"));
    assert!(!nested.starts_with("---"));
    assert!(index.contains("(nested/index.md)"));
    assert!(read(root, "gnosis/notes/index.md").contains("(guides/index.md)"));
    assert_eq!(
        fs::read_dir(root.join("gnosis/notes/guides/nested"))
            .unwrap()
            .count(),
        2
    );
    assert!(!root.join("gnosis/notes/guides.md").exists());
    cli(
        root,
        &[
            "new",
            "notes",
            "guides/nested/checklist",
            "--type",
            "Playbook",
        ],
    );
    assert!(read(root, "gnosis/notes/guides/nested/index.md").contains("(checklist.md)"));
    cli(root, &["check"]);
}

#[test]
fn folder_title_and_description_appear_in_parent_indexes_and_survive_reindexing() {
    let directory = authoring_workspace();
    let root = directory.path();
    cli(
        root,
        &["package", "beno-migrering", "--owner", "@sander-ed"],
    );
    write(
        root,
        "gnosis/beno-migrering/index.md",
        "---\nokf_version: \"0.2\"\n---\n<!-- Generated by gnosis. -->\n\n# beno-migrering\n",
    );
    let description = "Skal brukes når target plattform er Snowflake";
    cli(
        root,
        &[
            "new",
            "beno-migrering",
            "snowflake/",
            "--type",
            "Best Practise",
            "--title",
            "Snowflake",
            "--description",
            description,
        ],
    );
    let metadata: serde_yaml_ng::Value =
        serde_yaml_ng::from_str(&read(root, "gnosis/beno-migrering/snowflake/index.yml")).unwrap();
    assert_eq!(metadata["type"], "Best Practise");
    assert_eq!(metadata["title"], "Snowflake");
    assert_eq!(metadata["description"], description);
    let entry = format!("* [Snowflake](snowflake/index.md) - {description}\n");
    assert!(read(root, "gnosis/beno-migrering/index.md").contains(&entry));
    let folder_index = read(root, "gnosis/beno-migrering/snowflake/index.md");
    assert!(!folder_index.starts_with("---"));
    assert!(folder_index.contains(&format!("# Snowflake\n\n{description}\n")));
    assert!(!root.join("gnosis/beno-migrering/snowflake.md").exists());
    let before = snapshot(root);
    cli(root, &["index"]);
    assert_eq!(before, snapshot(root));

    cli(
        root,
        &[
            "new",
            "beno-migrering",
            "snowflake/checklist",
            "-T",
            "Playbook",
            "-t",
            "Cutover checklist",
            "-d",
            "Checks before cutover",
        ],
    );
    let (file_metadata, _) = document(root, "gnosis/beno-migrering/snowflake/checklist.md");
    assert_eq!(file_metadata["type"], "Playbook");
    assert_eq!(file_metadata["title"], "Cutover checklist");
    assert_eq!(file_metadata["description"], "Checks before cutover");
    assert!(
        read(root, "gnosis/beno-migrering/snowflake/index.md")
            .contains("* [Cutover checklist](checklist.md) - Checks before cutover\n")
    );
    assert!(read(root, "gnosis/beno-migrering/index.md").contains(&entry));

    cli(
        root,
        &[
            "new",
            "beno-migrering",
            "snowflake/advanced",
            "--dir",
            "-T",
            "Reference",
            "-t",
            "Advanced topics",
            "-d",
            "Special cases",
            "--tag",
            "snowflake",
            "--source",
            "/evidence.md",
            "--field",
            "custom={enabled: true}",
        ],
    );
    let folder_metadata: serde_yaml_ng::Value = serde_yaml_ng::from_str(&read(
        root,
        "gnosis/beno-migrering/snowflake/advanced/index.yml",
    ))
    .unwrap();
    assert_eq!(folder_metadata["type"], "Reference");
    assert_eq!(folder_metadata["tags"][0], "snowflake");
    assert_eq!(folder_metadata["sources"][0]["resource"], "/evidence.md");
    assert_eq!(folder_metadata["custom"]["enabled"].as_bool(), Some(true));
    assert!(
        read(root, "gnosis/beno-migrering/snowflake/index.md")
            .contains("* [Advanced topics](advanced/index.md) - Special cases\n")
    );
    cli(root, &["check"]);
}

#[test]
fn manual_folder_metadata_controls_navigation_and_invalid_metadata_is_rejected() {
    let directory = authoring_workspace();
    let root = directory.path();
    let metadata = "title: 'Custom [title]'\ndescription: 'Text: <details>'\ncustom: preserved\n";
    write(root, "gnosis/notes/my folder/index.yml", metadata);
    write(
        root,
        "gnosis/notes/plain/fact.md",
        concept("An ordinary folder without metadata."),
    );
    cli(root, &["index"]);
    let parent = read(root, "gnosis/notes/index.md");
    assert!(
        parent.contains("* [Custom \\[title\\]](my%20folder/index.md) - Text: &lt;details&gt;\n")
    );
    assert!(parent.contains("* [plain](plain/index.md)\n"));
    assert_eq!(read(root, "gnosis/notes/my folder/index.yml"), metadata);
    cli(root, &["check"]);

    for invalid in [
        "[not, a, mapping]",
        "title: [invalid]",
        "type: ''",
        "description: {}",
    ] {
        write(root, "gnosis/notes/my folder/index.yml", invalid);
        let before = snapshot(root);
        fails(
            root,
            &["index"],
            "invalid directory metadata my folder/index.yml",
        );
        fails(
            root,
            &["check"],
            "invalid directory metadata my folder/index.yml",
        );
        assert_eq!(before, snapshot(root));
    }
}

#[test]
fn folder_metadata_survives_upstream_sync_and_package_proposals() {
    let fixture = Fixture::new();
    fixture.install();
    cli(
        &fixture.consumer,
        &[
            "new",
            "platform",
            "snowflake/",
            "-T",
            "Best Practise",
            "-t",
            "Snowflake",
            "-d",
            "Local guidance",
        ],
    );
    let metadata = read(&fixture.consumer, "gnosis/platform/snowflake/index.yml");
    write(
        &fixture.source,
        "gnosis/platform/upstream.md",
        concept("New upstream content."),
    );
    commit(&fixture.source);
    cli(&fixture.consumer, &["sync", "--update"]);
    assert_eq!(
        read(&fixture.consumer, "gnosis/platform/snowflake/index.yml"),
        metadata
    );
    let entry = "* [Snowflake](snowflake/index.md) - Local guidance\n";
    assert!(read(&fixture.consumer, "gnosis/platform/index.md").contains(entry));

    let proposal = fixture.temporary.path().join("folder-proposal");
    cli(
        &fixture.consumer,
        &[
            "propose",
            "platform",
            "--output",
            proposal.to_str().unwrap(),
        ],
    );
    assert_eq!(
        read(&proposal, "gnosis/platform/snowflake/index.yml"),
        metadata
    );
    assert!(read(&proposal, "gnosis/platform/index.md").contains(entry));
    assert!(proposal.join("gnosis/platform/upstream.md").exists());
    assert!(!fixture.source.join("gnosis/platform/snowflake").exists());
    assert!(
        git(&proposal, &["diff", "--cached", "--name-only"])
            .lines()
            .all(|path| path.starts_with("gnosis/platform/"))
    );
}

#[test]
fn new_accepts_literal_file_and_stdin_bodies_without_rewriting_them() {
    use std::io::Write;
    use std::process::Stdio;

    let directory = authoring_workspace();
    let root = directory.path();
    let body = "# Exact body\r\n\r\nSymbols: : [] \"quotes\"\r\n";
    cli(root, &["new", "notes", "literal", "--body", body]);
    write(root, "draft.md", body);
    cli(
        root,
        &["new", "notes", "from-file", "--body-file", "draft.md"],
    );
    let mut child = Command::new(env!("CARGO_BIN_EXE_gnosis"))
        .arg("-C")
        .arg(root)
        .args(["new", "notes", "from-stdin", "--body-file", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(body.as_bytes())
        .unwrap();
    success(child.wait_with_output().unwrap());
    for name in ["literal", "from-file", "from-stdin"] {
        let (_, actual) = document(root, &format!("gnosis/notes/{name}.md"));
        assert_eq!(actual, body);
    }
    cli(root, &["new", "notes", "empty-body", "--body", ""]);
    assert_eq!(document(root, "gnosis/notes/empty-body.md").1, "");
}

#[test]
fn new_rejects_overwrites_reserved_paths_and_bad_inputs_without_changes() {
    let directory = authoring_workspace();
    let root = directory.path();
    cli(root, &["new", "notes", "existing"]);
    cli(root, &["new", "notes", "folder", "--dir"]);
    write(root, "invalid-body.bin", [0xff, 0xfe]);
    let before = snapshot(root);
    let cases: &[(&[&str], &str)] = &[
        (&["new", "notes", "existing"], "already exists"),
        (&["new", "notes", "existing.md"], "already exists"),
        (&["new", "notes", "folder", "--dir"], "already exists"),
        (&["new", "notes", "index"], "reserved"),
        (&["new", "notes", "log.md"], "reserved"),
        (&["new", "notes", "INDEX.md"], "reserved"),
        (&["new", "notes", "../escape"], "unsafe package path"),
        (&["new", "notes", "/escape"], "unsafe package path"),
        (&["new", "notes", ".git/escape"], "unsafe package path"),
        (
            &["new", "notes", "nested/../../escape"],
            "unsafe package path",
        ),
        (&["new", "notes", "invalid.txt"], ".md extension"),
        (
            &["new", "notes", "folder.md", "--dir"],
            "must not end in .md",
        ),
        (&["new", "unknown", "document"], "unknown package"),
        (
            &["new", "notes", "new-folder", "--dir", "--body", "text"],
            "cannot be used",
        ),
        (
            &[
                "new",
                "notes",
                "new-folder/",
                "--body",
                "Not a generated directory body",
            ],
            "directory bodies are generated",
        ),
        (
            &[
                "new",
                "notes",
                "document",
                "--body",
                "text",
                "--body-file",
                "draft.md",
            ],
            "cannot be used",
        ),
        (
            &["new", "notes", "document", "--type", ""],
            "nonempty string type",
        ),
        (
            &["new", "notes", "document", "--title", "one\ntwo"],
            "single line",
        ),
        (
            &["new", "notes", "document", "--field", "custom"],
            "KEY=YAML",
        ),
        (
            &["new", "notes", "document", "--field", "custom=["],
            "invalid YAML",
        ),
        (
            &["new", "notes", "document", "--field", "type=3"],
            "nonempty string type",
        ),
        (
            &[
                "new",
                "notes",
                "document",
                "--type",
                "Reference",
                "--field",
                "type=Other",
            ],
            "more than once",
        ),
        (
            &[
                "new", "notes", "document", "--field", "custom=1", "--field", "custom=2",
            ],
            "more than once",
        ),
        (
            &["new", "notes", "document", "--body-file", "absent.md"],
            "cannot read body file",
        ),
        (
            &[
                "new",
                "notes",
                "document",
                "--body-file",
                "invalid-body.bin",
            ],
            "must be UTF-8",
        ),
    ];
    for (arguments, error) in cases {
        fails(root, arguments, error);
        assert_eq!(
            snapshot(root),
            before,
            "failed command modified workspace: {arguments:?}"
        );
    }
}

#[test]
fn new_preserves_manual_indexes_and_explains_the_navigation_gap() {
    let directory = authoring_workspace();
    let root = directory.path();
    let manual = "# Human navigation\n\nKeep this introduction.\n";
    write(root, "gnosis/notes/index.md", manual);
    let output = cli_output(root, &["new", "notes", "nested/new-fact"]);
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("preserved hand-authored gnosis/notes/index.md")
    );
    success(output);
    assert_eq!(read(root, "gnosis/notes/index.md"), manual);
    assert!(read(root, "gnosis/notes/nested/index.md").contains("(new-fact.md)"));
}

#[test]
fn new_edits_imported_packages_offline_and_only_proposes_selected_content() {
    let fixture = Fixture::new();
    fixture.install();
    let lock = read(&fixture.consumer, "gnosis.lock");
    let source_before = snapshot(&fixture.source);
    let unavailable = fixture.temporary.path().join("temporarily-unavailable");
    fs::rename(&fixture.source, &unavailable).unwrap();
    cli(
        &fixture.consumer,
        &[
            "new",
            "platform",
            "agent-finding",
            "--body",
            "A local finding.",
        ],
    );
    fs::rename(&unavailable, &fixture.source).unwrap();
    assert_eq!(read(&fixture.consumer, "gnosis.lock"), lock);
    assert_eq!(snapshot(&fixture.source), source_before);
    let proposal = fixture.temporary.path().join("proposal");
    cli(
        &fixture.consumer,
        &[
            "propose",
            "platform",
            "--output",
            proposal.to_str().unwrap(),
        ],
    );
    assert!(
        git(&proposal, &["diff", "--cached", "--name-only"])
            .contains("gnosis/platform/agent-finding.md")
    );
    assert_eq!(
        document(&proposal, "gnosis/platform/agent-finding.md").1,
        "A local finding."
    );
    assert_eq!(snapshot(&fixture.source), source_before);
}

#[test]
fn add_accepts_names_copied_from_list() {
    let fixture = Fixture::new();
    let listing = cli(&fixture.consumer, &["list"]);
    let name = listing
        .lines()
        .find(|line| line.starts_with("team/platform\t"))
        .unwrap()
        .split_whitespace()
        .next()
        .unwrap();
    cli(&fixture.consumer, &["add", name]);
    cli(&fixture.consumer, &["check"]);
    assert!(fixture.consumer.join("gnosis/core/facts.md").exists());
    let manifest: toml::Value = toml::from_str(&read(&fixture.consumer, "gnosis.toml")).unwrap();
    assert_eq!(manifest["dependencies"]["platform"].as_str(), Some("team"));
    cli(&fixture.consumer, &["add", name, "--source", "team"]);
}

#[test]
fn add_infers_the_unique_publisher_among_sources() {
    let fixture = Fixture::new();
    let second = fixture.other("empty-source");
    git(&second, &["init", "--quiet", "--initial-branch=main"]);
    cli(&second, &["init"]);
    commit(&second);
    cli(
        &fixture.consumer,
        &["source", "other", second.to_str().unwrap()],
    );
    cli(&fixture.consumer, &["add", "platform"]);
    cli(&fixture.consumer, &["check"]);
    assert!(fixture.consumer.join("gnosis/core/facts.md").exists());
}

#[test]
fn add_reports_ambiguity_and_preserves_an_existing_selection() {
    let fixture = Fixture::new();
    cli(
        &fixture.consumer,
        &["source", "other", fixture.source.to_str().unwrap()],
    );
    let before = snapshot(&fixture.consumer);
    fails(
        &fixture.consumer,
        &["add", "platform"],
        "other/platform, team/platform",
    );
    assert_eq!(before, snapshot(&fixture.consumer));
    cli(&fixture.consumer, &["add", "team/core"]);
    cli(&fixture.consumer, &["add", "team/platform"]);
    let installed = snapshot(&fixture.consumer);
    cli(&fixture.consumer, &["add", "platform"]);
    assert_eq!(installed, snapshot(&fixture.consumer));
}

#[test]
fn add_rejects_conflicting_sources_and_invalid_names_without_changes() {
    let fixture = Fixture::new();
    let before = snapshot(&fixture.consumer);
    for (args, message) in [
        (
            vec!["add", "team/platform", "--source", "other"],
            "conflicts with --source",
        ),
        (vec!["add", "team/platform/extra"], "invalid name"),
        (vec!["add", "/platform"], "invalid name"),
        (vec!["add", "team/"], "invalid name"),
        (vec!["add", "missing/platform"], "unknown source"),
        (
            vec!["add", "missing"],
            "no configured source publishes missing",
        ),
    ] {
        fails(&fixture.consumer, &args, message);
        assert_eq!(before, snapshot(&fixture.consumer));
    }
}

#[test]
fn add_explains_local_packages_and_missing_sources() {
    let fixture = Fixture::new();
    let before = snapshot(&fixture.source);
    for args in [
        vec!["add", "platform"],
        vec!["add", "team/platform"],
        vec!["add", "platform", "--source", "team"],
    ] {
        fails(
            &fixture.source,
            &args,
            "already available in this workspace",
        );
        fails(&fixture.source, &args, "consuming project");
        assert_eq!(before, snapshot(&fixture.source));
    }
    fails(
        &fixture.source,
        &["add", "missing"],
        "gnosis source ALIAS REPOSITORY",
    );
    assert_eq!(before, snapshot(&fixture.source));
}
