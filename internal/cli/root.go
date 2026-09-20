// Package cli defines command routing, input/output, and exit semantics.
package cli

import (
	"context"
	"errors"
	"fmt"
	"io"

	"gnosis/internal/knowledge"
	"gnosis/internal/workspace"

	"github.com/spf13/cobra"
)

// Streams supplies the command's input and its separate output and diagnostic writers.
type Streams struct {
	In  io.Reader
	Out io.Writer
	Err io.Writer
}

type application struct {
	cwd     string
	io      Streams
	version string
	started bool
}

func (a *application) withWorkspace(ctx context.Context, mutate bool, fn func(workspace.Workspace) error) (err error) {
	w, err := workspace.Open(ctx, a.cwd)
	if err != nil {
		return err
	}
	if err := w.Initialized(); err != nil {
		return err
	}
	if mutate {
		release, err := w.Acquire()
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
		if err := knowledge.ValidateName(name); err != nil {
			return err
		}
	}
	return nil
}

func (a *application) command() *cobra.Command {
	var input string
	root := &cobra.Command{
		Use: "gnosis", Short: "Manage Git-backed Open Knowledge Format packages",
		Version: a.version, SilenceUsage: true, SilenceErrors: true,
		TraverseChildren: true,
		Args:             arguments(cobra.NoArgs),
		PersistentPreRunE: func(cmd *cobra.Command, _ []string) error {
			if err := cmd.ValidateRequiredFlags(); err != nil {
				return &usageError{err: err}
			}
			if err := cmd.ValidateFlagGroups(); err != nil {
				return &usageError{err: err}
			}
			a.started = true
			return nil
		},
		RunE: func(cmd *cobra.Command, _ []string) error {
			if cmd.Flags().Changed("input") {
				_, err := fmt.Fprintln(a.io.Out, input)
				return err
			}
			return cmd.Help()
		},
	}
	root.SetIn(a.io.In)
	root.SetOut(a.io.Out)
	root.SetErr(a.io.Err)
	root.SetFlagErrorFunc(func(_ *cobra.Command, err error) error { return &usageError{err: err} })
	root.PersistentFlags().StringVarP(&a.cwd, "directory", "C", a.cwd, "Run in this repository directory")
	root.Flags().StringVarP(&input, "input", "i", "", "Echo text (legacy proof-of-concept compatibility)")
	root.AddCommand(a.initCommand(), a.packageCommand(), a.registryCommand())
	root.AddCommand(a.listCommand(), a.addCommand(), a.pullCommand(), a.restoreCommand())
	root.AddCommand(a.indexCommand(), a.validateCommand(), a.codeownersCommand(), a.proposeCommand())
	for _, name := range []string{"status", "merge", "rebase"} {
		root.AddCommand(a.gitCommand(name))
	}
	root.AddCommand(a.skillsCommand())
	return root
}
