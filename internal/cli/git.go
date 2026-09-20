package cli

import (
	"errors"
	"os/exec"

	"gnosis/internal/workspace"

	"github.com/spf13/cobra"
)

func (a *application) gitCommand(name string) *cobra.Command {
	return &cobra.Command{
		Use: name + " [GIT ARGUMENTS...]", Short: "Run git " + name + " in the consumer repository",
		DisableFlagParsing: true,
		RunE: func(cmd *cobra.Command, args []string) error {
			return a.withWorkspace(cmd.Context(), name != "status", func(w workspace.Workspace) error {
				if name != "status" {
					if err := w.NoPending(); err != nil {
						return err
					}
				}
				gitArgs := append([]string{name}, args...)
				process := workspace.GitCommand(cmd.Context(), w.Root, gitArgs...)
				process.Stdin, process.Stdout, process.Stderr = a.io.In, a.io.Out, a.io.Err
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
