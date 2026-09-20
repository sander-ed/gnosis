package knowledge

import (
	"os"
	"path/filepath"
	"strings"
	"testing"

	"gnosis/internal/testutil"
)

func testManifest(name string, deps ...string) Manifest {
	return Manifest{
		GnosisVersion: 1, Package: name, Version: "1.0.0", Description: name + " description",
		Owner: Owner{Team: "@example/owners"}, Dependencies: append([]string{}, deps...),
	}
}

func testPackage(t *testing.T) string {
	t.Helper()
	dir := filepath.Join(t.TempDir(), "sample")
	if err := ScaffoldPackage(dir, testManifest("sample")); err != nil {
		t.Fatal(err)
	}
	return dir
}

func TestManifestFormats(t *testing.T) {
	for _, name := range []string{"package.yml", "package.yaml", "package.json"} {
		t.Run(name, func(t *testing.T) {
			dir := t.TempDir()
			data := `{"gnosis_version":1,"package":"test","version":"1.0.0",` +
				`"description":"test","owner":{"user":"@test"},"dependencies":[]}`
			testutil.Write(t, filepath.Join(dir, name), data)
			if _, err := ReadManifest(dir); err != nil {
				t.Fatal(err)
			}
			testutil.Write(t, filepath.Join(dir, "package.yml"), "gnosis_version: 1\npackage: test\nunknown: bad\n")
			if _, err := ReadManifest(dir); err == nil {
				t.Fatal("accepted unknown fields or ambiguous manifests")
			}
		})
	}
	for _, input := range []string{
		"package: one\npackage: two\n",
		"gnosis_version: 1\n---\ngnosis_version: 2\n",
		`{"gnosis_version":1,"gnosis_version":2}`,
	} {
		var m Manifest
		if err := Decode([]byte(input), &m); err == nil {
			t.Fatalf("accepted ambiguous metadata %q", input)
		}
	}
}

func TestOwner(t *testing.T) {
	for _, o := range []Owner{
		{}, {Team: "team"}, {User: "@a", Team: "@org/team"}, {User: "@org/team"},
		{User: "@a @b"}, {User: "@a\n* @b"},
	} {
		if _, err := o.Handle(); err == nil {
			t.Errorf("accepted invalid owner %+v", o)
		}
	}
	for _, o := range []Owner{{User: "@a"}, {Team: "@org/team"}} {
		if _, err := o.Handle(); err != nil {
			t.Error(err)
		}
	}
}

func TestOKFAndDynamicPolicy(t *testing.T) {
	dir := testPackage(t)
	file := filepath.Join(dir, "concept.md")
	doc := "---\ntype: Unregistered Type\ncustom: anything\nverified: {by: 'human:someone'}\n---\n# Summary\n\none two\n"
	testutil.Write(t, file, doc)
	if _, err := ValidatePackage(dir); err != nil {
		t.Fatal(err)
	}
	policy := standard{
		GnosisVersion: 1, Schema: "frontmatter.schema.json", IndexTemplate: "index.tmpl",
		RequiredHeads: []string{"# Summary"}, MaxWords: 4, RequiredFiles: []string{},
	}
	if err := WriteData(filepath.Join(dir, "standard.yml"), policy); err != nil {
		t.Fatal(err)
	}
	if _, err := ValidatePackage(dir); err != nil {
		t.Fatalf("exact word limit should pass: %v", err)
	}
	testutil.Write(t, file, doc+"three")
	if _, err := ValidatePackage(dir); err == nil || !strings.Contains(err.Error(), "max_words 4") {
		t.Fatalf("over-limit result: %v", err)
	}
	testutil.Write(t, file, strings.ReplaceAll(doc, "# Summary", "# Other"))
	if _, err := ValidatePackage(dir); err == nil || !strings.Contains(err.Error(), "required heading") {
		t.Fatalf("missing-heading result: %v", err)
	}
	testutil.Write(t, file, doc)
	testutil.Write(t, filepath.Join(dir, "frontmatter.schema.json"),
		`{"type":"object","required":["title"],"properties":{"title":{"type":"string"}}}`,
	)
	if _, err := ValidatePackage(dir); err == nil || !strings.Contains(err.Error(), "title") {
		t.Fatalf("custom schema result: %v", err)
	}
	testutil.Write(t, filepath.Join(dir, "frontmatter.schema.json"), `{}`)
	testutil.Write(t, file, "---\ntitle: no type\n---\n# Summary\n")
	if _, err := ValidatePackage(dir); err == nil || !strings.Contains(err.Error(), "OKF requires") {
		t.Fatalf("custom schema must not remove OKF baseline: %v", err)
	}
}

func TestIndexGenerationAndPreservation(t *testing.T) {
	dir := testPackage(t)
	doc := "---\ntype: Reference\ntitle: 'A [title]'\ndescription: Summary\nunknown: preserved\n---\nText\n"
	file := filepath.Join(dir, "nested", "deep", "a file.md")
	testutil.Write(t, file, doc)
	if err := GenerateIndexes(dir, false); err != nil {
		t.Fatal(err)
	}
	root := testutil.Read(t, filepath.Join(dir, "index.md"))
	if !strings.Contains(root, `okf_version: "0.2"`) || !strings.Contains(root, "[nested](nested/)") {
		t.Fatalf("bad root index: %s", root)
	}
	nested := testutil.Read(t, filepath.Join(dir, "nested", "deep", "index.md"))
	if strings.HasPrefix(nested, "---") || !strings.Contains(nested, "a%20file.md") {
		t.Fatalf("bad nested index: %s", nested)
	}
	if !strings.Contains(nested, `A \[title\]`) {
		t.Fatalf("title was not escaped: %s", nested)
	}
	if got := testutil.Read(t, file); got != doc {
		t.Fatal("index generation changed concept content or unknown metadata")
	}
	if err := GenerateIndexes(dir, false); err != nil {
		t.Fatal(err)
	}
	if next := testutil.Read(t, filepath.Join(dir, "index.md")); next != root {
		t.Fatal("index generation is not deterministic")
	}
	testutil.Write(t, filepath.Join(dir, "index.md"), "# Manual index\n")
	if err := GenerateIndexes(dir, false); err == nil {
		t.Fatal("overwrote a manual index")
	}
	if err := GenerateIndexes(dir, true); err != nil {
		t.Fatal(err)
	}
	if err := os.Remove(file); err != nil {
		t.Fatal(err)
	}
	if err := GenerateIndexes(dir, false); err != nil {
		t.Fatal(err)
	}
	if text := testutil.Read(t, filepath.Join(dir, "nested", "deep", "index.md")); strings.Contains(text, "a%20file.md") {
		t.Fatal("deleted concept remains in its directory index")
	}
}

func TestUnsafePathsAndSchemaReferences(t *testing.T) {
	for _, path := range []string{"../outside", "/absolute", "a/../../b", `a\b`, ".git/config", "x/.GIT/config"} {
		if err := ValidatePath(path); err == nil {
			t.Errorf("accepted %q", path)
		}
	}
	dir := testPackage(t)
	testutil.Write(t, filepath.Join(dir, "concept.md"), "---\ntype: Reference\n---\nText\n")
	for _, ref := range []string{"https://example.invalid/schema.json", "../outside.json"} {
		testutil.Write(t, filepath.Join(dir, "frontmatter.schema.json"), `{"$ref":"`+ref+`"}`)
		if _, err := ValidatePackage(dir); err == nil {
			t.Fatalf("accepted external schema %s", ref)
		}
	}
	testutil.Write(t, filepath.Join(dir, "frontmatter.schema.json"), `{}`)
	if err := os.Symlink(t.TempDir(), filepath.Join(dir, "escape")); err != nil {
		t.Fatal(err)
	}
	if _, err := ValidatePackage(dir); err == nil {
		t.Fatal("accepted symlink in a package")
	}
	if _, err := SafePath(dir, "escape/schema.json"); err == nil {
		t.Fatal("followed a symlink for a schema")
	}
}

func TestReservedFilesAndFrontmatter(t *testing.T) {
	for _, text := range []string{
		"body only", "---\ntype: Reference", "---\nnull\n---\nbody", "---\ntype: a\ntype: b\n---\n",
	} {
		if _, _, err := Frontmatter([]byte(text)); err == nil {
			t.Errorf("accepted %q", text)
		}
	}
	for _, test := range []struct{ path, content string }{
		{path: "nested/index.md", content: "---\nokf_version: '0.2'\n---\n"},
		{path: "index.md", content: "---\ntitle: no\n---\n"},
		{path: "log.md", content: "## Yesterday\n"},
	} {
		if err := validateReserved(test.path, []byte(test.content)); err == nil {
			t.Errorf("accepted invalid reserved file %s", test.path)
		}
	}
}

func TestIndexTemplateValidation(t *testing.T) {
	for _, kind := range []string{"missing", "directory", "symlink", "syntax"} {
		t.Run(kind, func(t *testing.T) {
			dir := testPackage(t)
			file := filepath.Join(dir, "index.tmpl")
			if err := os.Remove(file); err != nil {
				t.Fatal(err)
			}
			switch kind {
			case "directory":
				if err := os.Mkdir(file, 0o755); err != nil {
					t.Fatal(err)
				}
			case "symlink":
				if err := os.Symlink(filepath.Join(dir, "standard.yml"), file); err != nil {
					t.Fatal(err)
				}
			case "syntax":
				testutil.Write(t, file, "{{if .Root}}")
			}
			if _, err := ValidatePackage(dir); err == nil || !strings.Contains(err.Error(), "index.tmpl") {
				t.Fatalf("invalid template accepted by validation: %v", err)
			}
			if err := GenerateIndexes(dir, false); err == nil {
				t.Fatal("invalid template accepted by indexing")
			}
		})
	}
}
