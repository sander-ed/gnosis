package cli

import (
	"errors"
	"fmt"

	"gnosis/internal/knowledge"
	"gnosis/internal/workspace"

	"github.com/spf13/cobra"
)

func (a *application) addCommand() *cobra.Command {
	var asJSON bool
	cmd := &cobra.Command{
		Use: "add [NAME...]", Short: "List discoveries or install named packages and their dependencies",
		Args: arguments(packageNames),
		RunE: func(cmd *cobra.Command, args []string) error {
			if len(args) > 0 && asJSON {
				return &usageError{err: errors.New("--json is for discovery listing, not installation")}
			}
			return a.withWorkspace(cmd.Context(), len(args) > 0, func(w workspace.Workspace) error {
				if len(args) > 0 {
					return workspace.Sync(cmd.Context(), w, workspace.SyncOptions{Names: args}, a.io.Out)
				}
				r, err := knowledge.ReadRegistry(w.Dir)
				if err != nil {
					return err
				}
				if asJSON {
					return jsonOutput(a.io.Out, r)
				}
				for _, name := range knowledge.SortedKeys(r) {
					if _, err := fmt.Fprintf(a.io.Out, "%s\t%s\n", name, r[name].Description); err != nil {
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
			if (resume || abort) && len(args) != 0 {
				return &usageError{err: errors.New("use --continue or --abort alone, without names")}
			}
			return a.withWorkspace(cmd.Context(), true, func(w workspace.Workspace) error {
				if resume || abort {
					return workspace.Recover(cmd.Context(), w, abort)
				}
				return workspace.Sync(cmd.Context(), w, workspace.SyncOptions{Names: args, Update: true}, a.io.Out)
			})
		},
	}
	cmd.Flags().BoolVar(&resume, "continue", false, "Finish a resolved package merge and record its lock")
	cmd.Flags().BoolVar(&abort, "abort", false, "Abort an uncommitted package merge with git merge --abort")
	cmd.MarkFlagsMutuallyExclusive("continue", "abort")
	return cmd
}

func (a *application) restoreCommand() *cobra.Command {
	return &cobra.Command{
		Use: "restore", Short: "Install missing locked packages at their exact source commits",
		Args: arguments(cobra.NoArgs),
		RunE: func(cmd *cobra.Command, _ []string) error {
			return a.withWorkspace(cmd.Context(), true, func(w workspace.Workspace) error {
				return workspace.Sync(cmd.Context(), w, workspace.SyncOptions{Restore: true}, a.io.Out)
			})
		},
	}
}
