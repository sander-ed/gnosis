package cli

import (
	"errors"
	"fmt"

	"gnosis/internal/workspace"

	"github.com/spf13/cobra"
)

func (a *application) initCommand() *cobra.Command {
	install := true
	cmd := &cobra.Command{
		Use: "init", Short: "Initialize gnosis/ in an existing Git repository",
		Args: arguments(cobra.NoArgs),
		RunE: func(cmd *cobra.Command, _ []string) (err error) {
			w, err := workspace.Open(cmd.Context(), a.cwd)
			if err != nil {
				return err
			}
			release, err := w.Acquire()
			if err != nil {
				return err
			}
			defer func() { err = errors.Join(err, release()) }()
			if err := workspace.Init(w, install); err != nil {
				return err
			}
			_, err = fmt.Fprintf(a.io.Out, "Initialized %s\n", w.Dir)
			return err
		},
	}
	cmd.Flags().BoolVar(&install, "skills", true, "Install gnosis meta skills into .agents/skills")
	return cmd
}
