package integration

import (
	"os"
	"path/filepath"
	"strings"
	"testing"

	"gnosis/internal/testutil"
)

func TestMissingTemplatePreventsImport(t *testing.T) {
	source := testSource(t)
	consumer := testConsumer(t, source)
	if err := os.Remove(filepath.Join(source, "gnosis", "risk", "index.tmpl")); err != nil {
		t.Fatal(err)
	}
	testCommit(t, source, "missing template")
	_, failure, code := testCLIResult(source, "validate")
	if code != 1 || !strings.Contains(failure, "index.tmpl") {
		t.Fatalf("validation accepted missing template: %s", failure)
	}
	before := testutil.Git(t, consumer, "rev-parse", "HEAD")
	_, failure, code = testCLIResult(consumer, "add", "risk")
	if code != 1 || !strings.Contains(failure, "index.tmpl") {
		t.Fatalf("import accepted missing template: %s", failure)
	}
	if after := testutil.Git(t, consumer, "rev-parse", "HEAD"); after != before {
		t.Fatal("invalid template partially imported dependency closure")
	}
}
