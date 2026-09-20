package integration

import (
	"os"
	"path/filepath"
	"strings"
	"testing"

	"gnosis/internal/testutil"
)

func TestUnicodeDiscoveryRecoveryAndProposal(t *testing.T) {
	source := testutil.Repo(t)
	testCLI(t, source, "init", "--skills=false")
	testCLI(t, source, "package", "init", "sample", "--owner", "@test")
	prefix := "\u00e9quipe docs"
	filename := " caf\u00e9 \"note\"\t.md"
	testutil.Write(t, filepath.Join(source, "gnosis", "sample", filename), "---\ntype: Reference\n---\nOriginal\n")
	if err := os.Rename(filepath.Join(source, "gnosis"), filepath.Join(source, prefix)); err != nil {
		t.Fatal(err)
	}
	testCommit(t, source, "non-ASCII paths")
	consumer := testutil.Repo(t)
	testCLI(t, consumer, "init", "--skills=false")
	testCLI(t, consumer, "registry", "add", source, "--path", prefix)
	testCommit(t, consumer, "registry")
	testCLI(t, consumer, "add", "sample")
	file := filepath.Join(consumer, "gnosis", "sample", filename)
	testutil.Write(t, file, "---\ntype: Reference\n---\nLocal\n")
	testCommit(t, consumer, "local edit")
	testutil.Write(t, filepath.Join(source, prefix, "sample", filename), "---\ntype: Reference\n---\nUpstream\n")
	testCommit(t, source, "upstream edit")
	_, failure, code := testCLIResult(consumer, "pull", "sample")
	if code != 1 || !strings.Contains(failure, "operation retained") {
		t.Fatalf("expected Unicode-path conflict: %s", failure)
	}
	testutil.Write(t, file, "---\ntype: Reference\n---\nResolved\n")
	testutil.Git(t, consumer, "add", "--", "gnosis/sample/"+filename)
	testCLI(t, consumer, "pull", "--continue")
	output := testCLI(t, consumer, "propose", "sample")
	proposal := strings.TrimPrefix(strings.Split(output, "\n")[0], "Proposal checkout: ")
	changes := testutil.Git(t, proposal, "diff", "--name-only", "-z", "origin/main", "HEAD")
	if want := prefix + "/sample/" + filename + "\x00"; changes != want {
		t.Fatalf("proposal changed wrong paths: got %q want %q", changes, want)
	}
	if content := testutil.Read(t, filepath.Join(proposal, prefix, "sample", filename)); !strings.Contains(content, "Resolved") {
		t.Fatalf("proposal omitted resolved changes: %s", content)
	}
}
