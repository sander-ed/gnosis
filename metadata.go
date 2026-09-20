package main

import (
	"bytes"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"os"
	"path"
	"path/filepath"
	"regexp"
	"slices"
	"strings"

	"go.yaml.in/yaml/v3"
)

type owner struct {
	User string `json:"user,omitempty" yaml:"user,omitempty"`
	Team string `json:"team,omitempty" yaml:"team,omitempty"`
}

func (o owner) handle() (string, error) {
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

type manifest struct {
	GnosisVersion int      `json:"gnosis_version" yaml:"gnosis_version"`
	Package       string   `json:"package" yaml:"package"`
	Version       string   `json:"version" yaml:"version"`
	Description   string   `json:"description" yaml:"description"`
	Owner         owner    `json:"owner" yaml:"owner"`
	Dependencies  []string `json:"dependencies" yaml:"dependencies"`
}

type source struct {
	Type          string `json:"type" yaml:"type"`
	Repository    string `json:"repository" yaml:"repository"`
	Path          string `json:"path" yaml:"path"`
	DefaultBranch string `json:"default_branch" yaml:"default_branch"`
}

type registryEntry struct {
	Description string `json:"description" yaml:"description"`
	Owner       string `json:"owner" yaml:"owner"`
	Source      source `json:"source" yaml:"source"`
}

type registry map[string]registryEntry

type lockedPackage struct {
	Source        source   `json:"source" yaml:"source"`
	Version       string   `json:"version" yaml:"version"`
	Commit        string   `json:"commit" yaml:"commit"`
	SubtreeCommit string   `json:"subtree_commit" yaml:"subtree_commit"`
	Dependencies  []string `json:"dependencies" yaml:"dependencies"`
}

type lockfile struct {
	GnosisVersion int                      `json:"gnosis_version" yaml:"gnosis_version"`
	Packages      map[string]lockedPackage `json:"packages" yaml:"packages"`
}

type standard struct {
	GnosisVersion int      `json:"gnosis_version" yaml:"gnosis_version"`
	Schema        string   `json:"schema" yaml:"schema"`
	IndexTemplate string   `json:"index_template" yaml:"index_template"`
	RequiredFiles []string `json:"required_files" yaml:"required_files"`
	RequiredHeads []string `json:"required_headings" yaml:"required_headings"`
	MaxWords      int      `json:"max_words" yaml:"max_words"`
}

var (
	namePattern  = regexp.MustCompile(`^[a-z0-9]+(-[a-z0-9]+)*$`)
	ownerPattern = regexp.MustCompile(`^@[A-Za-z0-9][A-Za-z0-9-]*(/[A-Za-z0-9][A-Za-z0-9-]*)?$`)
	shaPattern   = regexp.MustCompile(`^([0-9a-f]{40}|[0-9a-f]{64})$`)
)

func validName(name string) error {
	if len(name) > 64 || !namePattern.MatchString(name) {
		return fmt.Errorf("invalid package name %q; use 1-64 lowercase letters, digits and single hyphens", name)
	}
	return nil
}

func validPath(value string) error {
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

func (s source) validate() error {
	if s.Type != "git" {
		return fmt.Errorf("unsupported source type %q; expected git", s.Type)
	}
	if err := validPath(s.Path); err != nil {
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

func (m manifest) validate() error {
	if m.GnosisVersion != 1 {
		return fmt.Errorf("unsupported gnosis_version %d", m.GnosisVersion)
	}
	if err := validName(m.Package); err != nil {
		return err
	}
	if strings.TrimSpace(m.Version) == "" {
		return errors.New("package version is required")
	}
	if _, err := m.Owner.handle(); err != nil {
		return err
	}
	seen := map[string]bool{}
	for _, dependency := range m.Dependencies {
		if err := validName(dependency); err != nil {
			return err
		}
		if dependency == m.Package || seen[dependency] {
			return fmt.Errorf("self or duplicate dependency %q in %s", dependency, m.Package)
		}
		seen[dependency] = true
	}
	return nil
}

func decode(data []byte, out any) error {
	decoder := yaml.NewDecoder(bytes.NewReader(data))
	decoder.KnownFields(true)
	if err := decoder.Decode(out); err != nil {
		return err
	}
	var extra any
	if err := decoder.Decode(&extra); err != io.EOF {
		if err != nil {
			return err
		}
		return errors.New("expected exactly one YAML or JSON document")
	}
	return nil
}

func readData(file string, out any) error {
	data, err := readRegular(file)
	if err != nil {
		return err
	}
	if err := decode(data, out); err != nil {
		return fmt.Errorf("%s: %w", file, err)
	}
	return nil
}

func readRegular(file string) ([]byte, error) {
	info, err := os.Lstat(file)
	if err != nil {
		return nil, err
	}
	if !info.Mode().IsRegular() {
		return nil, fmt.Errorf("%s: expected a regular file (symlinks are not supported)", file)
	}
	if info.Size() > 16<<20 {
		return nil, fmt.Errorf("%s: metadata/document exceeds 16 MiB", file)
	}
	return os.ReadFile(file)
}

func readManifest(dir string) (manifest, error) {
	var m manifest
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
		if err := readData(file, &m); err != nil {
			return m, err
		}
	}
	if found == "" {
		return m, fmt.Errorf("%s: no package.yml, package.yaml or package.json", dir)
	}
	if m.Dependencies == nil {
		m.Dependencies = []string{}
	}
	return m, m.validate()
}

func writeData(file string, value any) error {
	data, err := yaml.Marshal(value)
	if err != nil {
		return err
	}
	return writeAtomic(file, data)
}

func writeAtomic(file string, data []byte) (err error) {
	if info, statErr := os.Lstat(file); statErr == nil && !info.Mode().IsRegular() {
		return fmt.Errorf("refusing to replace non-regular file %s", file)
	} else if statErr != nil && !errors.Is(statErr, os.ErrNotExist) {
		return statErr
	}
	tmp, err := os.CreateTemp(filepath.Dir(file), ".gnosis-write-*")
	if err != nil {
		return err
	}
	defer func() {
		if removeErr := os.Remove(tmp.Name()); removeErr != nil && !errors.Is(removeErr, os.ErrNotExist) {
			err = errors.Join(err, removeErr)
		}
	}()
	if err := tmp.Chmod(0o644); err != nil {
		return errors.Join(err, tmp.Close())
	}
	if _, err := tmp.Write(data); err != nil {
		return errors.Join(err, tmp.Close())
	}
	if err := tmp.Sync(); err != nil {
		return errors.Join(err, tmp.Close())
	}
	if err := tmp.Close(); err != nil {
		return err
	}
	return os.Rename(tmp.Name(), file)
}

func readRegistry(dir string) (registry, error) {
	r := registry{}
	if err := readData(filepath.Join(dir, "registry.yml"), &r); err != nil {
		return nil, err
	}
	if r == nil {
		return nil, errors.New("registry must be a mapping, not null")
	}
	for name, entry := range r {
		if err := validName(name); err != nil {
			return nil, err
		}
		if !ownerPattern.MatchString(entry.Owner) {
			return nil, fmt.Errorf("%s: registry owner must be @user or @organization/team", name)
		}
		if err := entry.Source.validate(); err != nil {
			return nil, fmt.Errorf("%s: %w", name, err)
		}
	}
	return r, nil
}

func readLock(dir string) (lockfile, error) {
	var lock lockfile
	if err := readData(filepath.Join(dir, "gnosis.lock"), &lock); err != nil {
		return lock, err
	}
	if lock.GnosisVersion != 1 || lock.Packages == nil {
		return lock, errors.New("invalid gnosis.lock: expected gnosis_version 1 and packages mapping")
	}
	for name, p := range lock.Packages {
		if err := validName(name); err != nil {
			return lock, err
		}
		if err := p.Source.validate(); err != nil {
			return lock, err
		}
		if !shaPattern.MatchString(p.Commit) || !shaPattern.MatchString(p.SubtreeCommit) {
			return lock, fmt.Errorf("%s: lock requires full Git commit IDs", name)
		}
		if strings.TrimSpace(p.Version) == "" {
			return lock, fmt.Errorf("%s: lock version is required", name)
		}
		seen := map[string]bool{}
		for _, dep := range p.Dependencies {
			if err := validName(dep); err != nil {
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

func sortedKeys[V any](values map[string]V) []string {
	keys := make([]string, 0, len(values))
	for key := range values {
		keys = append(keys, key)
	}
	slices.Sort(keys)
	return keys
}

func jsonOutput(w io.Writer, value any) error {
	encoder := json.NewEncoder(w)
	encoder.SetIndent("", "  ")
	return encoder.Encode(value)
}
