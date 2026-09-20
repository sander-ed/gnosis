package cli

import (
	"context"
	"io"
	"os"
)

func run(args []string, stdout, stderr io.Writer) int {
	return Execute(context.Background(), args, Streams{In: os.Stdin, Out: stdout, Err: stderr}, "dev")
}
