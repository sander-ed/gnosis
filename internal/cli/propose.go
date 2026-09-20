package cli

import (
	"gnosis/internal/workspace"

	"github.com/spf13/cobra"
)

func (a *application) proposeCommand() *cobra.Command {
	var options workspace.ProposalOptions
	cmd := &cobra.Command{
		Use: "propose NAME", Short: "Prepare a source proposal; --publish pushes a branch and opens a PR",
		Args: arguments(cobra.MatchAll(cobra.ExactArgs(1), packageNames)),
		RunE: func(cmd *cobra.Command, args []string) error {
			options.Name = args[0]
			return a.withWorkspace(cmd.Context(), true, func(w workspace.Workspace) error {
				return workspace.Propose(cmd.Context(), w, options, a.io.Out)
			})
		},
	}
	cmd.Flags().StringVar(&options.Title, "title", "", "Proposal commit and pull request title")
	cmd.Flags().StringVar(&options.Body, "body", "", "Pull request body")
	cmd.Flags().StringVar(&options.Branch, "branch", "", "Proposal branch (must start with gnosis/)")
	cmd.Flags().BoolVar(&options.Publish, "publish", false, "Push proposal branch and create a PR using gh")
	return cmd
}
