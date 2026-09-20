package main

import (
	"archive/tar"
	"bytes"
	"context"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestArchiveRejectsUnsafeEntries(t *testing.T) {
	for _, header := range []*tar.Header{
		{Name: "../escape", Typeflag: tar.TypeReg},
		{Name: "/absolute", Typeflag: tar.TypeReg},
		{Name: "link", Typeflag: tar.TypeSymlink, Linkname: "/tmp"},
		{Name: "hardlink", Typeflag: tar.TypeLink, Linkname: "../escape"},
		{Name: ".git/config", Typeflag: tar.TypeReg},
	} {
		t.Run(header.Name, func(t *testing.T) {
			var data bytes.Buffer
			writer := tar.NewWriter(&data)
			if err := writer.WriteHeader(header); err != nil {
				t.Fatal(err)
			}
			if err := writer.Close(); err != nil {
				t.Fatal(err)
			}
			if err := extractTree(tar.NewReader(&data), t.TempDir()); err == nil {
				t.Fatal("accepted unsafe archive entry")
			}
		})
	}
}

func TestImportValidatesActualGitTree(t *testing.T) {
	source := testSource(t)
	consumer := testConsumer(t, source)
	testWrite(t, filepath.Join(source, "gnosis", "risk", ".gitattributes"), "invalid.md export-ignore\n")
	testWrite(t, filepath.Join(source, "gnosis", "risk", "invalid.md"), "Missing frontmatter\n")
	testCommit(t, source, "hidden invalid document")
	before := testGit(t, consumer, "rev-parse", "HEAD")
	_, failure, code := testCLIResult(consumer, "add", "risk")
	if code == 0 || !strings.Contains(failure, "invalid.md") {
		t.Fatalf("export-ignore bypassed validation: %s", failure)
	}
	if got := testGit(t, consumer, "rev-parse", "HEAD"); got != before {
		t.Fatal("validation failure partially imported dependencies")
	}
	testGit(t, source, "rm", "gnosis/risk/invalid.md")
	if err := os.Symlink("/tmp", filepath.Join(source, "gnosis", "risk", "escape")); err != nil {
		t.Fatal(err)
	}
	testCommit(t, source, "unsafe symlink")
	_, failure, code = testCLIResult(consumer, "add", "risk")
	if code == 0 || !strings.Contains(failure, "symlink") {
		t.Fatalf("symlink import not rejected: %s", failure)
	}
}

func TestLockValidationAndOperationLock(t *testing.T) {
	dir := testRepo(t)
	testCLI(t, dir, "init", "--skills=false")
	w, err := openWorkspace(context.Background(), dir)
	if err != nil {
		t.Fatal(err)
	}
	release, err := w.acquire()
	if err != nil {
		t.Fatal(err)
	}
	_, failure, code := testCLIResult(dir, "package", "init", "blocked", "--owner", "@test")
	if code == 0 || !strings.Contains(failure, "another gnosis operation") {
		t.Fatalf("concurrent mutation not rejected: %s", failure)
	}
	if err := release(); err != nil {
		t.Fatal(err)
	}
	testWrite(t, filepath.Join(dir, "gnosis", "gnosis.lock"), "gnosis_version: 99\npackages: {}\n")
	_, failure, code = testCLIResult(dir, "validate")
	if code == 0 || !strings.Contains(failure, "invalid gnosis.lock") {
		t.Fatalf("bad lock accepted: %s", failure)
	}
}

func TestProposalPublicationAndUpstreamIsolation(t *testing.T) {
	source := testSource(t)
	consumer := testConsumer(t, source)
	testCLI(t, consumer, "add", "risk")
	testWrite(t, filepath.Join(consumer, "gnosis", "risk", "local.md"), "---\ntype: Reference\n---\nLocal addition\n")
	testWrite(t, filepath.Join(consumer, "private.txt"), "consumer-only file\n")
	testCommit(t, consumer, "propose local addition")
	testWrite(t, filepath.Join(source, "gnosis", "risk", "upstream.md"), "---\ntype: Reference\n---\nUpstream addition\n")
	testWrite(t, filepath.Join(source, "gnosis", "unrelated", "concept.md"),
		"---\ntype: Reference\n---\nUnrelated update\n",
	)
	testCommit(t, source, "new upstream work")
	before := testGit(t, source, "rev-parse", "main")
	bin := t.TempDir()
	argsFile := filepath.Join(t.TempDir(), "args")
	testWrite(t, filepath.Join(bin, "gh"),
		"#!/bin/sh\nprintf '%s\\n' \"$@\" > \"$GNOSIS_GH_ARGS\"\nprintf 'https://github.example/pr/1\\n'\n",
	)
	if err := os.Chmod(filepath.Join(bin, "gh"), 0o755); err != nil {
		t.Fatal(err)
	}
	t.Setenv("PATH", bin+string(os.PathListSeparator)+os.Getenv("PATH"))
	t.Setenv("GNOSIS_GH_ARGS", argsFile)
	testCLI(t, consumer, "propose", "risk", "--publish", "--title", "Proposal", "--branch", "gnosis/published")
	if after := testGit(t, source, "rev-parse", "main"); after != before {
		t.Fatal("publish changed the authoritative branch")
	}
	if diff := testGit(t, source, "diff", "--name-only", "main", "gnosis/published"); diff != "gnosis/risk/local.md" {
		t.Fatalf("proposal clobbered upstream/consumer changes: %s", diff)
	}
	args := testRead(t, argsFile)
	if !strings.Contains(args, "pr\ncreate\n--base\nmain\n--head\ngnosis/published\n") {
		t.Fatalf("wrong gh invocation: %s", args)
	}
	testWrite(t, filepath.Join(bin, "gh"), "#!/bin/sh\nprintf 'not authenticated\\n' >&2\nexit 9\n")
	_, failure, code := testCLIResult(
		consumer, "propose", "risk", "--publish", "--branch", "gnosis/pr-failure",
	)
	if code == 0 || !strings.Contains(failure, "was pushed but PR creation failed") {
		t.Fatalf("PR failure was not surfaced accurately: %s", failure)
	}
	testGit(t, source, "rev-parse", "--verify", "refs/heads/gnosis/pr-failure")
	if after := testGit(t, source, "rev-parse", "main"); after != before {
		t.Fatal("failed PR creation changed the authoritative branch")
	}
}

func TestGitPassthrough(t *testing.T) {
	dir := testRepo(t)
	testCLI(t, dir, "init", "--skills=false")
	testCommit(t, dir, "initial")
	testGit(t, dir, "checkout", "--quiet", "-b", "feature")
	testWrite(t, filepath.Join(dir, "change.txt"), "feature\n")
	testCommit(t, dir, "feature")
	testCLI(t, dir, "rebase", "-X", "ours", "main")
	testCLI(t, dir, "merge", "--no-edit", "main")
	if status := testCLI(t, dir, "status", "--porcelain"); status != "" {
		t.Fatalf("unexpected status output: %s", status)
	}
	_, _, code := testCLIResult(dir, "rebase", "--abort")
	if code != 128 {
		t.Fatalf("native Git exit code was not preserved: %d", code)
	}
}
