package integration

import (
	"context"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"gnosis/internal/knowledge"
	"gnosis/internal/testutil"
	"gnosis/internal/workspace"
)

func TestPullValidatesMergedDependencyGraph(t *testing.T) {
	for _, conflict := range []bool{false, true} {
		name := "automatic merge"
		if conflict {
			name = "conflict continuation"
		}
		t.Run(name, func(t *testing.T) {
			source := testutil.Repo(t)
			testCLI(t, source, "init", "--skills=false")
			for _, name := range []string{"alpha", "beta"} {
				if err := knowledge.ScaffoldPackage(filepath.Join(source, "gnosis", name), testManifest(name)); err != nil {
					t.Fatal(err)
				}
			}
			testCommit(t, source, "independent packages")
			consumer := testConsumer(t, source)
			testCLI(t, consumer, "add", "alpha", "beta")
			lockBefore, err := knowledge.ReadLock(filepath.Join(consumer, "gnosis"))
			if err != nil {
				t.Fatal(err)
			}
			alpha := testManifest("alpha", "beta")
			if err := knowledge.WriteData(filepath.Join(consumer, "gnosis", "alpha", "package.yml"), alpha); err != nil {
				t.Fatal(err)
			}
			betaFile := filepath.Join(consumer, "gnosis", "beta", "package.yml")
			if conflict {
				local := testManifest("beta")
				local.Version = "local"
				if err := knowledge.WriteData(betaFile, local); err != nil {
					t.Fatal(err)
				}
			}
			testCommit(t, consumer, "local dependency")
			testCLI(t, consumer, "validate")
			upstream := testManifest("beta", "alpha")
			if conflict {
				upstream.Version = "upstream"
			}
			if err := knowledge.WriteData(filepath.Join(source, "gnosis", "beta", "package.yml"), upstream); err != nil {
				t.Fatal(err)
			}
			testCommit(t, source, "upstream dependency")
			testCLI(t, source, "validate")
			_, failure, code := testCLIResult(consumer, "pull")
			if code != 1 {
				t.Fatalf("invalid update exit=%d: %s", code, failure)
			}
			if conflict {
				if !strings.Contains(failure, "operation retained") {
					t.Fatalf("expected merge conflict: %s", failure)
				}
				if err := knowledge.WriteData(betaFile, upstream); err != nil {
					t.Fatal(err)
				}
				testutil.Git(t, consumer, "add", "gnosis/beta/package.yml")
			} else if !strings.Contains(failure, "cycle") {
				t.Fatalf("expected merged cycle rejection: %s", failure)
			}
			beforeContinue := testutil.Git(t, consumer, "rev-parse", "HEAD")
			_, failure, code = testCLIResult(consumer, "pull", "--continue")
			if code != 1 || !strings.Contains(failure, "cycle") {
				t.Fatalf("continuation accepted cycle: %s", failure)
			}
			if after := testutil.Git(t, consumer, "rev-parse", "HEAD"); after != beforeContinue {
				t.Fatal("invalid continuation committed changes")
			}
			lockAfter, err := knowledge.ReadLock(filepath.Join(consumer, "gnosis"))
			if err != nil {
				t.Fatal(err)
			}
			if lockAfter.Packages["beta"].Commit != lockBefore.Packages["beta"].Commit {
				t.Fatal("invalid merged graph advanced beta's lock")
			}
			w, err := workspace.Open(context.Background(), consumer)
			if err != nil {
				t.Fatal(err)
			}
			if _, err := os.Stat(w.PendingPath()); err != nil {
				t.Fatalf("invalid update lost recovery state: %v", err)
			}
			upstream.Dependencies = []string{}
			if err := knowledge.WriteData(betaFile, upstream); err != nil {
				t.Fatal(err)
			}
			if conflict {
				testutil.Git(t, consumer, "add", "gnosis/beta/package.yml")
			} else {
				testCommit(t, consumer, "repair merged dependency graph")
			}
			testCLI(t, consumer, "pull", "--continue")
			testCLI(t, consumer, "validate")
			if err := w.NoPending(); err != nil {
				t.Fatal(err)
			}
		})
	}
}

func TestRestoreAfterSourceBranchRename(t *testing.T) {
	source := testSource(t)
	consumer := testConsumer(t, source)
	testCLI(t, consumer, "add", "risk")
	lockedBytes := testutil.Read(t, filepath.Join(consumer, "gnosis", "gnosis.lock"))
	testutil.Git(t, source, "branch", "-m", "renamed")
	testutil.Write(t, filepath.Join(source, "gnosis", "core", "concept.md"), "---\ntype: Reference\n---\nNew tip\n")
	testCommit(t, source, "new source tip")
	testutil.Git(t, consumer, "rm", "-r", "gnosis/core")
	testCommit(t, consumer, "missing dependency")
	fresh := testutil.Repo(t)
	testCLI(t, fresh, "init", "--skills=false")
	testutil.Write(t, filepath.Join(fresh, "gnosis", "gnosis.lock"), lockedBytes)
	testCommit(t, fresh, "locked packages only")
	for _, repo := range []string{consumer, fresh} {
		testCLI(t, repo, "restore")
		testCLI(t, repo, "validate")
		if got := testutil.Read(t, filepath.Join(repo, "gnosis", "core", "concept.md")); !strings.Contains(got, "Original") {
			t.Fatalf("restore followed branch tip: %s", got)
		}
		if got := testutil.Read(t, filepath.Join(repo, "gnosis", "gnosis.lock")); got != lockedBytes {
			t.Fatal("restore changed locked sources or commits")
		}
	}
}

func TestPullHonorsExplicitRegistrySourceChange(t *testing.T) {
	source := testSource(t)
	consumer := testConsumer(t, source)
	testCLI(t, consumer, "add", "risk")
	testutil.Git(t, source, "checkout", "-b", "release")
	for _, name := range []string{"core", "risk"} {
		testutil.Write(t, filepath.Join(source, "gnosis", name, "concept.md"), "---\ntype: Reference\n---\nRelease content\n")
	}
	testCommit(t, source, "release updates")
	r, err := knowledge.ReadRegistry(filepath.Join(consumer, "gnosis"))
	if err != nil {
		t.Fatal(err)
	}
	for name, entry := range r {
		entry.Source.DefaultBranch = "release"
		r[name] = entry
	}
	registryFile := filepath.Join(consumer, "gnosis", "registry.yml")
	if err := knowledge.WriteData(registryFile, r); err != nil {
		t.Fatal(err)
	}
	testCommit(t, consumer, "explicitly retarget discovery")
	testCLI(t, consumer, "pull", "risk")
	if got := testutil.Read(t, filepath.Join(consumer, "gnosis", "risk", "concept.md")); !strings.Contains(got, "Release") {
		t.Fatalf("pull ignored registry source change: %s", got)
	}
	if got := testutil.Read(t, filepath.Join(consumer, "gnosis", "core", "concept.md")); !strings.Contains(got, "Original") {
		t.Fatalf("pull changed an unselected dependency pin: %s", got)
	}
	lock, err := knowledge.ReadLock(filepath.Join(consumer, "gnosis"))
	if err != nil {
		t.Fatal(err)
	}
	if lock.Packages["risk"].Source.DefaultBranch != "release" || lock.Packages["core"].Source.DefaultBranch != "main" {
		t.Fatalf("unexpected source pins: %+v", lock.Packages)
	}
	entry := r["risk"]
	entry.Source.DefaultBranch = "missing"
	r["risk"] = entry
	if err := knowledge.WriteData(registryFile, r); err != nil {
		t.Fatal(err)
	}
	testCommit(t, consumer, "invalid explicit source")
	before := testutil.Git(t, consumer, "rev-parse", "HEAD")
	_, failure, code := testCLIResult(consumer, "pull", "risk")
	if code != 1 || !strings.Contains(failure, "missing") {
		t.Fatalf("silently fell back to locked branch: %s", failure)
	}
	if after := testutil.Git(t, consumer, "rev-parse", "HEAD"); after != before {
		t.Fatal("invalid source changed consumer history")
	}
}

func TestPullRejectsCyclicPinsBeforeMutation(t *testing.T) {
	source := testSource(t)
	consumer := testConsumer(t, source)
	testCLI(t, consumer, "add", "risk")
	for _, repo := range []string{consumer, source} {
		m, err := knowledge.ReadManifest(filepath.Join(repo, "gnosis", "risk"))
		if err != nil {
			t.Fatal(err)
		}
		m.Dependencies = []string{}
		if err := knowledge.WriteData(filepath.Join(repo, "gnosis", "risk", "package.yml"), m); err != nil {
			t.Fatal(err)
		}
	}
	core, err := knowledge.ReadManifest(filepath.Join(source, "gnosis", "core"))
	if err != nil {
		t.Fatal(err)
	}
	core.Dependencies = []string{"risk"}
	if err := knowledge.WriteData(filepath.Join(source, "gnosis", "core", "package.yml"), core); err != nil {
		t.Fatal(err)
	}
	testCommit(t, source, "reverse dependency direction")
	testCommit(t, consumer, "local dependency removal")
	testCLI(t, source, "validate")
	testCLI(t, consumer, "validate")
	before := testutil.Git(t, consumer, "rev-parse", "HEAD")
	_, failure, code := testCLIResult(consumer, "pull", "core")
	if code != 1 || !strings.Contains(failure, "lock dependency graph") {
		t.Fatalf("accepted cyclic pins behind valid local graph: exit=%d %s", code, failure)
	}
	if after := testutil.Git(t, consumer, "rev-parse", "HEAD"); after != before {
		t.Fatal("invalid projected pins changed history")
	}
	testCLI(t, consumer, "pull", "core", "risk")
	testCLI(t, consumer, "validate")
}
