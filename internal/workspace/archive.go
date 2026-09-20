package workspace

import (
	"archive/tar"
	"bytes"
	"context"
	"errors"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"strings"
	"time"

	"gnosis/internal/knowledge"
)

// Export only regular files from the selected tree; do not follow repository symlinks.
func exportTree(ctx context.Context, repo, tree, destination string) (err error) {
	ctx, cancel := context.WithTimeout(ctx, 5*time.Minute)
	defer cancel()
	modes, err := gitPaths(ctx, repo, "ls-tree", "-r", tree)
	if err != nil {
		return err
	}
	for _, entry := range modes {
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
	defer func() { err = errors.Join(err, os.Remove(attributeDir)) }()
	processCtx, stop := context.WithCancel(ctx)
	defer stop()
	cmd := GitCommand(processCtx, repo,
		"-c", "core.attributesFile="+os.DevNull,
		"archive", "--worktree-attributes", "--format=tar", tree,
	)
	manageProcess(cmd)
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
	stopRead := context.AfterFunc(processCtx, func() { stdout.Close() })
	defer stopRead()
	extractErr := extractTree(tar.NewReader(stdout), destination)
	if extractErr != nil {
		stop()
	}
	waitErr := cmd.Wait()
	if ctx.Err() != nil {
		return ctx.Err()
	}
	if extractErr != nil {
		return extractErr
	}
	if waitErr != nil {
		return fmt.Errorf("git archive: %w: %s", waitErr, stderr.String())
	}
	return nil
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
		if err := knowledge.ValidatePath(name); err != nil {
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
			if header.Size > knowledge.MaxDocumentSize || total > 256<<20 {
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
