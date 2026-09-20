package knowledge

import (
	"errors"
	"fmt"
	"path/filepath"
	"strings"
)

type Source struct {
	Type          string `json:"type" yaml:"type"`
	Repository    string `json:"repository" yaml:"repository"`
	Path          string `json:"path" yaml:"path"`
	DefaultBranch string `json:"default_branch" yaml:"default_branch"`
}

type RegistryEntry struct {
	Description string `json:"description" yaml:"description"`
	Owner       string `json:"owner" yaml:"owner"`
	Source      Source `json:"source" yaml:"source"`
}

type Registry map[string]RegistryEntry

func (entry RegistryEntry) Validate(name string) error {
	if err := ValidateName(name); err != nil {
		return err
	}
	if !ownerPattern.MatchString(entry.Owner) {
		return fmt.Errorf("%s: registry owner must be @user or @organization/team", name)
	}
	if err := entry.Source.Validate(); err != nil {
		return fmt.Errorf("%s: %w", name, err)
	}
	return nil
}

func (s Source) Validate() error {
	if s.Type != "git" {
		return fmt.Errorf("unsupported source type %q; expected git", s.Type)
	}
	if err := ValidatePath(s.Path); err != nil {
		return err
	}
	if s.Repository == "" || strings.HasPrefix(s.Repository, "-") || strings.ContainsAny(s.Repository, "\x00\r\n") {
		return errors.New("source.repository must be a Git URL or absolute local path")
	}
	local := !strings.Contains(s.Repository, ":")
	if local && !filepath.IsAbs(s.Repository) {
		return errors.New("local source.repository must be an absolute path")
	}
	if s.DefaultBranch == "" || strings.HasPrefix(s.DefaultBranch, "-") {
		return errors.New("source.default_branch must be a branch name")
	}
	return nil
}

func ReadRegistry(dir string) (Registry, error) {
	r := Registry{}
	if err := ReadData(filepath.Join(dir, "registry.yml"), &r); err != nil {
		return nil, err
	}
	if r == nil {
		return nil, errors.New("registry must be a mapping, not null")
	}
	for name, entry := range r {
		if err := entry.Validate(name); err != nil {
			return nil, err
		}
	}
	return r, nil
}
