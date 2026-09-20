package knowledge

import (
	"bytes"
	"errors"
	"fmt"
	"io"
	"net/url"
	"path/filepath"
	"text/template"

	"github.com/santhosh-tekuri/jsonschema/v5"
)

func loadStandard(dir string) (standard, *jsonschema.Schema, error) {
	var policy standard
	if err := ReadData(filepath.Join(dir, "standard.yml"), &policy); err != nil {
		return policy, nil, err
	}
	if policy.GnosisVersion != 1 || policy.MaxWords < 0 {
		return policy, nil, errors.New("standard.yml requires gnosis_version 1 and nonnegative max_words")
	}
	schemaPath, err := SafePath(dir, policy.Schema)
	if err != nil {
		return policy, nil, err
	}
	if _, err := loadIndexTemplate(dir, policy.IndexTemplate); err != nil {
		return policy, nil, err
	}
	compiler := jsonschema.NewCompiler()
	compiler.LoadURL = func(location string) (io.ReadCloser, error) {
		u, err := url.Parse(location)
		if err != nil {
			return nil, err
		}
		if u.Scheme != "file" || u.Host != "" {
			return nil, fmt.Errorf("schema references must be package-local files: %s", location)
		}
		relative, err := filepath.Rel(dir, filepath.FromSlash(u.Path))
		if err != nil {
			return nil, err
		}
		file, err := SafePath(dir, filepath.ToSlash(relative))
		if err != nil {
			return nil, err
		}
		data, err := ReadRegular(file)
		if err != nil {
			return nil, err
		}
		return io.NopCloser(bytes.NewReader(data)), nil
	}
	schemaURL := (&url.URL{Scheme: "file", Path: filepath.ToSlash(schemaPath)}).String()
	schema, err := compiler.Compile(schemaURL)
	return policy, schema, err
}

func loadIndexTemplate(dir, relative string) (*template.Template, error) {
	file, err := SafePath(dir, relative)
	if err != nil {
		return nil, err
	}
	data, err := ReadRegular(file)
	if err != nil {
		return nil, err
	}
	tmpl, err := template.New("index").Option("missingkey=error").Parse(string(data))
	if err != nil {
		return nil, fmt.Errorf("%s: %w", file, err)
	}
	return tmpl, nil
}

type standard struct {
	GnosisVersion int      `json:"gnosis_version" yaml:"gnosis_version"`
	Schema        string   `json:"schema" yaml:"schema"`
	IndexTemplate string   `json:"index_template" yaml:"index_template"`
	RequiredFiles []string `json:"required_files" yaml:"required_files"`
	RequiredHeads []string `json:"required_headings" yaml:"required_headings"`
	MaxWords      int      `json:"max_words" yaml:"max_words"`
}
