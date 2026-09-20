package workspace

import (
	"context"
	"errors"
	"fmt"
	"io"
	"maps"
	"os"
	"os/exec"
	"path/filepath"
	"slices"
	"strings"

	"gnosis/internal/knowledge"
)

type preparedPackage struct {
	name string
	repo string
	lock knowledge.LockedPackage
}

type resolver struct {
	w        Workspace
	registry knowledge.Registry
	lock     knowledge.Lockfile
	local    map[string]knowledge.Manifest
	tmp      string
	clones   map[knowledge.Source]string
	states   map[string]int
	update   map[string]bool
	restore  bool
	prepared []preparedPackage
}

func (r *resolver) visit(ctx context.Context, name string) error {
	if err := knowledge.ValidateName(name); err != nil {
		return err
	}
	if r.states[name] == 1 {
		return fmt.Errorf("dependency cycle at %s", name)
	}
	if r.states[name] == 2 {
		return nil
	}
	r.states[name] = 1
	m, exists := r.local[name]
	locked, imported := r.lock.Packages[name]
	if exists && (!imported || !r.update[name]) {
		if _, err := knowledge.ValidatePackage(filepath.Join(r.w.Dir, name)); err != nil {
			return err
		}
		for _, dep := range m.Dependencies {
			if err := r.visit(ctx, dep); err != nil {
				return err
			}
		}
		r.states[name] = 2
		return nil
	}
	entry, discovered := r.registry[name]
	if !discovered && !imported {
		return fmt.Errorf("package %s is not in the registry or lock", name)
	}
	if r.restore && !imported {
		return fmt.Errorf("dependency %s is not locked; restore never resolves new packages", name)
	}
	s := entry.Source
	pinned := ""
	if imported {
		s = locked.Source
		if r.update[name] && discovered {
			s = entry.Source
		} else if !r.update[name] {
			pinned = locked.Commit
		}
	}
	key := s
	key.Path = "."
	repo, ok := r.clones[key]
	if !ok {
		var err error
		repo, err = cloneSource(ctx, s, r.tmp, pinned)
		if err != nil {
			return err
		}
		r.clones[key] = repo
	}
	commit := pinned
	if commit == "" {
		var err error
		commit, err = git(ctx, repo, "rev-parse", "refs/remotes/origin/"+s.DefaultBranch)
		if err != nil {
			return err
		}
	} else if _, err := git(ctx, repo, "cat-file", "-e", commit+"^{commit}"); err != nil {
		if _, err := git(ctx, repo, "fetch", "--quiet", "origin", commit); err != nil {
			return fmt.Errorf("locked source commit %s is unavailable: %w", commit, err)
		}
	}
	dir, err := os.MkdirTemp(r.tmp, "package-")
	if err != nil {
		return err
	}
	if err := exportTree(ctx, repo, subtreeTree(commit, s.Path), dir); err != nil {
		return err
	}
	m, err = knowledge.ValidatePackage(dir)
	if err != nil {
		return fmt.Errorf("%s: %w", name, err)
	}
	if m.Package != name {
		return fmt.Errorf("registry package %s resolves to manifest %s", name, m.Package)
	}
	for _, dep := range m.Dependencies {
		if err := r.visit(ctx, dep); err != nil {
			return fmt.Errorf("%s: %w", name, err)
		}
	}
	split := commit
	if s.Path != "." {
		if _, err := git(ctx, repo, "sparse-checkout", "set", "--cone", "--", s.Path); err != nil {
			return err
		}
		if _, err := git(ctx, repo, "checkout", "--quiet", "--detach", commit); err != nil {
			return err
		}
		// Native subtree parses ls-tree's display output rather than NUL-delimited paths.
		split, err = git(
			ctx, repo, "-c", "core.quotePath=false", "subtree", "split", "--quiet", "--prefix="+s.Path, commit,
		)
		if err != nil {
			return err
		}
	}
	if !knowledge.IsCommit(split) {
		return fmt.Errorf("git subtree returned an invalid commit %q", split)
	}
	next := knowledge.LockedPackage{
		Source: s, Version: m.Version, Commit: commit, SubtreeCommit: split,
		Dependencies: append([]string{}, m.Dependencies...),
	}
	if pinned != "" {
		if next.SubtreeCommit != locked.SubtreeCommit || next.Version != locked.Version ||
			!slices.Equal(next.Dependencies, locked.Dependencies) {
			return fmt.Errorf("%s: source contents do not match gnosis.lock", name)
		}
	}
	r.prepared = append(r.prepared, preparedPackage{name: name, repo: repo, lock: next})
	r.states[name] = 2
	return nil
}

type SyncOptions struct {
	Names   []string
	Update  bool
	Restore bool
}

func Sync(ctx context.Context, w Workspace, options SyncOptions, out io.Writer) (err error) {
	if err := w.NoPending(); err != nil {
		return err
	}
	if err := w.clean(ctx); err != nil {
		return err
	}
	lock, err := knowledge.ReadLock(w.Dir)
	if err != nil {
		return err
	}
	local, err := Packages(w)
	if err != nil {
		return err
	}
	discovered, err := knowledge.ReadRegistry(w.Dir)
	if err != nil {
		return err
	}
	if len(options.Names) == 0 {
		options.Names = knowledge.SortedKeys(lock.Packages)
	}
	tmp, err := os.MkdirTemp("", "gnosis-resolve-")
	if err != nil {
		return err
	}
	defer func() { err = errors.Join(err, os.RemoveAll(tmp)) }()
	r := resolver{
		w: w, registry: discovered, lock: lock, local: local, tmp: tmp,
		clones: map[knowledge.Source]string{}, states: map[string]int{}, update: map[string]bool{},
		restore: options.Restore, prepared: []preparedPackage{},
	}
	for _, name := range options.Names {
		if options.Update {
			if _, exists := lock.Packages[name]; !exists {
				return fmt.Errorf("%s is not imported; use add or the source repository's Git workflow", name)
			}
			r.update[name] = true
		}
	}
	for _, name := range options.Names {
		if err := r.visit(ctx, name); err != nil {
			return err
		}
	}
	projected := knowledge.Lockfile{GnosisVersion: lock.GnosisVersion, Packages: maps.Clone(lock.Packages)}
	for _, p := range r.prepared {
		projected.Packages[p.name] = p.lock
	}
	if err := validateLockGraph(local, projected); err != nil {
		return fmt.Errorf("%w; pull the affected dependencies together to update their pins", err)
	}
	for _, p := range r.prepared {
		if err := applyPackage(ctx, w, p); err != nil {
			return err
		}
		if _, err := fmt.Fprintf(out, "%s %s (%s)\n", p.name, p.lock.Version, p.lock.Commit); err != nil {
			return err
		}
	}
	return nil
}

type pendingUpdate struct {
	Name       string                  `yaml:"name"`
	BeforeHead string                  `yaml:"before_head"`
	Package    knowledge.LockedPackage `yaml:"package"`
}

func applyPackage(ctx context.Context, w Workspace, p preparedPackage) error {
	dir, err := knowledge.SafePath(w.Dir, p.name)
	if err != nil {
		return err
	}
	_, statErr := os.Lstat(dir)
	exists := statErr == nil
	if statErr != nil && !errors.Is(statErr, os.ErrNotExist) {
		return statErr
	}
	if !exists {
		ignored, err := git(ctx, w.Root, "check-ignore", "--no-index", "gnosis/"+p.name)
		if err == nil && ignored != "" {
			return fmt.Errorf("gnosis/%s is ignored by Git; correct .gitignore before importing", p.name)
		}
		if err != nil {
			var exit *exec.ExitError
			if !errors.As(err, &exit) || exit.ExitCode() != 1 {
				return err
			}
		}
	}
	if _, err := git(ctx, w.Root, "fetch", "--quiet", "--no-tags", p.repo, p.lock.SubtreeCommit); err != nil {
		return err
	}
	before, err := git(ctx, w.Root, "rev-parse", "HEAD")
	if err != nil {
		return err
	}
	pending := pendingUpdate{Name: p.name, BeforeHead: before, Package: p.lock}
	if err := knowledge.WriteData(w.PendingPath(), pending); err != nil {
		return err
	}
	verb := "add"
	if exists {
		verb = "merge"
	}
	_, err = git(ctx, w.Root,
		"subtree", verb, "--prefix=gnosis/"+p.name,
		"-m", "gnosis: "+verb+" "+p.name, p.lock.SubtreeCommit,
	)
	if err != nil {
		return fmt.Errorf("%w\noperation retained: resolve conflicts, then gnosis pull --continue; "+
			"or gnosis pull --abort", err)
	}
	return finishUpdate(ctx, w, pending)
}

func finishUpdate(ctx context.Context, w Workspace, pending pendingUpdate) error {
	if err := knowledge.ValidateName(pending.Name); err != nil {
		return err
	}
	if !knowledge.IsCommit(pending.Package.SubtreeCommit) || !knowledge.IsCommit(pending.BeforeHead) {
		return errors.New("invalid pending update commit IDs")
	}
	if _, err := git(ctx, w.Root, "merge-base", "--is-ancestor", pending.BeforeHead, "HEAD"); err != nil {
		return errors.New("HEAD no longer descends from the update's starting commit; recover the Git history first")
	}
	if _, err := git(ctx, w.Root, "merge-base", "--is-ancestor", pending.Package.SubtreeCommit, "HEAD"); err != nil {
		return errors.New("the package merge has not completed; resolve and commit it, or use pull --abort")
	}
	if err := pendingWorktree(ctx, w, false); err != nil {
		return err
	}
	if err := validateMergedPackage(w, pending.Name, pending.Package); err != nil {
		return fmt.Errorf("merged package failed validation; fix and commit it before pull --continue: %w", err)
	}
	lock, err := knowledge.ReadLock(w.Dir)
	if err != nil {
		return err
	}
	lock.Packages[pending.Name] = pending.Package
	if err := knowledge.WriteData(filepath.Join(w.Dir, "gnosis.lock"), lock); err != nil {
		return err
	}
	if err := commitMetadata(ctx, w, pending.Name); err != nil {
		return err
	}
	return os.Remove(w.PendingPath())
}

func validateMergedPackage(w Workspace, name string, next knowledge.LockedPackage) error {
	dir, err := knowledge.SafePath(w.Dir, name)
	if err != nil {
		return err
	}
	m, err := knowledge.ValidatePackage(dir)
	if err != nil {
		return err
	}
	if m.Package != name {
		return errors.New("merged manifest package name changed")
	}
	packages, err := Packages(w)
	if err != nil {
		return err
	}
	lock, err := knowledge.ReadLock(w.Dir)
	if err != nil {
		return err
	}
	lock.Packages[name] = next
	if err := validateLockGraph(packages, lock); err != nil {
		return err
	}
	// Missing locked packages may still be waiting in a multi-package restore.
	for name, p := range lock.Packages {
		if _, exists := packages[name]; !exists {
			packages[name] = knowledge.Manifest{Dependencies: p.Dependencies}
		}
	}
	_, err = dependencyOrder(packages, knowledge.SortedKeys(packages))
	return err
}

func Recover(ctx context.Context, w Workspace, abort bool) error {
	var pending pendingUpdate
	if err := knowledge.ReadData(w.PendingPath(), &pending); err != nil {
		return fmt.Errorf("no readable pending update: %w", err)
	}
	if err := knowledge.ValidateName(pending.Name); err != nil {
		return err
	}
	_, statErr := os.Lstat(filepath.Join(w.gitDir, "MERGE_HEAD"))
	merging := statErr == nil
	if statErr != nil && !errors.Is(statErr, os.ErrNotExist) {
		return statErr
	}
	if abort {
		if merging {
			if _, err := git(ctx, w.Root, "merge", "--abort"); err != nil {
				return err
			}
		}
		head, err := git(ctx, w.Root, "rev-parse", "HEAD")
		if err != nil {
			return err
		}
		if head != pending.BeforeHead {
			return errors.New("merge already committed; use pull --continue (gnosis will not reset your commits)")
		}
		status, err := git(ctx, w.Root, "status", "--porcelain")
		if err != nil {
			return err
		}
		if status != "" {
			return errors.New("working tree still has changes; inspect git status before retrying pull --abort")
		}
		return os.Remove(w.PendingPath())
	}
	if merging {
		unmerged, err := gitPaths(ctx, w.Root, "diff", "--name-only", "--diff-filter=U")
		if err != nil {
			return err
		}
		if len(unmerged) != 0 {
			return errors.New("resolve and stage all Git conflicts before pull --continue")
		}
		if err := pendingWorktree(ctx, w, true); err != nil {
			return err
		}
		staged, err := gitPaths(ctx, w.Root, "diff", "--cached", "--name-only")
		if err != nil {
			return err
		}
		for _, file := range staged {
			if !strings.HasPrefix(file, "gnosis/"+pending.Name+"/") {
				return fmt.Errorf("unrelated staged change %s; remove it from the index before continuing", file)
			}
		}
		if err := validateMergedPackage(w, pending.Name, pending.Package); err != nil {
			return err
		}
		if _, err := git(ctx, w.Root, "commit", "--no-edit"); err != nil {
			return err
		}
	}
	return finishUpdate(ctx, w, pending)
}

func pendingWorktree(ctx context.Context, w Workspace, merging bool) error {
	untracked, err := gitPaths(ctx, w.Root, "ls-files", "--others", "--exclude-standard")
	if err != nil {
		return err
	}
	if len(untracked) != 0 {
		return errors.New("commit or remove untracked changes before pull --continue")
	}
	unstaged, err := gitPaths(ctx, w.Root, "diff", "--name-only")
	if err != nil {
		return err
	}
	for _, file := range unstaged {
		metadata := file == "gnosis/gnosis.lock" || file == "gnosis/index.md"
		if merging || !metadata {
			return fmt.Errorf("stage/commit the unstaged change in %s before pull --continue", file)
		}
	}
	return nil
}
