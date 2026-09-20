package main

import (
	"context"
	"errors"
	"fmt"
	"io"
	"os/exec"
	"path/filepath"
	"strings"

	"github.com/spf13/cobra"
)

var version = "dev"

type streams struct {
	in  io.Reader
	out io.Writer
	err io.Writer
}

type usageError struct{ err error }

func (e *usageError) Error() string { return e.err.Error() }
func (e *usageError) Unwrap() error { return e.err }

type application struct {
	cwd     string
	io      streams
	started bool
}

func (a *application) withWorkspace(ctx context.Context, mutate bool, fn func(workspace) error) (err error) {
	w, err := openWorkspace(ctx, a.cwd)
	if err != nil {
		return err
	}
	if err := w.initialized(); err != nil {
		return err
	}
	if mutate {
		release, err := w.acquire()
		if err != nil {
			return err
		}
		defer func() { err = errors.Join(err, release()) }()
	}
	return fn(w)
}

func arguments(validator cobra.PositionalArgs) cobra.PositionalArgs {
	return func(cmd *cobra.Command, args []string) error {
		if err := validator(cmd, args); err != nil {
			return &usageError{err: err}
		}
		return nil
	}
}

func packageNames(_ *cobra.Command, args []string) error {
	for _, name := range args {
		if err := validName(name); err != nil {
			return err
		}
	}
	return nil
}

func (a *application) command() *cobra.Command {
	var input string
	root := &cobra.Command{
		Use: "gnosis", Short: "Manage Git-backed Open Knowledge Format packages",
		Version: version, SilenceUsage: true, SilenceErrors: true,
		TraverseChildren: true,
		Args:             arguments(cobra.NoArgs),
		PersistentPreRun: func(_ *cobra.Command, _ []string) { a.started = true },
		RunE: func(cmd *cobra.Command, _ []string) error {
			if cmd.Flags().Changed("input") {
				_, err := fmt.Fprintln(a.io.out, input)
				return err
			}
			return cmd.Help()
		},
	}
	root.SetIn(a.io.in)
	root.SetOut(a.io.out)
	root.SetErr(a.io.err)
	root.SetFlagErrorFunc(func(_ *cobra.Command, err error) error { return &usageError{err: err} })
	root.PersistentFlags().StringVarP(&a.cwd, "directory", "C", a.cwd, "Run in this repository directory")
	root.Flags().StringVarP(&input, "input", "i", "", "Echo text (legacy proof-of-concept compatibility)")
	root.AddCommand(a.initCommand(), a.packageCommand(), a.registryCommand())
	root.AddCommand(a.listCommand(), a.addCommand(), a.pullCommand(), a.restoreCommand())
	root.AddCommand(a.indexCommand(), a.validateCommand(), a.codeownersCommand(), a.proposeCommand())
	for _, name := range []string{"status", "merge", "rebase"} {
		root.AddCommand(a.gitCommand(name))
	}
	skills := &cobra.Command{
		Use: "skills", Short: "Install bundled agent meta skills without overwriting customizations",
		Args: arguments(cobra.NoArgs),
		RunE: func(cmd *cobra.Command, _ []string) error {
			return a.withWorkspace(cmd.Context(), true, func(w workspace) error {
				if err := installSkills(w.root, true); err != nil {
					return err
				}
				return installSkills(w.root, false)
			})
		},
	}
	root.AddCommand(skills)
	return root
}

func (a *application) initCommand() *cobra.Command {
	install := true
	cmd := &cobra.Command{
		Use: "init", Short: "Initialize gnosis/ in an existing Git repository",
		Args: arguments(cobra.NoArgs),
		RunE: func(cmd *cobra.Command, _ []string) (err error) {
			w, err := openWorkspace(cmd.Context(), a.cwd)
			if err != nil {
				return err
			}
			release, err := w.acquire()
			if err != nil {
				return err
			}
			defer func() { err = errors.Join(err, release()) }()
			if err := initWorkspace(w, install); err != nil {
				return err
			}
			_, err = fmt.Fprintf(a.io.out, "Initialized %s\n", w.dir)
			return err
		},
	}
	cmd.Flags().BoolVar(&install, "skills", true, "Install gnosis meta skills into .agents/skills")
	return cmd
}

func (a *application) packageCommand() *cobra.Command {
	parent := &cobra.Command{Use: "package", Short: "Manage locally authored knowledge packages"}
	var handle, description string
	dependencies := []string{}
	cmd := &cobra.Command{
		Use: "init NAME", Short: "Create a package with an editable standard and one owner",
		Args: arguments(cobra.ExactArgs(1)),
		RunE: func(cmd *cobra.Command, args []string) error {
			if handle == "" {
				return &usageError{err: errors.New("--owner @user or @organization/team is required")}
			}
			o := owner{User: handle}
			if strings.Contains(handle, "/") {
				o = owner{Team: handle}
			}
			m := manifest{
				GnosisVersion: 1, Package: args[0], Version: "0.1.0",
				Description: description, Owner: o, Dependencies: dependencies,
			}
			if err := m.validate(); err != nil {
				return &usageError{err: err}
			}
			return a.withWorkspace(cmd.Context(), true, func(w workspace) error {
				if err := w.noPending(); err != nil {
					return err
				}
				dir, err := safePath(w.dir, m.Package)
				if err != nil {
					return err
				}
				if err := scaffoldPackage(dir, m); err != nil {
					return err
				}
				return rootIndex(w)
			})
		},
	}
	cmd.Flags().StringVar(&handle, "owner", "", "One GitHub @user or @organization/team (required)")
	cmd.Flags().StringVar(&description, "description", "", "Package summary used in the index")
	cmd.Flags().StringSliceVar(&dependencies, "dependency", []string{}, "Dependency names (repeat or comma-separate)")
	parent.AddCommand(cmd)
	return parent
}

func (a *application) registryCommand() *cobra.Command {
	parent := &cobra.Command{Use: "registry", Short: "Discover packages and maintain the local registry"}
	ref, prefix := "main", "gnosis"
	add := &cobra.Command{
		Use: "add REPOSITORY", Short: "Discover packages/registry in a public, private or local Git repository",
		Args: arguments(cobra.ExactArgs(1)),
		RunE: func(cmd *cobra.Command, args []string) error {
			repository := args[0]
			if !strings.Contains(repository, ":") {
				var err error
				repository, err = filepath.Abs(filepath.Join(a.cwd, repository))
				if filepath.IsAbs(args[0]) {
					repository = args[0]
				}
				if err != nil {
					return err
				}
			}
			s := source{Type: "git", Repository: repository, Path: prefix, DefaultBranch: ref}
			if err := s.validate(); err != nil {
				return &usageError{err: err}
			}
			return a.withWorkspace(cmd.Context(), true, func(w workspace) error {
				if err := w.noPending(); err != nil {
					return err
				}
				r, err := discoverRegistry(cmd.Context(), s)
				if err != nil {
					return err
				}
				return mergeRegistry(w, r)
			})
		},
	}
	add.Flags().StringVar(&ref, "ref", "main", "Source default branch")
	add.Flags().StringVar(&prefix, "path", "gnosis", "Repository-relative bundle root or package parent")
	importCmd := &cobra.Command{
		Use: "import FILE", Short: "Import a YAML or JSON registry mapping",
		Args: arguments(cobra.ExactArgs(1)),
		RunE: func(cmd *cobra.Command, args []string) error {
			file := args[0]
			if !filepath.IsAbs(file) {
				file = filepath.Join(a.cwd, file)
			}
			r := registry{}
			if err := readData(file, &r); err != nil {
				return err
			}
			if r == nil {
				return errors.New("registry must be a mapping")
			}
			return a.withWorkspace(cmd.Context(), true, func(w workspace) error {
				if err := w.noPending(); err != nil {
					return err
				}
				return mergeRegistry(w, r)
			})
		},
	}
	parent.AddCommand(add, importCmd)
	return parent
}

func (a *application) listCommand() *cobra.Command {
	var asJSON bool
	cmd := &cobra.Command{
		Use: "list", Short: "List installed packages",
		Args: arguments(cobra.NoArgs),
		RunE: func(cmd *cobra.Command, _ []string) error {
			return a.withWorkspace(cmd.Context(), false, func(w workspace) error {
				packages, err := localPackages(w)
				if err != nil {
					return err
				}
				if asJSON {
					return jsonOutput(a.io.out, packages)
				}
				for _, name := range sortedKeys(packages) {
					if _, err := fmt.Fprintf(a.io.out, "%s\t%s\t%s\n",
						name, packages[name].Version, packages[name].Description,
					); err != nil {
						return err
					}
				}
				return nil
			})
		},
	}
	cmd.Flags().BoolVar(&asJSON, "json", false, "Output JSON")
	return cmd
}

func (a *application) addCommand() *cobra.Command {
	var asJSON bool
	cmd := &cobra.Command{
		Use: "add [NAME...]", Short: "List discoveries or install named packages and their dependencies",
		Args: arguments(packageNames),
		RunE: func(cmd *cobra.Command, args []string) error {
			if len(args) > 0 && asJSON {
				return &usageError{err: errors.New("--json is for discovery listing, not installation")}
			}
			return a.withWorkspace(cmd.Context(), len(args) > 0, func(w workspace) error {
				if len(args) > 0 {
					return syncPackages(cmd.Context(), w, syncOptions{names: args}, a.io.out)
				}
				r, err := readRegistry(w.dir)
				if err != nil {
					return err
				}
				if asJSON {
					return jsonOutput(a.io.out, r)
				}
				for _, name := range sortedKeys(r) {
					if _, err := fmt.Fprintf(a.io.out, "%s\t%s\n", name, r[name].Description); err != nil {
						return err
					}
				}
				return nil
			})
		},
	}
	cmd.Flags().BoolVar(&asJSON, "json", false, "Output discoveries as JSON")
	return cmd
}

func (a *application) pullCommand() *cobra.Command {
	var resume, abort bool
	cmd := &cobra.Command{
		Use: "pull [NAME...]", Short: "Update imported packages, leaving conflicts to native Git",
		Args: arguments(packageNames),
		RunE: func(cmd *cobra.Command, args []string) error {
			if (resume && abort) || ((resume || abort) && len(args) != 0) {
				return &usageError{err: errors.New("use --continue or --abort alone, without names")}
			}
			return a.withWorkspace(cmd.Context(), true, func(w workspace) error {
				if resume || abort {
					return recoverUpdate(cmd.Context(), w, abort)
				}
				return syncPackages(cmd.Context(), w, syncOptions{names: args, update: true}, a.io.out)
			})
		},
	}
	cmd.Flags().BoolVar(&resume, "continue", false, "Finish a resolved package merge and record its lock")
	cmd.Flags().BoolVar(&abort, "abort", false, "Abort an uncommitted package merge with git merge --abort")
	return cmd
}

func (a *application) restoreCommand() *cobra.Command {
	return &cobra.Command{
		Use: "restore", Short: "Install missing locked packages at their exact source commits",
		Args: arguments(cobra.NoArgs),
		RunE: func(cmd *cobra.Command, _ []string) error {
			return a.withWorkspace(cmd.Context(), true, func(w workspace) error {
				return syncPackages(cmd.Context(), w, syncOptions{restore: true}, a.io.out)
			})
		},
	}
}

func (a *application) indexCommand() *cobra.Command {
	var force bool
	cmd := &cobra.Command{
		Use: "index [NAME]", Short: "Generate indexes using each package's local template",
		Args: arguments(cobra.MatchAll(cobra.MaximumNArgs(1), packageNames)),
		RunE: func(cmd *cobra.Command, args []string) error {
			return a.withWorkspace(cmd.Context(), true, func(w workspace) error {
				if err := w.noPending(); err != nil {
					return err
				}
				packages, err := localPackages(w)
				if err != nil {
					return err
				}
				if len(args) == 0 {
					args = sortedKeys(packages)
				}
				for _, name := range args {
					if _, exists := packages[name]; !exists {
						return fmt.Errorf("unknown package %q", name)
					}
					if err := generateIndexes(filepath.Join(w.dir, name), force); err != nil {
						return err
					}
				}
				return rootIndex(w)
			})
		},
	}
	cmd.Flags().BoolVar(&force, "force", false, "Explicitly replace hand-authored package indexes")
	return cmd
}

func (a *application) validateCommand() *cobra.Command {
	return &cobra.Command{
		Use: "validate [NAME]", Short: "Validate OKF structure, package policies and dependency closure",
		Args: arguments(cobra.MatchAll(cobra.MaximumNArgs(1), packageNames)),
		RunE: func(cmd *cobra.Command, args []string) error {
			return a.withWorkspace(cmd.Context(), false, func(w workspace) error {
				if _, err := readRegistry(w.dir); err != nil {
					return err
				}
				if _, err := readLock(w.dir); err != nil {
					return err
				}
				return validateWorkspace(w, args)
			})
		},
	}
}

func (a *application) codeownersCommand() *cobra.Command {
	return &cobra.Command{
		Use: "codeowners", Short: "Generate ownership rules for locally authored packages",
		Args: arguments(cobra.NoArgs),
		RunE: func(cmd *cobra.Command, _ []string) error {
			return a.withWorkspace(cmd.Context(), true, func(w workspace) error {
				if err := w.noPending(); err != nil {
					return err
				}
				if err := generateCodeowners(w); err != nil {
					return err
				}
				_, err := fmt.Fprintln(a.io.err,
					"CODEOWNERS generated. Protect this file and require code-owner reviews in GitHub branch rules.",
				)
				return err
			})
		},
	}
}

func (a *application) proposeCommand() *cobra.Command {
	var options proposalOptions
	cmd := &cobra.Command{
		Use: "propose NAME", Short: "Prepare a source proposal; --publish pushes a branch and opens a PR",
		Args: arguments(cobra.MatchAll(cobra.ExactArgs(1), packageNames)),
		RunE: func(cmd *cobra.Command, args []string) error {
			options.name = args[0]
			return a.withWorkspace(cmd.Context(), true, func(w workspace) error {
				return propose(cmd.Context(), w, options, a.io.out)
			})
		},
	}
	cmd.Flags().StringVar(&options.title, "title", "", "Proposal commit and pull request title")
	cmd.Flags().StringVar(&options.body, "body", "", "Pull request body")
	cmd.Flags().StringVar(&options.branch, "branch", "", "Proposal branch (must start with gnosis/)")
	cmd.Flags().BoolVar(&options.publish, "publish", false, "Push proposal branch and create a PR using gh")
	return cmd
}

func (a *application) gitCommand(name string) *cobra.Command {
	return &cobra.Command{
		Use: name + " [GIT ARGUMENTS...]", Short: "Run git " + name + " in the consumer repository",
		DisableFlagParsing: true,
		RunE: func(cmd *cobra.Command, args []string) error {
			return a.withWorkspace(cmd.Context(), name != "status", func(w workspace) error {
				if name != "status" {
					if err := w.noPending(); err != nil {
						return err
					}
				}
				gitArgs := append([]string{name}, args...)
				process := gitCommand(cmd.Context(), w.root, gitArgs...)
				process.Stdin, process.Stdout, process.Stderr = a.io.in, a.io.out, a.io.err
				if err := process.Run(); err != nil {
					if cmd.Context().Err() != nil {
						return cmd.Context().Err()
					}
					var exit *exec.ExitError
					if errors.As(err, &exit) {
						return &gitExitError{code: exit.ExitCode(), err: err}
					}
					return err
				}
				return nil
			})
		},
	}
}
