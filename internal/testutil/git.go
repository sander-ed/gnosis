package testutil

import (
	"os"
	"os/exec"
	"strings"
	"testing"
)

func Git(t *testing.T, dir string, args ...string) string {
	t.Helper()
	cmd := exec.Command("git", append([]string{"-C", dir}, args...)...)
	data, err := cmd.CombinedOutput()
	if err != nil {
		t.Fatalf("git %v: %v\n%s", args, err, data)
	}
	return strings.TrimSpace(string(data))
}

func Repo(t *testing.T) string {
	t.Helper()
	t.Setenv("GIT_CONFIG_GLOBAL", os.DevNull)
	t.Setenv("GIT_CONFIG_NOSYSTEM", "1")
	dir := t.TempDir()
	Git(t, dir, "init", "--quiet", "-b", "main")
	Git(t, dir, "config", "user.name", "Gnosis Test")
	Git(t, dir, "config", "user.email", "test@example.invalid")
	Git(t, dir, "config", "commit.gpgsign", "false")
	Git(t, dir, "config", "core.hooksPath", "/dev/null")
	return dir
}
