package workspace

import (
	"bytes"
	"errors"
	"fmt"
	"os"
	"path/filepath"

	"gnosis/internal/knowledge"
	builtin "gnosis/skills"
)

func InstallSkills(root string, checkOnly bool) error {
	names := []string{"gnosis-navigate", "gnosis-update", "gnosis-cli"}
	for _, name := range names {
		file, err := knowledge.SafePath(root, ".agents/skills/"+name+"/SKILL.md")
		if err != nil {
			return err
		}
		data, err := builtin.Files.ReadFile(name + "/SKILL.md")
		if err != nil {
			return err
		}
		existing, err := knowledge.ReadRegular(file)
		if err == nil && !bytes.Equal(existing, data) {
			return fmt.Errorf("%s already exists with different content; refusing to overwrite", file)
		}
		if err != nil && !errors.Is(err, os.ErrNotExist) {
			return err
		}
		if checkOnly {
			continue
		}
		if err := os.MkdirAll(filepath.Dir(file), 0o755); err != nil {
			return err
		}
		if err := knowledge.WriteAtomic(file, data); err != nil {
			return err
		}
	}
	return nil
}
