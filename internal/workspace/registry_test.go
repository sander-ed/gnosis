package workspace

import (
	"path/filepath"
	"strings"
	"testing"

	"gnosis/internal/knowledge"
	"gnosis/internal/testutil"
)

func TestRegistryWriteLimitsPreserveExistingData(t *testing.T) {
	dir := t.TempDir()
	w := Workspace{Dir: dir}
	if err := knowledge.WriteData(filepath.Join(dir, "registry.yml"), knowledge.Registry{}); err != nil {
		t.Fatal(err)
	}
	entry := knowledge.RegistryEntry{
		Description: strings.Repeat("x", knowledge.MaxDocumentSize/2), Owner: "@test",
		Source: knowledge.Source{Type: "git", Repository: dir, Path: ".", DefaultBranch: "main"},
	}
	if err := MergeRegistry(w, knowledge.Registry{"first": entry}); err != nil {
		t.Fatal(err)
	}
	before := testutil.Read(t, filepath.Join(dir, "registry.yml"))
	if err := MergeRegistry(w, knowledge.Registry{"second": entry}); err == nil || !strings.Contains(err.Error(), "16 MiB") {
		t.Fatalf("oversized merged registry: %v", err)
	}
	if after := testutil.Read(t, filepath.Join(dir, "registry.yml")); after != before {
		t.Fatal("rejected registry merge changed existing data")
	}
	if _, err := knowledge.ReadRegistry(dir); err != nil {
		t.Fatal(err)
	}
}

func TestRegistryEntryValidation(t *testing.T) {
	valid := knowledge.RegistryEntry{
		Owner:  "@test",
		Source: knowledge.Source{Type: "git", Repository: t.TempDir(), Path: ".", DefaultBranch: "main"},
	}
	for _, tc := range []struct {
		name  string
		entry knowledge.RegistryEntry
	}{
		{name: "INVALID", entry: valid},
		{name: "bad-owner", entry: knowledge.RegistryEntry{Owner: "test", Source: valid.Source}},
		{name: "bad-source", entry: knowledge.RegistryEntry{Owner: valid.Owner}},
	} {
		t.Run(tc.name, func(t *testing.T) {
			dir := t.TempDir()
			file := filepath.Join(dir, "registry.yml")
			if err := knowledge.WriteData(file, knowledge.Registry{}); err != nil {
				t.Fatal(err)
			}
			invalid := knowledge.Registry{tc.name: tc.entry}
			mergeErr := MergeRegistry(Workspace{Dir: dir}, invalid)
			if err := knowledge.WriteData(file, invalid); err != nil {
				t.Fatal(err)
			}
			_, readErr := knowledge.ReadRegistry(dir)
			if mergeErr == nil || readErr == nil || mergeErr.Error() != readErr.Error() {
				t.Fatalf("inconsistent validation: merge=%v read=%v", mergeErr, readErr)
			}
		})
	}
}
