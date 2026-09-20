package integration

import (
	"encoding/json"
	"path/filepath"
	"strings"
	"testing"

	"gnosis/internal/knowledge"
	"gnosis/internal/testutil"
)

func TestDiscoveryRejectsOversizedMetadata(t *testing.T) {
	for _, name := range []string{"registry.json", "sample/package.json"} {
		t.Run(name, func(t *testing.T) {
			sourceRepo := testutil.Repo(t)
			description := strings.Repeat("x", knowledge.MaxDocumentSize)
			var value any
			if strings.HasPrefix(name, "sample/") {
				m := testManifest("sample")
				m.Description = description
				value = m
			} else {
				value = knowledge.Registry{"sample": {
					Description: description, Owner: "@test",
					Source: knowledge.Source{Type: "git", Repository: sourceRepo, Path: ".", DefaultBranch: "main"},
				}}
			}
			data, err := json.Marshal(value)
			if err != nil {
				t.Fatal(err)
			}
			testutil.Write(t, filepath.Join(sourceRepo, "gnosis", name), string(data))
			testCommit(t, sourceRepo, "oversized metadata")
			consumer := testutil.Repo(t)
			testCLI(t, consumer, "init", "--skills=false")
			before := testutil.Read(t, filepath.Join(consumer, "gnosis", "registry.yml"))
			_, failure, code := testCLIResult(consumer, "registry", "add", sourceRepo)
			if code != 1 || !strings.Contains(failure, "output exceeds 16 MiB") {
				t.Fatalf("oversized discovery exit=%d: %s", code, failure)
			}
			if after := testutil.Read(t, filepath.Join(consumer, "gnosis", "registry.yml")); after != before {
				t.Fatal("oversized discovery changed the registry")
			}
			testCLI(t, consumer, "add", "--json")
		})
	}
}
