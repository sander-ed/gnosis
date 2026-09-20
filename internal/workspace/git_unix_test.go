//go:build unix

package workspace

import (
	"context"
	"errors"
	"path/filepath"
	"testing"
	"time"

	"gnosis/internal/testutil"
)

func TestGitCancellationStopsDescendants(t *testing.T) {
	for _, timeout := range []bool{false, true} {
		name := "cancel"
		if timeout {
			name = "deadline"
		}
		t.Run(name, func(t *testing.T) {
			repo := testutil.Repo(t)
			testutil.Git(t, repo, "config", "alias.block", "!"+testutil.BlockingChild)
			marker := filepath.Join(t.TempDir(), "child")
			t.Setenv("GNOSIS_CHILD_PID", marker)
			ctx, cancel := context.WithTimeout(context.Background(), 2*time.Second)
			defer cancel()
			done := make(chan error, 1)
			go func() {
				_, err := git(ctx, repo, "block")
				done <- err
			}()
			pid := testutil.WaitForChild(t, marker)
			want := context.DeadlineExceeded
			if !timeout {
				cancel()
				want = context.Canceled
			}
			select {
			case err := <-done:
				if !errors.Is(err, want) {
					t.Fatalf("cancellation error=%v want %v", err, want)
				}
			case <-time.After(5 * time.Second):
				t.Fatal("cancellation blocked waiting for inherited pipes")
			}
			testutil.AssertChildStopped(t, pid)
		})
	}
}
