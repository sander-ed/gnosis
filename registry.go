package main

import (
	"context"
	"errors"
	"fmt"
	"os"
	"path"
	"path/filepath"
	"strings"
)

func mergeRegistry(w workspace, incoming registry) error {
	existing, err := readRegistry(w.dir)
	if err != nil {
		return err
	}
	for name, entry := range incoming {
		if err := validName(name); err != nil {
			return err
		}
		if !ownerPattern.MatchString(entry.Owner) {
			return fmt.Errorf("%s: owner must be @user or @organization/team", name)
		}
		if err := entry.Source.validate(); err != nil {
			return fmt.Errorf("%s: %w", name, err)
		}
		old, exists := existing[name]
		if exists && old.Source != entry.Source {
			return fmt.Errorf(
				"%s already resolves to a different source; edit registry.yml explicitly to change authority", name,
			)
		}
		existing[name] = entry
	}
	return writeData(filepath.Join(w.dir, "registry.yml"), existing)
}

func discoverRegistry(ctx context.Context, s source) (result registry, err error) {
	tmp, err := os.MkdirTemp("", "gnosis-discover-")
	if err != nil {
		return nil, err
	}
	defer func() { err = errors.Join(err, os.RemoveAll(tmp)) }()
	repo, err := cloneSource(ctx, s, tmp)
	if err != nil {
		return nil, err
	}
	commit, err := git(ctx, repo, "rev-parse", "HEAD")
	if err != nil {
		return nil, err
	}
	listing, err := git(ctx, repo, "ls-tree", "-r", "--name-only", commit)
	if err != nil {
		return nil, err
	}
	result = registry{}
	files := strings.Split(listing, "\n")
	registryFile := ""
	for _, file := range files {
		base := path.Base(file)
		registryName := base == "registry.yml" || base == "registry.yaml" || base == "registry.json"
		if path.Dir(file) != s.Path || !registryName {
			continue
		}
		if registryFile != "" {
			return nil, fmt.Errorf("ambiguous source registries: %s and %s", registryFile, file)
		}
		registryFile = file
		data, err := git(ctx, repo, "show", commit+":"+file)
		if err != nil {
			return nil, err
		}
		var entries registry
		if err := decode([]byte(data), &entries); err != nil {
			return nil, err
		}
		if entries == nil {
			return nil, errors.New("source registry must be a mapping, not null")
		}
		for name, entry := range entries {
			if entry.Source.Repository == "." {
				entry.Source.Repository = s.Repository
			}
			result[name] = entry
		}
	}
	manifestDirs := map[string]string{}
	for _, file := range files {
		base := path.Base(file)
		if base != "package.yml" && base != "package.yaml" && base != "package.json" {
			continue
		}
		parent := path.Dir(file)
		if parent != s.Path && path.Dir(parent) != s.Path {
			continue
		}
		if previous, exists := manifestDirs[parent]; exists {
			return nil, fmt.Errorf("ambiguous source manifests: %s and %s", previous, file)
		}
		manifestDirs[parent] = file
		data, err := git(ctx, repo, "show", commit+":"+file)
		if err != nil {
			return nil, err
		}
		var m manifest
		if err := decode([]byte(data), &m); err != nil {
			return nil, fmt.Errorf("%s: %w", file, err)
		}
		if err := m.validate(); err != nil {
			return nil, fmt.Errorf("%s: %w", file, err)
		}
		if parent != s.Path && path.Base(parent) != m.Package {
			return nil, fmt.Errorf("%s: package name must match its directory", file)
		}
		owner, err := m.Owner.handle()
		if err != nil {
			return nil, err
		}
		packageSource := s
		packageSource.Path = parent
		entry := registryEntry{Description: m.Description, Owner: owner, Source: packageSource}
		if existing, exists := result[m.Package]; exists && existing != entry {
			return nil, fmt.Errorf("conflicting definitions of %s in source registry/manifests", m.Package)
		}
		result[m.Package] = entry
	}
	if len(result) == 0 {
		return nil, fmt.Errorf("no packages discovered at %s in %s", s.Path, s.Repository)
	}
	return result, nil
}
