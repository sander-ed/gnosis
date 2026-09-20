package cli

import (
	"gnosis/internal/workspace"

	"github.com/spf13/cobra"
)

func (a *application) skillsCommand() *cobra.Command {
	return &cobra.Command{
		Use: "skills", Short: "Install bundled agent meta skills without overwriting customizations",
		Args: arguments(cobra.NoArgs),
		RunE: func(cmd *cobra.Command, _ []string) error {
			return a.withWorkspace(cmd.Context(), true, func(w workspace.Workspace) error {
				if err := workspace.InstallSkills(w.Root, true); err != nil {
					return err
				}
				return workspace.InstallSkills(w.Root, false)
			})
		},
	}
}
