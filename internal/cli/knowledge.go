package cli

import (
	"fmt"
	"path/filepath"

	"gnosis/internal/knowledge"
	"gnosis/internal/workspace"

	"github.com/spf13/cobra"
)

func (a *application) listCommand() *cobra.Command {
	var asJSON bool
	cmd := &cobra.Command{
		Use: "list", Short: "List installed packages",
		Args: arguments(cobra.NoArgs),
		RunE: func(cmd *cobra.Command, _ []string) error {
			return a.withWorkspace(cmd.Context(), false, func(w workspace.Workspace) error {
				packages, err := workspace.Packages(w)
				if err != nil {
					return err
				}
				if asJSON {
					return jsonOutput(a.io.Out, packages)
				}
				for _, name := range knowledge.SortedKeys(packages) {
					if _, err := fmt.Fprintf(a.io.Out, "%s\t%s\t%s\n",
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

func (a *application) indexCommand() *cobra.Command {
	var force bool
	cmd := &cobra.Command{
		Use: "index [NAME]", Short: "Generate indexes using each package's local template",
		Args: arguments(cobra.MatchAll(cobra.MaximumNArgs(1), packageNames)),
		RunE: func(cmd *cobra.Command, args []string) error {
			return a.withWorkspace(cmd.Context(), true, func(w workspace.Workspace) error {
				if err := w.NoPending(); err != nil {
					return err
				}
				packages, err := workspace.Packages(w)
				if err != nil {
					return err
				}
				if len(args) == 0 {
					args = knowledge.SortedKeys(packages)
				}
				for _, name := range args {
					if _, exists := packages[name]; !exists {
						return fmt.Errorf("unknown package %q", name)
					}
					if err := knowledge.GenerateIndexes(filepath.Join(w.Dir, name), force); err != nil {
						return err
					}
				}
				return workspace.GenerateIndex(w)
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
			return a.withWorkspace(cmd.Context(), false, func(w workspace.Workspace) error {
				if _, err := knowledge.ReadRegistry(w.Dir); err != nil {
					return err
				}
				if _, err := knowledge.ReadLock(w.Dir); err != nil {
					return err
				}
				return workspace.Validate(w, args)
			})
		},
	}
}
