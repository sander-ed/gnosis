package cli

import (
	"errors"
	"strings"

	"gnosis/internal/knowledge"
	"gnosis/internal/workspace"

	"github.com/spf13/cobra"
)

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
			o := knowledge.Owner{User: handle}
			if strings.Contains(handle, "/") {
				o = knowledge.Owner{Team: handle}
			}
			m := knowledge.Manifest{
				GnosisVersion: 1, Package: args[0], Version: "0.1.0",
				Description: description, Owner: o, Dependencies: dependencies,
			}
			if err := m.Validate(); err != nil {
				return &usageError{err: err}
			}
			return a.withWorkspace(cmd.Context(), true, func(w workspace.Workspace) error {
				if err := w.NoPending(); err != nil {
					return err
				}
				dir, err := knowledge.SafePath(w.Dir, m.Package)
				if err != nil {
					return err
				}
				if err := knowledge.ScaffoldPackage(dir, m); err != nil {
					return err
				}
				return workspace.GenerateIndex(w)
			})
		},
	}
	cmd.Flags().StringVar(&handle, "owner", "", "One GitHub @user or @organization/team (required)")
	if err := cmd.MarkFlagRequired("owner"); err != nil {
		panic(err)
	}
	cmd.Flags().StringVar(&description, "description", "", "Package summary used in the index")
	cmd.Flags().StringSliceVar(&dependencies, "dependency", []string{}, "Dependency names (repeat or comma-separate)")
	parent.AddCommand(cmd)
	return parent
}
