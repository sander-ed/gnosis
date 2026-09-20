package workspace

import (
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"strings"

	"gnosis/internal/knowledge"
)

func GenerateCodeowners(w Workspace) error {
	packages, err := Packages(w)
	if err != nil {
		return err
	}
	lock, err := knowledge.ReadLock(w.Dir)
	if err != nil {
		return err
	}
	file := ""
	data := []byte{}
	for _, candidate := range []string{".github/CODEOWNERS", "CODEOWNERS", "docs/CODEOWNERS"} {
		candidatePath, err := knowledge.SafePath(w.Root, candidate)
		if err != nil {
			return err
		}
		content, err := knowledge.ReadRegular(candidatePath)
		if errors.Is(err, os.ErrNotExist) {
			continue
		}
		if err != nil {
			return err
		}
		file, data = candidatePath, content
		break
	}
	if file == "" {
		file, err = knowledge.SafePath(w.Root, ".github/CODEOWNERS")
		if err != nil {
			return err
		}
	}
	const begin = "# BEGIN GNOSIS OWNERS"
	const end = "# END GNOSIS OWNERS"
	text := string(data)
	start, finish := strings.Index(text, begin), strings.Index(text, end)
	if (start < 0) != (finish < 0) || finish < start {
		return errors.New("malformed gnosis CODEOWNERS block; repair it before regeneration")
	}
	if strings.Count(text, begin) > 1 || strings.Count(text, end) > 1 {
		return errors.New("duplicate gnosis CODEOWNERS blocks")
	}
	var block strings.Builder
	block.WriteString(begin + "\n")
	for _, name := range knowledge.SortedKeys(packages) {
		if _, imported := lock.Packages[name]; imported {
			continue
		}
		handle, err := packages[name].Owner.Handle()
		if err != nil {
			return err
		}
		fmt.Fprintf(&block, "/gnosis/%s/ %s\n", name, handle)
	}
	block.WriteString(end)
	if start >= 0 {
		text = text[:start] + block.String() + text[finish+len(end):]
	} else {
		text = strings.TrimRight(text, "\n") + "\n\n" + block.String() + "\n"
		text = strings.TrimLeft(text, "\n")
	}
	if err := os.MkdirAll(filepath.Dir(file), 0o755); err != nil {
		return err
	}
	return knowledge.WriteAtomic(file, []byte(text))
}
