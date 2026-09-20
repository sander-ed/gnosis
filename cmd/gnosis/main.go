package main

import (
	"context"
	"os"
	"os/signal"
	"syscall"

	"gnosis/internal/cli"
)

var version = "dev"

func main() {
	ctx, stop := signal.NotifyContext(context.Background(), os.Interrupt, syscall.SIGTERM)
	code := cli.Execute(ctx, os.Args[1:], cli.Streams{In: os.Stdin, Out: os.Stdout, Err: os.Stderr}, version)
	stop()
	os.Exit(code)
}
