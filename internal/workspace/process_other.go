//go:build !unix && !windows

package workspace

import (
	"os/exec"
)

func cancelProcessTree(cmd *exec.Cmd) {
	cmd.Cancel = func() error { return cmd.Process.Kill() }
}
