package workspace

import (
	"archive/tar"
	"bytes"
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
