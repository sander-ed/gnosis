// Package knowledge implements package metadata, policies, and OKF documents.
package knowledge

import (
	"encoding/json"
	"fmt"
	"io/fs"
	"os"
	"path/filepath"
	"strings"
)

// ValidatePackage checks a bundle and returns its validated manifest.
func ValidatePackage(dir string) (Manifest, error) {
	m, _, _, err := scanPackage(dir, true)
	return m, err
}

func scanPackage(dir string, checkIndexes bool) (Manifest, standard, []concept, error) {
	m, err := ReadManifest(dir)
	if err != nil {
		return m, standard{}, nil, err
	}
	policy, schema, err := loadStandard(dir)
	if err != nil {
		return m, policy, nil, err
	}
	for _, relative := range policy.RequiredFiles {
		file, err := SafePath(dir, relative)
		if err != nil {
			return m, policy, nil, err
		}
		if _, err := ReadRegular(file); err != nil {
			return m, policy, nil, fmt.Errorf("required file: %w", err)
		}
	}
	concepts := []concept{}
	err = filepath.WalkDir(dir, func(file string, entry fs.DirEntry, walkErr error) error {
		if walkErr != nil {
			return walkErr
		}
		relative, err := filepath.Rel(dir, file)
		if err != nil {
			return err
		}
		relative = filepath.ToSlash(relative)
		if err := ValidatePath(relative); err != nil {
			return err
		}
		if entry.Type()&os.ModeSymlink != 0 {
			return fmt.Errorf("%s: symlinks are not supported in packages", relative)
		}
		if entry.IsDir() {
			return nil
		}
		if !entry.Type().IsRegular() {
			return fmt.Errorf("%s: package entries must be regular files", relative)
		}
		if filepath.Ext(file) != ".md" {
			return nil
		}
		data, err := ReadRegular(file)
		if err != nil {
			return err
		}
		if entry.Name() == "index.md" || entry.Name() == "log.md" {
			if entry.Name() == "index.md" && !checkIndexes {
				return nil
			}
			return validateReserved(relative, data)
		}
		fields, body, err := Frontmatter(data)
		if err != nil {
			return fmt.Errorf("%s: %w", relative, err)
		}
		kind, ok := fields["type"].(string)
		if !ok || strings.TrimSpace(kind) == "" {
			return fmt.Errorf("%s: OKF requires a nonempty string type", relative)
		}
		// Normalize YAML timestamps and numbers to JSON Schema's data model.
		jsonBytes, err := json.Marshal(fields)
		if err != nil {
			return fmt.Errorf("%s: frontmatter must use JSON-compatible keys and values: %w", relative, err)
		}
		var value any
		if err := json.Unmarshal(jsonBytes, &value); err != nil {
			return err
		}
		if err := schema.Validate(value); err != nil {
			return fmt.Errorf("%s: package frontmatter policy: %w", relative, err)
		}
		for _, heading := range policy.RequiredHeads {
			if !strings.Contains("\n"+body+"\n", "\n"+heading+"\n") {
				return fmt.Errorf("%s: required heading %q is missing", relative, heading)
			}
		}
		if policy.MaxWords > 0 && len(strings.Fields(body)) > policy.MaxWords {
			return fmt.Errorf("%s: body exceeds max_words %d", relative, policy.MaxWords)
		}
		title, _ := fields["title"].(string)
		if title == "" {
			title = strings.TrimSuffix(entry.Name(), ".md")
		}
		description, _ := fields["description"].(string)
		concepts = append(concepts, concept{Path: relative, Title: title, Description: description})
		return nil
	})
	return m, policy, concepts, err
}

func ScaffoldPackage(dir string, m Manifest) error {
	if err := m.Validate(); err != nil {
		return err
	}
	if err := os.Mkdir(dir, 0o755); err != nil {
		return err
	}
	for _, name := range []string{"standard.yml", "frontmatter.schema.json", "index.tmpl"} {
		data, err := assets.ReadFile("assets/" + name)
		if err != nil {
			return err
		}
		if err := WriteAtomic(filepath.Join(dir, name), data); err != nil {
			return err
		}
	}
	if err := WriteData(filepath.Join(dir, "package.yml"), m); err != nil {
		return err
	}
	return GenerateIndexes(dir, false)
}
