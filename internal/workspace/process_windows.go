package workspace

import (
	"context"
	"errors"
	"fmt"
	"os/exec"
	"strconv"
	"time"
)

func cancelProcessTree(cmd *exec.Cmd) {
	cmd.Cancel = func() error {
		// Cleanup must outlive the cancelled operation, but remain bounded itself.
		ctx, cancel := context.WithTimeout(context.Background(), 2*time.Second)
		defer cancel()
		stop := exec.CommandContext(ctx, "taskkill", "/F", "/T", "/PID", strconv.Itoa(cmd.Process.Pid))
		stop.WaitDelay = 2 * time.Second
		if output, err := stop.CombinedOutput(); err != nil {
			return errors.Join(fmt.Errorf("terminate process tree: %w: %s", err, output), cmd.Process.Kill())
		}
		return nil
	}
}
