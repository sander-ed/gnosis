package main

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func testWrite(t *testing.T, file, content string) {
	t.Helper()
	if err := os.MkdirAll(filepath.Dir(file), 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(file, []byte(content), 0o644); err != nil {
		t.Fatal(err)
	}
}

func testRead(t *testing.T, file string) string {
	t.Helper()
	data, err := os.ReadFile(file)
	if err != nil {
		t.Fatal(err)
	}
	return string(data)
}

func testManifest(name string, deps ...string) manifest {
	return manifest{
		GnosisVersion: 1, Package: name, Version: "1.0.0", Description: name + " description",
		Owner: owner{Team: "@example/owners"}, Dependencies: append([]string{}, deps...),
	}
}

func testPackage(t *testing.T) string {
	t.Helper()
	dir := filepath.Join(t.TempDir(), "sample")
	if err := scaffoldPackage(dir, testManifest("sample")); err != nil {
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
			testWrite(t, filepath.Join(dir, name), data)
			if _, err := readManifest(dir); err != nil {
				t.Fatal(err)
			}
			testWrite(t, filepath.Join(dir, "package.yml"), "gnosis_version: 1\npackage: test\nunknown: bad\n")
			if _, err := readManifest(dir); err == nil {
				t.Fatal("accepted unknown fields or ambiguous manifests")
			}
		})
	}
	for _, input := range []string{
		"package: one\npackage: two\n",
		"gnosis_version: 1\n---\ngnosis_version: 2\n",
		`{"gnosis_version":1,"gnosis_version":2}`,
	} {
		var m manifest
		if err := decode([]byte(input), &m); err == nil {
			t.Fatalf("accepted ambiguous metadata %q", input)
		}
	}
}

func TestOwner(t *testing.T) {
	for _, o := range []owner{
		{}, {Team: "team"}, {User: "@a", Team: "@org/team"}, {User: "@org/team"},
		{User: "@a @b"}, {User: "@a\n* @b"},
	} {
		if _, err := o.handle(); err == nil {
			t.Errorf("accepted invalid owner %+v", o)
		}
	}
	for _, o := range []owner{{User: "@a"}, {Team: "@org/team"}} {
		if _, err := o.handle(); err != nil {
			t.Error(err)
		}
	}
}

func TestOKFAndDynamicPolicy(t *testing.T) {
	dir := testPackage(t)
	file := filepath.Join(dir, "concept.md")
	doc := "---\ntype: Unregistered Type\ncustom: anything\nverified: {by: 'human:someone'}\n---\n# Summary\n\none two\n"
	testWrite(t, file, doc)
	if _, _, _, err := scanPackage(dir, true); err != nil {
		t.Fatal(err)
	}
	policy := standard{
		GnosisVersion: 1, Schema: "frontmatter.schema.json", IndexTemplate: "index.tmpl",
		RequiredHeads: []string{"# Summary"}, MaxWords: 4, RequiredFiles: []string{},
	}
	if err := writeData(filepath.Join(dir, "standard.yml"), policy); err != nil {
		t.Fatal(err)
	}
	if _, _, _, err := scanPackage(dir, true); err != nil {
		t.Fatalf("exact word limit should pass: %v", err)
	}
	testWrite(t, file, doc+"three")
	if _, _, _, err := scanPackage(dir, true); err == nil || !strings.Contains(err.Error(), "max_words 4") {
		t.Fatalf("over-limit result: %v", err)
	}
	testWrite(t, file, strings.ReplaceAll(doc, "# Summary", "# Other"))
	if _, _, _, err := scanPackage(dir, true); err == nil || !strings.Contains(err.Error(), "required heading") {
		t.Fatalf("missing-heading result: %v", err)
	}
	testWrite(t, file, doc)
	testWrite(t, filepath.Join(dir, "frontmatter.schema.json"),
		`{"type":"object","required":["title"],"properties":{"title":{"type":"string"}}}`,
	)
	if _, _, _, err := scanPackage(dir, true); err == nil || !strings.Contains(err.Error(), "title") {
		t.Fatalf("custom schema result: %v", err)
	}
	testWrite(t, filepath.Join(dir, "frontmatter.schema.json"), `{}`)
	testWrite(t, file, "---\ntitle: no type\n---\n# Summary\n")
	if _, _, _, err := scanPackage(dir, true); err == nil || !strings.Contains(err.Error(), "OKF requires") {
		t.Fatalf("custom schema must not remove OKF baseline: %v", err)
	}
}

func TestIndexGenerationAndPreservation(t *testing.T) {
	dir := testPackage(t)
	doc := "---\ntype: Reference\ntitle: 'A [title]'\ndescription: Summary\nunknown: preserved\n---\nText\n"
	file := filepath.Join(dir, "nested", "deep", "a file.md")
	testWrite(t, file, doc)
	if err := generateIndexes(dir, false); err != nil {
		t.Fatal(err)
	}
	root := testRead(t, filepath.Join(dir, "index.md"))
	if !strings.Contains(root, `okf_version: "0.2"`) || !strings.Contains(root, "[nested](nested/)") {
		t.Fatalf("bad root index: %s", root)
	}
	nested := testRead(t, filepath.Join(dir, "nested", "deep", "index.md"))
	if strings.HasPrefix(nested, "---") || !strings.Contains(nested, "a%20file.md") {
		t.Fatalf("bad nested index: %s", nested)
	}
	if !strings.Contains(nested, `A \[title\]`) {
		t.Fatalf("title was not escaped: %s", nested)
	}
	if got := testRead(t, file); got != doc {
		t.Fatal("index generation changed concept content or unknown metadata")
	}
	if err := generateIndexes(dir, false); err != nil {
		t.Fatal(err)
	}
	if next := testRead(t, filepath.Join(dir, "index.md")); next != root {
		t.Fatal("index generation is not deterministic")
	}
	testWrite(t, filepath.Join(dir, "index.md"), "# Manual index\n")
	if err := generateIndexes(dir, false); err == nil {
		t.Fatal("overwrote a manual index")
	}
	if err := generateIndexes(dir, true); err != nil {
		t.Fatal(err)
	}
	if err := os.Remove(file); err != nil {
		t.Fatal(err)
	}
	if err := generateIndexes(dir, false); err != nil {
		t.Fatal(err)
	}
	if text := testRead(t, filepath.Join(dir, "nested", "deep", "index.md")); strings.Contains(text, "a%20file.md") {
		t.Fatal("deleted concept remains in its directory index")
	}
}

func TestUnsafePathsAndSchemaReferences(t *testing.T) {
	for _, path := range []string{"../outside", "/absolute", "a/../../b", `a\b`, ".git/config", "x/.GIT/config"} {
		if err := validPath(path); err == nil {
			t.Errorf("accepted %q", path)
		}
	}
	dir := testPackage(t)
	testWrite(t, filepath.Join(dir, "concept.md"), "---\ntype: Reference\n---\nText\n")
	for _, ref := range []string{"https://example.invalid/schema.json", "../outside.json"} {
		testWrite(t, filepath.Join(dir, "frontmatter.schema.json"), `{"$ref":"`+ref+`"}`)
		if _, _, _, err := scanPackage(dir, true); err == nil {
			t.Fatalf("accepted external schema %s", ref)
		}
	}
	testWrite(t, filepath.Join(dir, "frontmatter.schema.json"), `{}`)
	if err := os.Symlink(t.TempDir(), filepath.Join(dir, "escape")); err != nil {
		t.Fatal(err)
	}
	if _, _, _, err := scanPackage(dir, true); err == nil {
		t.Fatal("accepted symlink in a package")
	}
	if _, err := safePath(dir, "escape/schema.json"); err == nil {
		t.Fatal("followed a symlink for a schema")
	}
}

func TestReservedFilesAndFrontmatter(t *testing.T) {
	for _, text := range []string{
		"body only", "---\ntype: Reference", "---\nnull\n---\nbody", "---\ntype: a\ntype: b\n---\n",
	} {
		if _, _, err := frontmatter([]byte(text)); err == nil {
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
