// Package workspace coordinates repository state and Git-backed knowledge workflows.
package workspace

import (
	"bytes"
	"context"
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"strings"

	"gnosis/internal/knowledge"
)

type Workspace struct {
	Root   string
	Dir    string
	gitDir string
}

func Open(ctx context.Context, cwd string) (Workspace, error) {
	root, err := git(ctx, cwd, "rev-parse", "--show-toplevel")
	if err != nil {
		return Workspace{}, fmt.Errorf("run inside a Git repository: %w", err)
	}
	gitDir, err := git(ctx, root, "rev-parse", "--absolute-git-dir")
	if err != nil {
		return Workspace{}, err
	}
	dir, err := knowledge.SafePath(root, "gnosis")
	if err != nil {
		return Workspace{}, err
	}
	return Workspace{Root: root, Dir: dir, gitDir: gitDir}, nil
}

func (w Workspace) Initialized() error {
	var config struct {
		GnosisVersion int `yaml:"gnosis_version"`
	}
	if err := knowledge.ReadData(filepath.Join(w.Dir, "gnosis.yml"), &config); err != nil {
		return fmt.Errorf("knowledge base is not initialized; run gnosis init: %w", err)
	}
	if config.GnosisVersion != 1 {
		return fmt.Errorf("unsupported gnosis_version %d", config.GnosisVersion)
	}
	return nil
}

func (w Workspace) clean(ctx context.Context) error {
	status, err := git(ctx, w.Root, "status", "--porcelain", "--untracked-files=all")
	if err != nil {
		return err
	}
	if status != "" {
		return errors.New("commit or remove pending changes before this operation; gnosis never stashes or discards them")
	}
	if _, err := git(ctx, w.Root, "rev-parse", "--verify", "HEAD"); err != nil {
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
		value, err := git(ctx, w.Root, "config", "--get", field)
		if err != nil || value == "" {
			return fmt.Errorf("configure git %s before creating subtree commits", field)
		}
	}
	return nil
}

func (w Workspace) Acquire() (func() error, error) {
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

func (w Workspace) PendingPath() string {
	return filepath.Join(w.gitDir, "gnosis", "pending.yml")
}

func (w Workspace) NoPending() error {
	_, err := os.Lstat(w.PendingPath())
	if err == nil {
		return errors.New("a package update is pending; use gnosis pull --continue or --abort")
	}
	if !errors.Is(err, os.ErrNotExist) {
		return err
	}
	return nil
}

func Init(w Workspace, install bool) error {
	if _, err := os.Lstat(w.Dir); !errors.Is(err, os.ErrNotExist) {
		if err != nil {
			return err
		}
		return fmt.Errorf("%s already exists; initialization never overwrites it", w.Dir)
	}
	if install {
		if err := InstallSkills(w.Root, true); err != nil {
			return err
		}
	}
	if err := os.Mkdir(w.Dir, 0o755); err != nil {
		return err
	}
	if err := knowledge.WriteAtomic(filepath.Join(w.Dir, "gnosis.yml"), []byte("gnosis_version: 1\n")); err != nil {
		return err
	}
	if err := knowledge.WriteData(filepath.Join(w.Dir, "registry.yml"), knowledge.Registry{}); err != nil {
		return err
	}
	lock := knowledge.Lockfile{GnosisVersion: 1, Packages: map[string]knowledge.LockedPackage{}}
	if err := knowledge.WriteData(filepath.Join(w.Dir, "gnosis.lock"), lock); err != nil {
		return err
	}
	if err := GenerateIndex(w); err != nil {
		return err
	}
	if install {
		return InstallSkills(w.Root, false)
	}
	return nil
}

func Packages(w Workspace) (map[string]knowledge.Manifest, error) {
	entries, err := os.ReadDir(w.Dir)
	if err != nil {
		return nil, err
	}
	packages := map[string]knowledge.Manifest{}
	for _, entry := range entries {
		if entry.Type()&os.ModeSymlink != 0 {
			return nil, fmt.Errorf("%s: symlink is not allowed in the knowledge root", entry.Name())
		}
		if !entry.IsDir() {
			continue
		}
		if err := knowledge.ValidateName(entry.Name()); err != nil {
			return nil, err
		}
		m, err := knowledge.ReadManifest(filepath.Join(w.Dir, entry.Name()))
		if err != nil {
			return nil, err
		}
		if m.Package != entry.Name() {
			return nil, fmt.Errorf("%s: manifest package must match its directory", entry.Name())
		}
		packages[m.Package] = m
	}
	return packages, nil
}

func GenerateIndex(w Workspace) error {
	packages, err := Packages(w)
	if err != nil {
		return err
	}
	var text strings.Builder
	text.WriteString("<!-- Generated by gnosis; edit package descriptions instead. -->\n\n# Knowledge packages\n")
	for _, name := range knowledge.SortedKeys(packages) {
		m := packages[name]
		fmt.Fprintf(&text, "\n* [%s](%s/index.md) - %s\n",
			name, name, knowledge.MarkdownText(m.Description),
		)
	}
	file := filepath.Join(w.Dir, "index.md")
	old, err := knowledge.ReadRegular(file)
	if err != nil && !errors.Is(err, os.ErrNotExist) {
		return err
	}
	if err == nil && !bytes.Contains(old, []byte(knowledge.GeneratedIndex)) {
		return errors.New("gnosis/index.md is hand-authored; move it aside before regenerating the root index")
	}
	return knowledge.WriteAtomic(file, []byte(text.String()))
}

func dependencyOrder(packages map[string]knowledge.Manifest, names []string) ([]string, error) {
	state := map[string]int{}
	order := []string{}
	var visit func(string) error
	visit = func(name string) error {
		if state[name] == 1 {
			return fmt.Errorf("dependency cycle at %s", name)
		}
		if state[name] == 2 {
			return nil
		}
		m, exists := packages[name]
		if !exists {
			return fmt.Errorf("missing dependency %s; discover and add it first", name)
		}
		state[name] = 1
		for _, dep := range m.Dependencies {
			if err := visit(dep); err != nil {
				return fmt.Errorf("%s: %w", name, err)
			}
		}
		state[name] = 2
		order = append(order, name)
		return nil
	}
	for _, name := range names {
		if err := visit(name); err != nil {
			return nil, err
		}
	}
	return order, nil
}

func Validate(w Workspace, names []string) error {
	packages, err := Packages(w)
	if err != nil {
		return err
	}
	lock, err := knowledge.ReadLock(w.Dir)
	if err != nil {
		return err
	}
	for name := range lock.Packages {
		if _, exists := packages[name]; !exists {
			return fmt.Errorf("locked package %s is missing; run gnosis restore", name)
		}
	}
	if err := validateLockGraph(packages, lock); err != nil {
		return err
	}
	if len(names) == 0 {
		names = knowledge.SortedKeys(packages)
	}
	order, err := dependencyOrder(packages, names)
	if err != nil {
		return err
	}
	for _, name := range order {
		if _, err := knowledge.ValidatePackage(filepath.Join(w.Dir, name)); err != nil {
			return fmt.Errorf("%s: %w", name, err)
		}
	}
	return nil
}

func validateLockGraph(packages map[string]knowledge.Manifest, lock knowledge.Lockfile) error {
	locked := map[string]knowledge.Manifest{}
	for name, p := range lock.Packages {
		locked[name] = knowledge.Manifest{Dependencies: p.Dependencies}
	}
	for name, m := range packages {
		if _, exists := locked[name]; !exists {
			locked[name] = m
		}
	}
	if _, err := dependencyOrder(locked, knowledge.SortedKeys(locked)); err != nil {
		return fmt.Errorf("lock dependency graph: %w", err)
	}
	return nil
}

func commitMetadata(ctx context.Context, w Workspace, name string) error {
	if err := GenerateIndex(w); err != nil {
		return err
	}
	if _, err := git(ctx, w.Root, "add", "--", "gnosis/gnosis.lock", "gnosis/index.md"); err != nil {
		return err
	}
	diff, err := gitPaths(ctx, w.Root, "diff", "--cached", "--name-only")
	if err != nil {
		return err
	}
	if len(diff) == 0 {
		return nil
	}
	for _, file := range diff {
		if file != "gnosis/gnosis.lock" && file != "gnosis/index.md" {
			return fmt.Errorf("refusing to include unrelated staged file %s in lock commit", file)
		}
	}
	_, err = git(ctx, w.Root, "commit", "-m", "gnosis: lock "+name)
	return err
}
