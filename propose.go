package main

import (
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

type proposalOptions struct {
	name    string
	title   string
	body    string
	branch  string
	publish bool
}

func propose(ctx context.Context, w workspace, options proposalOptions, out io.Writer) error {
	ctx, cancel := context.WithTimeout(ctx, 10*time.Minute)
	defer cancel()
	if err := validName(options.name); err != nil {
		return err
	}
	if err := w.noPending(); err != nil {
		return err
	}
	if err := w.clean(ctx); err != nil {
		return err
	}
	if err := validateWorkspace(w, []string{options.name}); err != nil {
		return err
	}
	lock, err := readLock(w.dir)
	if err != nil {
		return err
	}
	p, exists := lock.Packages[options.name]
	if !exists {
		return errors.New("package is locally authored; propose changes with the source repository's normal Git/PR workflow")
	}
	if strings.TrimSpace(options.title) == "" {
		options.title = "Update " + options.name
	}
	if options.branch == "" {
		options.branch = fmt.Sprintf("gnosis/%s-%d", options.name, time.Now().UnixNano())
	}
	if !strings.HasPrefix(options.branch, "gnosis/") || options.branch == p.Source.DefaultBranch {
		return errors.New("proposal branch must start with gnosis/ and must not be the default branch")
	}
	if _, err := git(ctx, w.root, "check-ref-format", "refs/heads/"+options.branch); err != nil {
		return err
	}
	if options.publish {
		if _, err := exec.LookPath("gh"); err != nil {
			return errors.New("publishing requires GitHub CLI (gh); install it and run gh auth login")
		}
	}
	cmd := gitCommand(ctx, w.root,
		"diff", "--binary", "--full-index", "--no-ext-diff", "--no-textconv",
		p.SubtreeCommit, "HEAD:gnosis/"+options.name, "--",
	)
	var stderr bytes.Buffer
	cmd.Stderr = &stderr
	patch, err := cmd.Output()
	if err != nil {
		if ctx.Err() != nil {
			return ctx.Err()
		}
		return fmt.Errorf("compute package changes: %w: %s", err, stderr.String())
	}
	if len(patch) == 0 {
		return errors.New("no package changes to propose")
	}
	parent := filepath.Join(w.gitDir, "gnosis", "proposals")
	if err := os.MkdirAll(parent, 0o700); err != nil {
		return err
	}
	repo, err := cloneSource(ctx, p.Source, parent)
	if err != nil {
		return fmt.Errorf("prepare proposal (checkout retained at %s): %w", repo, err)
	}
	// Proposals remain on disk on success and failure for native Git recovery.
	if _, err := fmt.Fprintf(out, "Proposal checkout: %s\n", repo); err != nil {
		return err
	}
	for _, field := range []string{"user.name", "user.email"} {
		value, err := git(ctx, w.root, "config", "--get", field)
		if err != nil {
			return err
		}
		if _, err := git(ctx, repo, "config", field, value); err != nil {
			return err
		}
	}
	if p.Source.Path != "." {
		if _, err := git(ctx, repo, "sparse-checkout", "set", "--cone", "--", p.Source.Path); err != nil {
			return err
		}
	}
	_, err = git(
		ctx, repo, "checkout", "--quiet", "-b", options.branch, "origin/"+p.Source.DefaultBranch,
	)
	if err != nil {
		return err
	}
	args := []string{"apply", "--3way", "--index", "--whitespace=nowarn"}
	if p.Source.Path != "." {
		args = append(args, "--directory="+p.Source.Path)
	}
	apply := gitCommand(ctx, repo, args...)
	apply.Stdin = bytes.NewReader(patch)
	result, err := apply.CombinedOutput()
	if err != nil {
		if ctx.Err() != nil {
			err = ctx.Err()
		}
		return fmt.Errorf("proposal patch could not be applied: %w\n%s\n"+
			"resolve with native Git in %s; the consumer and default branch are unchanged",
			err, result, repo,
		)
	}
	if err := validateProposal(ctx, repo, p.Source.Path); err != nil {
		return fmt.Errorf("proposal failed validation in %s: %w", repo, err)
	}
	changes, err := git(ctx, repo, "diff", "--cached", "--name-only")
	if err != nil {
		return err
	}
	if changes == "" {
		return fmt.Errorf("source already contains these changes; checkout retained at %s", repo)
	}
	if p.Source.Path != "." {
		for _, file := range strings.Split(changes, "\n") {
			if !strings.HasPrefix(file, p.Source.Path+"/") {
				return fmt.Errorf("proposal unexpectedly changes %s outside its package", file)
			}
		}
	}
	if _, err := git(ctx, repo, "commit", "-m", options.title); err != nil {
		return err
	}
	if !options.publish {
		_, err := fmt.Fprintf(out, "Prepared branch %s; nothing pushed. Review with git -C %q show.\n", options.branch, repo)
		return err
	}
	if _, err := git(ctx, repo, "push", "--set-upstream", "origin",
		"HEAD:refs/heads/"+options.branch,
	); err != nil {
		return fmt.Errorf("proposal retained at %s; pushing requires source write access "+
			"(or set up a fork there manually): %w", repo, err)
	}
	ghCtx, cancel := context.WithTimeout(ctx, 2*time.Minute)
	defer cancel()
	pr := exec.CommandContext(ghCtx, "gh",
		"pr", "create", "--base", p.Source.DefaultBranch, "--head", options.branch,
		"--title", options.title, "--body", options.body,
	)
	pr.Dir = repo
	pr.Stdout = out
	var ghError bytes.Buffer
	pr.Stderr = &ghError
	if err := pr.Run(); err != nil {
		if ghCtx.Err() != nil {
			err = ghCtx.Err()
		}
		return fmt.Errorf("branch %s was pushed but PR creation failed; retry gh pr create in %s: %w\n%s",
			options.branch, repo, err, ghError.String(),
		)
	}
	return nil
}

func validateProposal(ctx context.Context, repo, prefix string) (err error) {
	tree, err := git(ctx, repo, "write-tree")
	if err != nil {
		return err
	}
	dir, err := os.MkdirTemp("", "gnosis-proposal-validation-")
	if err != nil {
		return err
	}
	defer func() { err = errors.Join(err, os.RemoveAll(dir)) }()
	if err := exportTree(ctx, repo, subtreeTree(tree, prefix), dir); err != nil {
		return err
	}
	_, _, _, err = scanPackage(dir, true)
	return err
}
