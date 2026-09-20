package knowledge

import (
	"bytes"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestDocumentSizeLimits(t *testing.T) {
	file := filepath.Join(t.TempDir(), "document")
	exact := bytes.Repeat([]byte("x"), MaxDocumentSize)
	if err := WriteAtomic(file, exact); err != nil {
		t.Fatal(err)
	}
	data, err := ReadRegular(file)
	if err != nil || len(data) != MaxDocumentSize {
		t.Fatalf("exact limit rejected: size=%d err=%v", len(data), err)
	}
	oversized := append(exact, 'x')
	if err := WriteAtomic(file, oversized); err == nil || !strings.Contains(err.Error(), "16 MiB") {
		t.Fatalf("oversized write: %v", err)
	}
	if data, err := ReadRegular(file); err != nil || len(data) != MaxDocumentSize {
		t.Fatalf("rejected write changed existing document: size=%d err=%v", len(data), err)
	}
	if err := os.WriteFile(file, oversized, 0o644); err != nil {
		t.Fatal(err)
	}
	if _, err := ReadRegular(file); err == nil || !strings.Contains(err.Error(), "16 MiB") {
		t.Fatalf("oversized read: %v", err)
	}
}
