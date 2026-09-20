package knowledge

import (
	"errors"
	"fmt"
	"os"
	"path"
	"path/filepath"
	"strings"
)

func SafePath(root, relative string) (string, error) {
	if err := ValidatePath(relative); err != nil {
		return "", err
	}
	current := root
	for _, part := range strings.Split(relative, "/") {
		current = filepath.Join(current, part)
		info, err := os.Lstat(current)
		if errors.Is(err, os.ErrNotExist) {
			continue
		}
		if err != nil {
			return "", err
		}
		if info.Mode()&os.ModeSymlink != 0 {
			return "", fmt.Errorf("symlink is not allowed: %s", current)
		}
	}
	return filepath.Join(root, filepath.FromSlash(relative)), nil
}

func ValidatePath(value string) error {
	if value == "" || strings.ContainsAny(value, "\\\x00\r\n") {
		return fmt.Errorf("invalid relative path %q", value)
	}
	if path.IsAbs(value) || path.Clean(value) != value || value == ".." || strings.HasPrefix(value, "../") {
		return fmt.Errorf("path must stay inside its package: %q", value)
	}
	for _, part := range strings.Split(value, "/") {
		if strings.EqualFold(part, ".git") {
			return fmt.Errorf("Git metadata path is not allowed: %q", value)
		}
	}
	return nil
}
