//go:build unix

package integration

import (
	"bytes"
	"context"
	"os"
	"os/exec"
	"path/filepath"
	"testing"
	"time"

	"gnosis/internal/cli"
	"gnosis/internal/testutil"
	"gnosis/internal/workspace"
)

func TestArchiveCancellationExitAndLockRelease(t *testing.T) {
	source := testSource(t)
	consumer := testConsumer(t, source)
	realGit, err := exec.LookPath("git")
	if err != nil {
		t.Fatal(err)
	}
	bin := t.TempDir()
	wrapper := filepath.Join(bin, "git")
	testutil.Write(t, wrapper, "#!/bin/sh\n"+
		"for arg in \"$@\"; do\n"+
		"if [ \"$arg\" = archive ]; then\n"+testutil.BlockingChild+"exit 0\nfi\ndone\n"+
		"exec \"$GNOSIS_REAL_GIT\" \"$@\"\n",
	)
	if err := os.Chmod(wrapper, 0o755); err != nil {
		t.Fatal(err)
	}
	t.Setenv("GNOSIS_REAL_GIT", realGit)
	t.Setenv("PATH", bin+string(os.PathListSeparator)+os.Getenv("PATH"))
	marker := filepath.Join(t.TempDir(), "child")
	t.Setenv("GNOSIS_CHILD_PID", marker)
	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()
	var stdout, stderr bytes.Buffer
	done := make(chan int, 1)
	go func() {
		done <- cli.Execute(ctx, []string{"-C", consumer, "add", "risk"}, cli.Streams{Out: &stdout, Err: &stderr}, "dev")
	}()
	pid := testutil.WaitForChild(t, marker)
	cancel()
	select {
	case code := <-done:
		if code != 130 {
			t.Fatalf("active archive cancellation exit=%d: %s", code, stderr.String())
		}
	case <-time.After(5 * time.Second):
		t.Fatal("archive cancellation blocked")
	}
	testutil.AssertChildStopped(t, pid)
	w, err := workspace.Open(context.Background(), consumer)
	if err != nil {
		t.Fatal(err)
	}
	release, err := w.Acquire()
	if err != nil {
		t.Fatalf("cancelled operation retained its lock: %v", err)
	}
	if err := release(); err != nil {
		t.Fatal(err)
	}
	if err := w.NoPending(); err != nil {
		t.Fatal(err)
	}
}

func TestMergeHookCancellationRetainsRecovery(t *testing.T) {
	source := testSource(t)
	consumer := testConsumer(t, source)
	testCLI(t, consumer, "add", "risk")
	testutil.Write(t, filepath.Join(source, "gnosis", "risk", "new.md"), "---\ntype: Reference\n---\nUpdate\n")
	testCommit(t, source, "upstream update")
	hooks := t.TempDir()
	hook := filepath.Join(hooks, "pre-merge-commit")
	testutil.Write(t, hook, "#!/bin/sh\n"+testutil.BlockingChild)
	if err := os.Chmod(hook, 0o755); err != nil {
		t.Fatal(err)
	}
	testutil.Git(t, consumer, "config", "core.hooksPath", hooks)
	marker := filepath.Join(t.TempDir(), "child")
	t.Setenv("GNOSIS_CHILD_PID", marker)
	before := testutil.Read(t, filepath.Join(consumer, "gnosis", "gnosis.lock"))
	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()
	var stdout, stderr bytes.Buffer
	done := make(chan int, 1)
	go func() {
		done <- cli.Execute(ctx, []string{"-C", consumer, "pull", "risk"}, cli.Streams{Out: &stdout, Err: &stderr}, "dev")
	}()
	pid := testutil.WaitForChild(t, marker)
	cancel()
	select {
	case code := <-done:
		if code != 130 {
			t.Fatalf("active merge cancellation exit=%d: %s", code, stderr.String())
		}
	case <-time.After(5 * time.Second):
		t.Fatal("merge cancellation blocked on hook")
	}
	testutil.AssertChildStopped(t, pid)
	if after := testutil.Read(t, filepath.Join(consumer, "gnosis", "gnosis.lock")); after != before {
		t.Fatal("cancelled merge advanced the lock")
	}
	w, err := workspace.Open(context.Background(), consumer)
	if err != nil {
		t.Fatal(err)
	}
	if _, err := os.Stat(w.PendingPath()); err != nil {
		t.Fatalf("cancelled merge lost recovery metadata: %v", err)
	}
	release, err := w.Acquire()
	if err != nil {
		t.Fatalf("cancelled merge retained its operation lock: %v", err)
	}
	if err := release(); err != nil {
		t.Fatal(err)
	}
}
