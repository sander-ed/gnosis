package knowledge

import (
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"regexp"
	"strings"
)

type Owner struct {
	User string `json:"user,omitempty" yaml:"user,omitempty"`
	Team string `json:"team,omitempty" yaml:"team,omitempty"`
}

func (o Owner) Handle() (string, error) {
	if (o.User == "") == (o.Team == "") {
		return "", errors.New("owner must have exactly one of user or team")
	}
	value := o.User
	if o.Team != "" {
		value = o.Team
		if !strings.Contains(value, "/") {
			return "", errors.New("owner.team must be @organization/team")
		}
	}
	if !ownerPattern.MatchString(value) {
		return "", fmt.Errorf("invalid GitHub owner %q; use @user or @organization/team", value)
	}
	if o.User != "" && strings.Contains(value, "/") {
		return "", errors.New("owner.user must be @username")
	}
	return value, nil
}

type Manifest struct {
	GnosisVersion int      `json:"gnosis_version" yaml:"gnosis_version"`
	Package       string   `json:"package" yaml:"package"`
	Version       string   `json:"version" yaml:"version"`
	Description   string   `json:"description" yaml:"description"`
	Owner         Owner    `json:"owner" yaml:"owner"`
	Dependencies  []string `json:"dependencies" yaml:"dependencies"`
}

var (
	namePattern  = regexp.MustCompile(`^[a-z0-9]+(-[a-z0-9]+)*$`)
	ownerPattern = regexp.MustCompile(`^@[A-Za-z0-9][A-Za-z0-9-]*(/[A-Za-z0-9][A-Za-z0-9-]*)?$`)
	shaPattern   = regexp.MustCompile(`^([0-9a-f]{40}|[0-9a-f]{64})$`)
)

func ValidateName(name string) error {
	if len(name) > 64 || !namePattern.MatchString(name) {
		return fmt.Errorf("invalid package name %q; use 1-64 lowercase letters, digits and single hyphens", name)
	}
	return nil
}

func (m Manifest) Validate() error {
	if m.GnosisVersion != 1 {
		return fmt.Errorf("unsupported gnosis_version %d", m.GnosisVersion)
	}
	if err := ValidateName(m.Package); err != nil {
		return err
	}
	if strings.TrimSpace(m.Version) == "" {
		return errors.New("package version is required")
	}
	if _, err := m.Owner.Handle(); err != nil {
		return err
	}
	seen := map[string]bool{}
	for _, dependency := range m.Dependencies {
		if err := ValidateName(dependency); err != nil {
			return err
		}
		if dependency == m.Package || seen[dependency] {
			return fmt.Errorf("self or duplicate dependency %q in %s", dependency, m.Package)
		}
		seen[dependency] = true
	}
	return nil
}

func ReadManifest(dir string) (Manifest, error) {
	var m Manifest
	found := ""
	for _, name := range []string{"package.yml", "package.yaml", "package.json"} {
		file := filepath.Join(dir, name)
		_, err := os.Lstat(file)
		if errors.Is(err, os.ErrNotExist) {
			continue
		}
		if err != nil {
			return m, err
		}
		if found != "" {
			return m, fmt.Errorf("%s: ambiguous manifests %s and %s", dir, found, name)
		}
		found = name
		if err := ReadData(file, &m); err != nil {
			return m, err
		}
	}
	if found == "" {
		return m, fmt.Errorf("%s: no package.yml, package.yaml or package.json", dir)
	}
	if m.Dependencies == nil {
		m.Dependencies = []string{}
	}
	return m, m.Validate()
}
