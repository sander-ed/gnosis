package main

import (
	"context"
	"errors"
	"fmt"
	"io"
	"os"
	"os/exec"
	"path/filepath"
	"slices"
	"strings"
)

type preparedPackage struct {
	name string
	repo string
	lock lockedPackage
}

type resolver struct {
	w        workspace
	registry registry
	lock     lockfile
	local    map[string]manifest
	tmp      string
	clones   map[source]string
	states   map[string]int
	update   map[string]bool
	restore  bool
	prepared []preparedPackage
}

func (r *resolver) visit(ctx context.Context, name string) error {
	if err := validName(name); err != nil {
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
		if _, _, _, err := scanPackage(filepath.Join(r.w.dir, name), true); err != nil {
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
		if !r.update[name] {
			pinned = locked.Commit
		}
	}
	key := s
	key.Path = "."
	repo, ok := r.clones[key]
	if !ok {
		var err error
		repo, err = cloneSource(ctx, s, r.tmp)
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
	m, _, _, err = scanPackage(dir, true)
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
		split, err = git(ctx, repo, "subtree", "split", "--quiet", "--prefix="+s.Path, commit)
		if err != nil {
			return err
		}
	}
	if !shaPattern.MatchString(split) {
		return fmt.Errorf("git subtree returned an invalid commit %q", split)
	}
	next := lockedPackage{
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

type syncOptions struct {
	names   []string
	update  bool
	restore bool
}

func syncPackages(ctx context.Context, w workspace, options syncOptions, out io.Writer) (err error) {
	if err := w.noPending(); err != nil {
		return err
	}
	if err := w.clean(ctx); err != nil {
		return err
	}
	lock, err := readLock(w.dir)
	if err != nil {
		return err
	}
	local, err := localPackages(w)
	if err != nil {
		return err
	}
	discovered, err := readRegistry(w.dir)
	if err != nil {
		return err
	}
	if len(options.names) == 0 {
		options.names = sortedKeys(lock.Packages)
	}
	tmp, err := os.MkdirTemp("", "gnosis-resolve-")
	if err != nil {
		return err
	}
	defer func() { err = errors.Join(err, os.RemoveAll(tmp)) }()
	r := resolver{
		w: w, registry: discovered, lock: lock, local: local, tmp: tmp,
		clones: map[source]string{}, states: map[string]int{}, update: map[string]bool{},
		restore: options.restore, prepared: []preparedPackage{},
	}
	for _, name := range options.names {
		if options.update {
			if _, exists := lock.Packages[name]; !exists {
				return fmt.Errorf("%s is not imported; use add or the source repository's Git workflow", name)
			}
			r.update[name] = true
		}
	}
	for _, name := range options.names {
		if err := r.visit(ctx, name); err != nil {
			return err
		}
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
	Name       string        `yaml:"name"`
	BeforeHead string        `yaml:"before_head"`
	Package    lockedPackage `yaml:"package"`
}

func applyPackage(ctx context.Context, w workspace, p preparedPackage) error {
	dir, err := safePath(w.dir, p.name)
	if err != nil {
		return err
	}
	_, statErr := os.Lstat(dir)
	exists := statErr == nil
	if statErr != nil && !errors.Is(statErr, os.ErrNotExist) {
		return statErr
	}
	if !exists {
		ignored, err := git(ctx, w.root, "check-ignore", "--no-index", "gnosis/"+p.name)
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
	if _, err := git(ctx, w.root, "fetch", "--quiet", "--no-tags", p.repo, p.lock.SubtreeCommit); err != nil {
		return err
	}
	before, err := git(ctx, w.root, "rev-parse", "HEAD")
	if err != nil {
		return err
	}
	pending := pendingUpdate{Name: p.name, BeforeHead: before, Package: p.lock}
	if err := writeData(w.pendingPath(), pending); err != nil {
		return err
	}
	verb := "add"
	if exists {
		verb = "merge"
	}
	_, err = git(ctx, w.root,
		"subtree", verb, "--prefix=gnosis/"+p.name,
		"-m", "gnosis: "+verb+" "+p.name, p.lock.SubtreeCommit,
	)
	if err != nil {
		return fmt.Errorf("%w\noperation retained: resolve conflicts, then gnosis pull --continue; "+
			"or gnosis pull --abort", err)
	}
	return finishUpdate(ctx, w, pending)
}

func finishUpdate(ctx context.Context, w workspace, pending pendingUpdate) error {
	if err := validName(pending.Name); err != nil {
		return err
	}
	if !shaPattern.MatchString(pending.Package.SubtreeCommit) || !shaPattern.MatchString(pending.BeforeHead) {
		return errors.New("invalid pending update commit IDs")
	}
	if _, err := git(ctx, w.root, "merge-base", "--is-ancestor", pending.BeforeHead, "HEAD"); err != nil {
		return errors.New("HEAD no longer descends from the update's starting commit; recover the Git history first")
	}
	if _, err := git(ctx, w.root, "merge-base", "--is-ancestor", pending.Package.SubtreeCommit, "HEAD"); err != nil {
		return errors.New("the package merge has not completed; resolve and commit it, or use pull --abort")
	}
	if err := pendingWorktree(ctx, w, false); err != nil {
		return err
	}
	dir, err := safePath(w.dir, pending.Name)
	if err != nil {
		return err
	}
	m, _, _, err := scanPackage(dir, true)
	if err != nil {
		return fmt.Errorf("merged package failed validation; fix and commit it before pull --continue: %w", err)
	}
	if m.Package != pending.Name {
		return errors.New("merged manifest package name changed")
	}
	lock, err := readLock(w.dir)
	if err != nil {
		return err
	}
	lock.Packages[pending.Name] = pending.Package
	if err := writeData(filepath.Join(w.dir, "gnosis.lock"), lock); err != nil {
		return err
	}
	if err := commitMetadata(ctx, w, pending.Name); err != nil {
		return err
	}
	return os.Remove(w.pendingPath())
}

func recoverUpdate(ctx context.Context, w workspace, abort bool) error {
	var pending pendingUpdate
	if err := readData(w.pendingPath(), &pending); err != nil {
		return fmt.Errorf("no readable pending update: %w", err)
	}
	if err := validName(pending.Name); err != nil {
		return err
	}
	_, statErr := os.Lstat(filepath.Join(w.gitDir, "MERGE_HEAD"))
	merging := statErr == nil
	if statErr != nil && !errors.Is(statErr, os.ErrNotExist) {
		return statErr
	}
	if abort {
		if merging {
			if _, err := git(ctx, w.root, "merge", "--abort"); err != nil {
				return err
			}
		}
		head, err := git(ctx, w.root, "rev-parse", "HEAD")
		if err != nil {
			return err
		}
		if head != pending.BeforeHead {
			return errors.New("merge already committed; use pull --continue (gnosis will not reset your commits)")
		}
		status, err := git(ctx, w.root, "status", "--porcelain")
		if err != nil {
			return err
		}
		if status != "" {
			return errors.New("working tree still has changes; inspect git status before retrying pull --abort")
		}
		return os.Remove(w.pendingPath())
	}
	if merging {
		unmerged, err := git(ctx, w.root, "diff", "--name-only", "--diff-filter=U")
		if err != nil {
			return err
		}
		if unmerged != "" {
			return errors.New("resolve and stage all Git conflicts before pull --continue")
		}
		if err := pendingWorktree(ctx, w, true); err != nil {
			return err
		}
		staged, err := git(ctx, w.root, "diff", "--cached", "--name-only")
		if err != nil {
			return err
		}
		for _, file := range strings.Split(staged, "\n") {
			if file != "" && !strings.HasPrefix(file, "gnosis/"+pending.Name+"/") {
				return fmt.Errorf("unrelated staged change %s; remove it from the index before continuing", file)
			}
		}
		if _, _, _, err := scanPackage(filepath.Join(w.dir, pending.Name), true); err != nil {
			return err
		}
		if _, err := git(ctx, w.root, "commit", "--no-edit"); err != nil {
			return err
		}
	}
	return finishUpdate(ctx, w, pending)
}

func pendingWorktree(ctx context.Context, w workspace, merging bool) error {
	untracked, err := git(ctx, w.root, "ls-files", "--others", "--exclude-standard")
	if err != nil {
		return err
	}
	if untracked != "" {
		return errors.New("commit or remove untracked changes before pull --continue")
	}
	unstaged, err := git(ctx, w.root, "diff", "--name-only")
	if err != nil {
		return err
	}
	for _, file := range strings.Split(unstaged, "\n") {
		if file == "" {
			continue
		}
		metadata := file == "gnosis/gnosis.lock" || file == "gnosis/index.md"
		if merging || !metadata {
			return fmt.Errorf("stage/commit the unstaged change in %s before pull --continue", file)
		}
	}
	return nil
}
