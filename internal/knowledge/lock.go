package knowledge

import (
	"errors"
	"fmt"
	"path/filepath"
	"strings"
)

type LockedPackage struct {
	Source        Source   `json:"source" yaml:"source"`
	Version       string   `json:"version" yaml:"version"`
	Commit        string   `json:"commit" yaml:"commit"`
	SubtreeCommit string   `json:"subtree_commit" yaml:"subtree_commit"`
	Dependencies  []string `json:"dependencies" yaml:"dependencies"`
}

type Lockfile struct {
	GnosisVersion int                      `json:"gnosis_version" yaml:"gnosis_version"`
	Packages      map[string]LockedPackage `json:"packages" yaml:"packages"`
}

func IsCommit(value string) bool {
	return shaPattern.MatchString(value)
}

func ReadLock(dir string) (Lockfile, error) {
	var lock Lockfile
	if err := ReadData(filepath.Join(dir, "gnosis.lock"), &lock); err != nil {
		return lock, err
	}
	if lock.GnosisVersion != 1 || lock.Packages == nil {
		return lock, errors.New("invalid gnosis.lock: expected gnosis_version 1 and packages mapping")
	}
	for name, p := range lock.Packages {
		if err := ValidateName(name); err != nil {
			return lock, err
		}
		if err := p.Source.Validate(); err != nil {
			return lock, err
		}
		if !IsCommit(p.Commit) || !IsCommit(p.SubtreeCommit) {
			return lock, fmt.Errorf("%s: lock requires full Git commit IDs", name)
		}
		if strings.TrimSpace(p.Version) == "" {
			return lock, fmt.Errorf("%s: lock version is required", name)
		}
		seen := map[string]bool{}
		for _, dep := range p.Dependencies {
			if err := ValidateName(dep); err != nil {
				return lock, err
			}
			if dep == name || seen[dep] {
				return lock, fmt.Errorf("%s: self or duplicate dependency in lock", name)
			}
			seen[dep] = true
		}
	}
	return lock, nil
}
