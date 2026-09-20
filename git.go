package main

import (
	"archive/tar"
	"bytes"
	"context"
	"errors"
	"fmt"
	"io"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"time"
)

type workspace struct {
	root   string
	dir    string
	gitDir string
}

type gitExitError struct {
	code int
	err  error
}

func (e *gitExitError) Error() string { return e.err.Error() }
func (e *gitExitError) Unwrap() error { return e.err }

func gitCommand(ctx context.Context, dir string, args ...string) *exec.Cmd {
	args = append([]string{"--no-pager", "-c", "protocol.ext.allow=never", "-C", dir}, args...)
	cmd := exec.CommandContext(ctx, "git", args...)
	cmd.Env = append(os.Environ(), "GIT_TERMINAL_PROMPT=0", "GIT_ALLOW_PROTOCOL=file:ssh:https:http")
	return cmd
}

func git(ctx context.Context, dir string, args ...string) (string, error) {
	ctx, cancel := context.WithTimeout(ctx, 5*time.Minute)
	defer cancel()
	cmd := gitCommand(ctx, dir, args...)
	var stderr bytes.Buffer
	cmd.Stderr = &stderr
	output, err := cmd.Output()
	if err != nil {
		if ctx.Err() != nil {
			return "", ctx.Err()
		}
		return "", fmt.Errorf("git %s: %w\n%s", strings.Join(args, " "), err, strings.TrimSpace(stderr.String()))
	}
	return strings.TrimSpace(string(output)), nil
}

func openWorkspace(ctx context.Context, cwd string) (workspace, error) {
	root, err := git(ctx, cwd, "rev-parse", "--show-toplevel")
	if err != nil {
		return workspace{}, fmt.Errorf("run inside a Git repository: %w", err)
	}
	gitDir, err := git(ctx, root, "rev-parse", "--absolute-git-dir")
	if err != nil {
		return workspace{}, err
	}
	dir, err := safePath(root, "gnosis")
	if err != nil {
		return workspace{}, err
	}
	return workspace{root: root, dir: dir, gitDir: gitDir}, nil
}

func (w workspace) initialized() error {
	var config struct {
		GnosisVersion int `yaml:"gnosis_version"`
	}
	if err := readData(filepath.Join(w.dir, "gnosis.yml"), &config); err != nil {
		return fmt.Errorf("knowledge base is not initialized; run gnosis init: %w", err)
	}
	if config.GnosisVersion != 1 {
		return fmt.Errorf("unsupported gnosis_version %d", config.GnosisVersion)
	}
	return nil
}

func (w workspace) clean(ctx context.Context) error {
	status, err := git(ctx, w.root, "status", "--porcelain", "--untracked-files=all")
	if err != nil {
		return err
	}
	if status != "" {
		return errors.New("commit or remove pending changes before this operation; gnosis never stashes or discards them")
	}
	if _, err := git(ctx, w.root, "rev-parse", "--verify", "HEAD"); err != nil {
		return errors.New("create an initial Git commit before importing packages")
	}
	for _, state := range []string{"MERGE_HEAD", "CHERRY_PICK_HEAD", "REVERT_HEAD", "rebase-merge", "rebase-apply"} {
		_, err := os.Lstat(filepath.Join(w.gitDir, state))
		if err == nil {
			return fmt.Errorf("finish the active Git operation (%s) first", state)
		}
		if !errors.Is(err, os.ErrNotExist) {
			return err
		}
	}
	for _, field := range []string{"user.name", "user.email"} {
		value, err := git(ctx, w.root, "config", "--get", field)
		if err != nil || value == "" {
			return fmt.Errorf("configure git %s before creating subtree commits", field)
		}
	}
	return nil
}

func (w workspace) acquire() (func() error, error) {
	state := filepath.Join(w.gitDir, "gnosis")
	if err := os.MkdirAll(state, 0o700); err != nil {
		return nil, err
	}
	file := filepath.Join(state, "operation.lock")
	f, err := os.OpenFile(file, os.O_WRONLY|os.O_CREATE|os.O_EXCL, 0o600)
	if err != nil {
		return nil, fmt.Errorf(
			"another gnosis operation may be active; inspect %s before removing a stale lock: %w", file, err,
		)
	}
	if _, err := fmt.Fprintln(f, os.Getpid()); err != nil {
		return nil, errors.Join(err, f.Close(), os.Remove(file))
	}
	if err := f.Close(); err != nil {
		return nil, errors.Join(err, os.Remove(file))
	}
	return func() error { return os.Remove(file) }, nil
}

func (w workspace) pendingPath() string {
	return filepath.Join(w.gitDir, "gnosis", "pending.yml")
}

func (w workspace) noPending() error {
	_, err := os.Lstat(w.pendingPath())
	if err == nil {
		return errors.New("a package update is pending; use gnosis pull --continue or --abort")
	}
	if !errors.Is(err, os.ErrNotExist) {
		return err
	}
	return nil
}

func cloneSource(ctx context.Context, s source, parent string) (string, error) {
	if err := s.validate(); err != nil {
		return "", err
	}
	if _, err := git(ctx, parent, "check-ref-format", "refs/heads/"+s.DefaultBranch); err != nil {
		return "", fmt.Errorf("invalid default_branch: %w", err)
	}
	dir, err := os.MkdirTemp(parent, "source-")
	if err != nil {
		return "", err
	}
	_, err = git(ctx, parent,
		"clone", "--quiet", "--no-checkout", "--filter=blob:none", "--no-tags",
		"--single-branch", "--branch", s.DefaultBranch, "--", s.Repository, dir,
	)
	return dir, err
}

// Export only regular files from the selected tree; do not follow repository symlinks.
func exportTree(ctx context.Context, repo, tree, destination string) error {
	ctx, cancel := context.WithTimeout(ctx, 5*time.Minute)
	defer cancel()
	modes, err := git(ctx, repo, "ls-tree", "-r", tree)
	if err != nil {
		return err
	}
	for _, entry := range strings.Split(modes, "\n") {
		if entry != "" && !strings.HasPrefix(entry, "100644 ") && !strings.HasPrefix(entry, "100755 ") {
			return fmt.Errorf("package contains a symlink, submodule or special entry: %s", entry)
		}
	}
	// A blank attribute worktree prevents export-ignore/export-subst from hiding
	// or rewriting files that git subtree will actually import.
	attributeDir, err := os.MkdirTemp(destination, ".attributes-")
	if err != nil {
		return err
	}
	cmd := gitCommand(ctx, repo,
		"-c", "core.attributesFile="+os.DevNull,
		"archive", "--worktree-attributes", "--format=tar", tree,
	)
	cmd.Env = append(cmd.Env, "GIT_WORK_TREE="+attributeDir)
	var stderr bytes.Buffer
	cmd.Stderr = &stderr
	stdout, err := cmd.StdoutPipe()
	if err != nil {
		return err
	}
	if err := cmd.Start(); err != nil {
		return err
	}
	extractErr := extractTree(tar.NewReader(stdout), destination)
	if extractErr != nil {
		cancel()
	}
	waitErr := cmd.Wait()
	removeErr := os.Remove(attributeDir)
	if extractErr != nil {
		return errors.Join(extractErr, removeErr)
	}
	if waitErr != nil {
		return errors.Join(fmt.Errorf("git archive: %w: %s", waitErr, stderr.String()), removeErr)
	}
	return removeErr
}

func extractTree(reader *tar.Reader, destination string) error {
	var total int64
	for {
		header, err := reader.Next()
		if errors.Is(err, io.EOF) {
			return nil
		}
		if err != nil {
			return err
		}
		name := strings.TrimSuffix(header.Name, "/")
		if err := validPath(name); err != nil {
			return err
		}
		file := filepath.Join(destination, filepath.FromSlash(name))
		switch header.Typeflag {
		case tar.TypeXGlobalHeader:
			continue
		case tar.TypeDir:
			if err := os.MkdirAll(file, 0o755); err != nil {
				return err
			}
		case tar.TypeReg:
			total += header.Size
			if header.Size > 16<<20 || total > 256<<20 {
				return errors.New("package exceeds 16 MiB per file or 256 MiB total")
			}
			if err := os.MkdirAll(filepath.Dir(file), 0o755); err != nil {
				return err
			}
			mode := os.FileMode(0o644)
			if header.Mode&0o111 != 0 {
				mode = 0o755
			}
			f, err := os.OpenFile(file, os.O_WRONLY|os.O_CREATE|os.O_EXCL, mode)
			if err != nil {
				return err
			}
			_, copyErr := io.CopyN(f, reader, header.Size)
			if err := errors.Join(copyErr, f.Close()); err != nil {
				return err
			}
		default:
			return fmt.Errorf("%s: symlinks and special files are not supported", name)
		}
	}
}

func subtreeTree(commit string, prefix string) string {
	if prefix == "." {
		return commit
	}
	return commit + ":" + prefix
}
