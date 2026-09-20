package workspace

import (
	"os/exec"
	"time"
)

func manageProcess(cmd *exec.Cmd) {
	cmd.WaitDelay = 2 * time.Second
	cancelProcessTree(cmd)
}
