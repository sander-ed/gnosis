package cli

import (
	"context"
	"errors"
	"fmt"
	"os"
)

// Execute runs a command and returns its process exit code without exiting the caller.
func Execute(ctx context.Context, args []string, io Streams, version string) int {
	cwd, err := os.Getwd()
	if err != nil {
		fmt.Fprintln(io.Err, "gnosis:", err)
		return 1
	}
	app := application{cwd: cwd, io: io, version: version}
	cmd := app.command()
	cmd.SetArgs(args)
	if err := cmd.ExecuteContext(ctx); err != nil {
		fmt.Fprintln(io.Err, "gnosis:", err)
		var usage *usageError
		var gitErr *gitExitError
		switch {
		case errors.Is(err, context.Canceled):
			return 130
		case errors.As(err, &gitErr):
			return gitErr.code
		case errors.As(err, &usage), !app.started:
			return 2
		default:
			return 1
		}
	}
	return 0
}
