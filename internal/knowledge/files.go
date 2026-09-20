package knowledge

import (
	"bytes"
	"errors"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"slices"

	"go.yaml.in/yaml/v3"
)

const MaxDocumentSize = 16 << 20

func Decode(data []byte, out any) error {
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

func ReadData(file string, out any) error {
	data, err := ReadRegular(file)
	if err != nil {
		return err
	}
	if err := Decode(data, out); err != nil {
		return fmt.Errorf("%s: %w", file, err)
	}
	return nil
}

func ReadRegular(file string) ([]byte, error) {
	info, err := os.Lstat(file)
	if err != nil {
		return nil, err
	}
	if !info.Mode().IsRegular() {
		return nil, fmt.Errorf("%s: expected a regular file (symlinks are not supported)", file)
	}
	if info.Size() > MaxDocumentSize {
		return nil, fmt.Errorf("%s: metadata/document exceeds 16 MiB", file)
	}
	f, err := os.Open(file)
	if err != nil {
		return nil, err
	}
	defer f.Close()
	data, err := io.ReadAll(io.LimitReader(f, MaxDocumentSize+1))
	if err != nil {
		return nil, err
	}
	if len(data) > MaxDocumentSize {
		return nil, fmt.Errorf("%s: metadata/document exceeds 16 MiB", file)
	}
	return data, nil
}

func WriteData(file string, value any) error {
	data, err := yaml.Marshal(value)
	if err != nil {
		return err
	}
	return WriteAtomic(file, data)
}

func WriteAtomic(file string, data []byte) (err error) {
	if len(data) > MaxDocumentSize {
		return fmt.Errorf("%s: metadata/document exceeds 16 MiB", file)
	}
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

func SortedKeys[V any](values map[string]V) []string {
	keys := make([]string, 0, len(values))
	for key := range values {
		keys = append(keys, key)
	}
	slices.Sort(keys)
	return keys
}
