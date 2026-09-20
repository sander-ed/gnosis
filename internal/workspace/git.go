package workspace

import (
	"bytes"
	"context"
	"errors"
	"fmt"
	"os"
	"os/exec"
	"strings"
	"time"

	"gnosis/internal/knowledge"
)

func GitCommand(ctx context.Context, dir string, args ...string) *exec.Cmd {
	args = append([]string{"--no-pager", "-c", "protocol.ext.allow=never", "-C", dir}, args...)
	cmd := exec.CommandContext(ctx, "git", args...)
	cmd.Env = append(os.Environ(), "GIT_TERMINAL_PROMPT=0", "GIT_ALLOW_PROTOCOL=file:ssh:https:http")
	return cmd
}

func git(ctx context.Context, dir string, args ...string) (string, error) {
	output, err := gitOutput(ctx, dir, args...)
	return strings.TrimSpace(string(output)), err
}

type commandOutput struct {
	buffer   bytes.Buffer
	cancel   context.CancelFunc
	exceeded bool
}

func (b *commandOutput) Write(data []byte) (int, error) {
	if len(data) > knowledge.MaxDocumentSize-b.buffer.Len() {
		b.exceeded = true
		b.cancel()
		return 0, errors.New("command output exceeds 16 MiB")
	}
	return b.buffer.Write(data)
}

func gitOutput(ctx context.Context, dir string, args ...string) ([]byte, error) {
	ctx, cancel := context.WithTimeout(ctx, 5*time.Minute)
	defer cancel()
	processCtx, stop := context.WithCancel(ctx)
	defer stop()
	cmd := GitCommand(processCtx, dir, args...)
	manageProcess(cmd)
	stdout := commandOutput{cancel: stop}
	stderr := commandOutput{cancel: stop}
	cmd.Stdout = &stdout
	cmd.Stderr = &stderr
	err := cmd.Run()
	if ctx.Err() != nil {
		return nil, ctx.Err()
	}
	if stdout.exceeded || stderr.exceeded {
		return nil, fmt.Errorf("git %s: command output exceeds 16 MiB", strings.Join(args, " "))
	}
	if err != nil {
		return nil, fmt.Errorf("git %s: %w\n%s", strings.Join(args, " "), err, strings.TrimSpace(stderr.buffer.String()))
	}
	return stdout.buffer.Bytes(), nil
}

func gitPaths(ctx context.Context, dir string, args ...string) ([]string, error) {
	command := append([]string{args[0], "-z"}, args[1:]...)
	output, err := gitOutput(ctx, dir, command...)
	if err != nil {
		return nil, err
	}
	if len(output) == 0 {
		return []string{}, nil
	}
	return strings.Split(strings.TrimSuffix(string(output), "\x00"), "\x00"), nil
}

func cloneSource(ctx context.Context, s knowledge.Source, parent, pinned string) (string, error) {
	if err := s.Validate(); err != nil {
		return "", err
	}
	if pinned == "" {
		if _, err := git(ctx, parent, "check-ref-format", "refs/heads/"+s.DefaultBranch); err != nil {
			return "", fmt.Errorf("invalid default_branch: %w", err)
		}
	}
	dir, err := os.MkdirTemp(parent, "source-")
	if err != nil {
		return "", err
	}
	args := []string{"clone", "--quiet", "--no-checkout", "--filter=blob:none", "--no-tags"}
	if pinned == "" {
		args = append(args, "--single-branch", "--branch", s.DefaultBranch)
	}
	args = append(args, "--", s.Repository, dir)
	_, err = git(ctx, parent, args...)
	return dir, err
}
