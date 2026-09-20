package integration

import (
	"bytes"
	"context"
	"encoding/json"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"gnosis/internal/cli"
	"gnosis/internal/knowledge"
	"gnosis/internal/testutil"
)

func testCLI(t *testing.T, dir string, args ...string) string {
	t.Helper()
	output, failure, code := testCLIResult(dir, args...)
	if code != 0 {
		t.Fatalf("gnosis %v: exit %d\n%s\n%s", args, code, output, failure)
	}
	return output
}

func testCLIResult(dir string, args ...string) (string, string, int) {
	var stdout, stderr bytes.Buffer
	code := run(append([]string{"-C", dir}, args...), &stdout, &stderr)
	return stdout.String(), stderr.String(), code
}

func testCommit(t *testing.T, dir, message string) {
	t.Helper()
	testutil.Git(t, dir, "add", ".")
	testutil.Git(t, dir, "commit", "--quiet", "-m", message)
}

func testSource(t *testing.T) string {
	t.Helper()
	dir := testutil.Repo(t)
	testCLI(t, dir, "init", "--skills=false")
	for _, name := range []string{"core", "risk", "unrelated"} {
		args := []string{"package", "init", name, "--owner", "@example/owners", "--description", name}
		if name == "risk" {
			args = append(args, "--dependency", "core")
		}
		testCLI(t, dir, args...)
		testutil.Write(t, filepath.Join(dir, "gnosis", name, "concept.md"),
			"---\ntype: Reference\ntitle: "+name+"\n---\nOriginal "+name+"\n",
		)
	}
	testCLI(t, dir, "index")
	testCommit(t, dir, "initial knowledge")
	return dir
}

func testConsumer(t *testing.T, source string) string {
	t.Helper()
	dir := testutil.Repo(t)
	testCLI(t, dir, "init", "--skills=false")
	testCLI(t, dir, "registry", "add", source)
	testCommit(t, dir, "configure registry")
	return dir
}

func TestInitializeAndOwners(t *testing.T) {
	dir := testutil.Repo(t)
	testCLI(t, dir, "init")
	for _, name := range []string{"gnosis-cli", "gnosis-update", "gnosis-navigate"} {
		data := testutil.Read(t, filepath.Join(dir, ".agents", "skills", name, "SKILL.md"))
		fields, _, err := knowledge.Frontmatter([]byte(data))
		if err != nil || fields["name"] != name || fields["description"] == "" {
			t.Fatalf("invalid skill %s: %v", name, err)
		}
	}
	_, failure, code := testCLIResult(dir, "init")
	if code == 0 || !strings.Contains(failure, "already exists") {
		t.Fatalf("reinitialization did not fail: %s", failure)
	}
	testCLI(t, dir, "package", "init", "architecture", "--owner", "@sander-ed")
	testutil.Write(t, filepath.Join(dir, ".github", "CODEOWNERS"), "/other/ @someone\n")
	testCLI(t, dir, "codeowners")
	before := testutil.Read(t, filepath.Join(dir, ".github", "CODEOWNERS"))
	testCLI(t, dir, "codeowners")
	after := testutil.Read(t, filepath.Join(dir, ".github", "CODEOWNERS"))
	if before != after || !strings.Contains(after, "/other/ @someone") ||
		!strings.Contains(after, "/gnosis/architecture/ @sander-ed") {
		t.Fatalf("bad CODEOWNERS update: %s", after)
	}
	testCLI(t, filepath.Join(dir, "gnosis", "architecture"), "validate")
	var packages map[string]knowledge.Manifest
	if err := json.Unmarshal([]byte(testCLI(t, dir, "list", "--json")), &packages); err != nil {
		t.Fatal(err)
	}
	if len(packages) != 1 {
		t.Fatalf("unexpected packages: %+v", packages)
	}
}

func TestAddPullRestoreAndProposal(t *testing.T) {
	source := testSource(t)
	consumer := testConsumer(t, source)
	testCLI(t, consumer, "add", "risk")
	testCLI(t, consumer, "validate")
	dir := filepath.Join(consumer, "gnosis")
	lock, err := knowledge.ReadLock(dir)
	if err != nil {
		t.Fatal(err)
	}
	if len(lock.Packages) != 2 {
		t.Fatalf("wrong dependency closure: %+v", lock.Packages)
	}
	if _, err := os.Stat(filepath.Join(dir, "unrelated")); !os.IsNotExist(err) {
		t.Fatalf("imported unrelated package: %v", err)
	}
	if status := testutil.Git(t, consumer, "status", "--porcelain"); status != "" {
		t.Fatalf("install left a dirty worktree: %s", status)
	}
	testCLI(t, consumer, "add", "risk")
	testCLI(t, consumer, "codeowners")
	if data := testutil.Read(t, filepath.Join(consumer, ".github", "CODEOWNERS")); strings.Contains(data, "/gnosis/risk/") {
		t.Fatal("imported owners were imposed on the consumer")
	}
	testCommit(t, consumer, "consumer ownership")
	lockedBytes := testutil.Read(t, filepath.Join(dir, "gnosis.lock"))
	for _, update := range []string{"Second", "Third"} {
		testutil.Write(t, filepath.Join(source, "gnosis", "risk", "concept.md"),
			"---\ntype: Reference\ntitle: risk\n---\n"+update+" risk\n",
		)
		testCommit(t, source, update)
		testCLI(t, consumer, "pull", "risk")
		if got := testutil.Read(t, filepath.Join(dir, "risk", "concept.md")); !strings.Contains(got, update) {
			t.Fatalf("pull did not update content: %s", got)
		}
	}
	restore := testutil.Repo(t)
	testCLI(t, restore, "init", "--skills=false")
	testutil.Write(t, filepath.Join(restore, "gnosis", "gnosis.lock"), lockedBytes)
	testCommit(t, restore, "restore pinned dependencies without registry")
	testCLI(t, restore, "restore")
	if got := testutil.Read(t, filepath.Join(restore, "gnosis", "risk", "concept.md")); !strings.Contains(got, "Original") {
		t.Fatalf("restore followed tip instead of lock: %s", got)
	}
	testutil.Write(t, filepath.Join(dir, "risk", "concept.md"),
		"---\ntype: Reference\ntitle: risk\n---\nThird risk\n\nLocal proposal\n",
	)
	testutil.Write(t, filepath.Join(consumer, "consumer.txt"), "do not publish this")
	testCommit(t, consumer, "package and consumer changes")
	remoteHead := testutil.Git(t, source, "rev-parse", "HEAD")
	output := testCLI(t, consumer, "propose", "risk", "--title", "Improve risk", "--branch", "gnosis/test")
	lines := strings.Split(output, "\n")
	proposal := strings.TrimPrefix(lines[0], "Proposal checkout: ")
	diff := testutil.Git(t, proposal, "diff", "--name-only", "origin/main", "HEAD")
	if diff != "gnosis/risk/concept.md" {
		t.Fatalf("proposal included unexpected files: %s", diff)
	}
	if got := testutil.Git(t, source, "rev-parse", "HEAD"); got != remoteHead {
		t.Fatal("prepare changed source default branch")
	}
	content := testutil.Read(t, filepath.Join(proposal, "gnosis", "risk", "concept.md"))
	if !strings.Contains(content, "Local proposal") {
		t.Fatal("proposal omitted local changes")
	}
}

func TestRootPackageAndRegistryImport(t *testing.T) {
	sourceRepo := testPackage(t)
	testutil.Write(t, filepath.Join(sourceRepo, "concept.md"), "---\ntype: Reference\n---\nRoot package\n")
	testutil.Git(t, sourceRepo, "init", "--quiet", "-b", "main")
	testutil.Git(t, sourceRepo, "config", "user.name", "Gnosis Test")
	testutil.Git(t, sourceRepo, "config", "user.email", "test@example.invalid")
	testutil.Git(t, sourceRepo, "config", "commit.gpgsign", "false")
	testCommit(t, sourceRepo, "initial root package")
	consumer := testutil.Repo(t)
	testCLI(t, consumer, "init", "--skills=false")
	r := knowledge.Registry{"sample": {
		Description: "sample description", Owner: "@example/owners",
		Source: knowledge.Source{Type: "git", Repository: sourceRepo, Path: ".", DefaultBranch: "main"},
	}}
	data, err := json.Marshal(r)
	if err != nil {
		t.Fatal(err)
	}
	input := filepath.Join(t.TempDir(), "registry.json")
	testutil.Write(t, input, string(data))
	testCLI(t, consumer, "registry", "import", input)
	testCLI(t, consumer, "registry", "add", sourceRepo, "--path", ".")
	testCommit(t, consumer, "root package registry")
	testCLI(t, consumer, "add", "sample")
	testutil.Write(t, filepath.Join(consumer, "gnosis", "sample", "concept.md"),
		"---\ntype: Reference\n---\nRoot package\n\nProposed addition\n",
	)
	testCommit(t, consumer, "root package improvement")
	output := testCLI(t, consumer, "propose", "sample")
	proposal := strings.TrimPrefix(strings.Split(output, "\n")[0], "Proposal checkout: ")
	if diff := testutil.Git(t, proposal, "diff", "--name-only", "origin/main"); diff != "concept.md" {
		t.Fatalf("wrong root proposal scope: %s", diff)
	}
}

func TestConflictContinueAndAbort(t *testing.T) {
	source := testSource(t)
	consumer := testConsumer(t, source)
	testCLI(t, consumer, "add", "risk")
	file := filepath.Join(consumer, "gnosis", "risk", "concept.md")
	testutil.Write(t, file, "---\ntype: Reference\n---\nConsumer change\n")
	testCommit(t, consumer, "local change")
	lockBefore := testutil.Read(t, filepath.Join(consumer, "gnosis", "gnosis.lock"))
	testutil.Write(t, filepath.Join(source, "gnosis", "risk", "concept.md"),
		"---\ntype: Reference\n---\nUpstream change\n",
	)
	testCommit(t, source, "upstream change")
	for _, abort := range []bool{true, false} {
		_, failure, code := testCLIResult(consumer, "pull", "risk")
		if code == 0 || !strings.Contains(failure, "operation retained") {
			t.Fatalf("expected conflict: code=%d error=%s", code, failure)
		}
		if got := testutil.Read(t, filepath.Join(consumer, "gnosis", "gnosis.lock")); got != lockBefore {
			t.Fatal("conflict advanced the lock")
		}
		if abort {
			testCLI(t, consumer, "pull", "--abort")
			if got := testutil.Read(t, file); !strings.Contains(got, "Consumer change") {
				t.Fatal("abort discarded local committed changes")
			}
			continue
		}
		_, failure, code = testCLIResult(consumer, "pull", "--continue")
		if code == 0 || !strings.Contains(failure, "resolve and stage") {
			t.Fatalf("accepted unresolved conflict: %s", failure)
		}
		testutil.Write(t, file, "---\ntype: Reference\n---\nResolved change\n")
		testutil.Git(t, consumer, "add", "gnosis/risk/concept.md")
		testCLI(t, consumer, "pull", "--continue")
		if got := testutil.Read(t, filepath.Join(consumer, "gnosis", "gnosis.lock")); got == lockBefore {
			t.Fatal("completed merge did not advance lock")
		}
		if status := testutil.Git(t, consumer, "status", "--porcelain"); status != "" {
			t.Fatalf("conflict recovery left dirty worktree: %s", status)
		}
		testCLI(t, consumer, "pull", "risk")
	}
}

func TestResolutionErrorsDoNotMutateConsumer(t *testing.T) {
	source := testSource(t)
	consumer := testConsumer(t, source)
	before := testutil.Git(t, consumer, "rev-parse", "HEAD")
	riskPath := filepath.Join(source, "gnosis", "core", "package.yml")
	m, err := knowledge.ReadManifest(filepath.Dir(riskPath))
	if err != nil {
		t.Fatal(err)
	}
	m.Dependencies = []string{"risk"}
	if err := knowledge.WriteData(riskPath, m); err != nil {
		t.Fatal(err)
	}
	testCommit(t, source, "cyclic dependency")
	_, failure, code := testCLIResult(consumer, "add", "risk")
	if code == 0 || !strings.Contains(failure, "cycle") {
		t.Fatalf("expected cycle error: %s", failure)
	}
	if after := testutil.Git(t, consumer, "rev-parse", "HEAD"); after != before {
		t.Fatal("preflight failure changed consumer history")
	}
	testutil.Write(t, filepath.Join(consumer, "uncommitted"), "keep me")
	_, failure, code = testCLIResult(consumer, "add", "risk")
	if code == 0 || !strings.Contains(failure, "pending changes") {
		t.Fatalf("dirty checkout not rejected: %s", failure)
	}
}

func TestCancellation(t *testing.T) {
	ctx, cancel := context.WithCancel(context.Background())
	cancel()
	var stdout, stderr bytes.Buffer
	code := cli.Execute(ctx, []string{"status"}, cli.Streams{Out: &stdout, Err: &stderr}, "dev")
	if code != 130 {
		t.Fatalf("cancellation exit = %d: %s", code, stderr.String())
	}
}
