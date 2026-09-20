package cli

import (
	"fmt"

	"gnosis/internal/workspace"

	"github.com/spf13/cobra"
)

func (a *application) codeownersCommand() *cobra.Command {
	return &cobra.Command{
		Use: "codeowners", Short: "Generate ownership rules for locally authored packages",
		Args: arguments(cobra.NoArgs),
		RunE: func(cmd *cobra.Command, _ []string) error {
			return a.withWorkspace(cmd.Context(), true, func(w workspace.Workspace) error {
				if err := w.NoPending(); err != nil {
					return err
				}
				if err := workspace.GenerateCodeowners(w); err != nil {
					return err
				}
				_, err := fmt.Fprintln(a.io.Err,
					"CODEOWNERS generated. Protect this file and require code-owner reviews in GitHub branch rules.",
				)
				return err
			})
		},
	}
}
