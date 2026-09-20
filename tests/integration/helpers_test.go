package integration

import (
	"context"
	"io"
	"os"
	"path/filepath"
	"testing"

	"gnosis/internal/cli"
	"gnosis/internal/knowledge"
)

func testManifest(name string, deps ...string) knowledge.Manifest {
	return knowledge.Manifest{
		GnosisVersion: 1, Package: name, Version: "1.0.0", Description: name + " description",
		Owner: knowledge.Owner{Team: "@example/owners"}, Dependencies: append([]string{}, deps...),
	}
}

func testPackage(t *testing.T) string {
	t.Helper()
	dir := filepath.Join(t.TempDir(), "sample")
	if err := knowledge.ScaffoldPackage(dir, testManifest("sample")); err != nil {
		t.Fatal(err)
	}
	return dir
}

func run(args []string, stdout, stderr io.Writer) int {
	return cli.Execute(context.Background(), args, cli.Streams{In: os.Stdin, Out: stdout, Err: stderr}, "dev")
}
