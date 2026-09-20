package knowledge

import (
	"errors"
	"fmt"
	"path/filepath"
	"strings"
	"time"
	"unicode/utf8"
)

func Frontmatter(data []byte) (map[string]any, string, error) {
	if !utf8.Valid(data) {
		return nil, "", errors.New("document must be UTF-8")
	}
	text := strings.ReplaceAll(string(data), "\r\n", "\n")
	lines := strings.Split(text, "\n")
	if len(lines) == 0 || lines[0] != "---" {
		return nil, "", errors.New("concept must start with YAML frontmatter (---)")
	}
	for i := 1; i < len(lines); i++ {
		if lines[i] != "---" {
			continue
		}
		fields := map[string]any{}
		if err := Decode([]byte(strings.Join(lines[1:i], "\n")), &fields); err != nil {
			return nil, "", fmt.Errorf("frontmatter: %w", err)
		}
		if fields == nil {
			return nil, "", errors.New("frontmatter must be a mapping")
		}
		return fields, strings.Join(lines[i+1:], "\n"), nil
	}
	return nil, "", errors.New("frontmatter is missing its closing ---")
}

func validateReserved(relative string, data []byte) error {
	if !utf8.Valid(data) {
		return fmt.Errorf("%s must be UTF-8", relative)
	}
	text := strings.ReplaceAll(string(data), "\r\n", "\n")
	if strings.HasPrefix(text, "---\n") {
		if relative != "index.md" {
			return fmt.Errorf("%s: only the bundle-root index.md may contain frontmatter", relative)
		}
		fields, _, err := Frontmatter(data)
		if err != nil {
			return err
		}
		if len(fields) != 1 {
			return errors.New("root index frontmatter may contain only okf_version")
		}
		if value, ok := fields["okf_version"].(string); !ok || strings.TrimSpace(value) == "" {
			return errors.New("root index requires a string okf_version")
		}
	}
	if filepath.Base(relative) == "log.md" {
		for _, line := range strings.Split(text, "\n") {
			if strings.HasPrefix(line, "## ") {
				if _, err := time.Parse("2006-01-02", strings.TrimPrefix(line, "## ")); err != nil {
					return fmt.Errorf("%s: log date headings must be ## YYYY-MM-DD", relative)
				}
			}
		}
	}
	return nil
}
