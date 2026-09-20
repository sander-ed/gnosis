package main

import (
	"context"
	"errors"
	"fmt"
	"io"
	"os"
	"os/signal"
	"syscall"
)

func main() {
	ctx, stop := signal.NotifyContext(context.Background(), os.Interrupt, syscall.SIGTERM)
	code := execute(ctx, os.Args[1:], streams{in: os.Stdin, out: os.Stdout, err: os.Stderr})
	stop()
	os.Exit(code)
}

func run(args []string, stdout, stderr io.Writer) int {
	return execute(context.Background(), args, streams{in: os.Stdin, out: stdout, err: stderr})
}

func execute(ctx context.Context, args []string, io streams) int {
	cwd, err := os.Getwd()
	if err != nil {
		fmt.Fprintln(io.err, "gnosis:", err)
		return 1
	}
	app := application{cwd: cwd, io: io}
	cmd := app.command()
	cmd.SetArgs(args)
	if err := cmd.ExecuteContext(ctx); err != nil {
		fmt.Fprintln(io.err, "gnosis:", err)
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
