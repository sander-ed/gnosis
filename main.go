package main

import (
	"flag"
	"fmt"
	"io"
	"os"
)

func main() {
	os.Exit(run(os.Args[1:], os.Stdout, os.Stderr))
}

func run(args []string, stdout, stderr io.Writer) int {
	flags := flag.NewFlagSet("gnosis", flag.ContinueOnError)
	flags.SetOutput(stderr)

	var input string
	flags.StringVar(&input, "input", "", "Text to return")
	flags.StringVar(&input, "i", "", "Text to return (shorthand for --input)")

	if err := flags.Parse(args); err != nil {
		if err == flag.ErrHelp {
			return 0
		}
		return 2
	}
	if flags.NArg() != 0 {
		fmt.Fprintln(stderr, "gnosis: unexpected positional arguments; use --input or -i")
		return 2
	}

	if _, err := fmt.Fprintln(stdout, input); err != nil {
		fmt.Fprintln(stderr, "gnosis:", err)
		return 1
	}
	return 0
}
