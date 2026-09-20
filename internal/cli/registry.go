package cli

import (
	"errors"
	"path/filepath"
	"strings"

	"gnosis/internal/knowledge"
	"gnosis/internal/workspace"

	"github.com/spf13/cobra"
)

func (a *application) registryCommand() *cobra.Command {
	parent := &cobra.Command{Use: "registry", Short: "Discover packages and maintain the local registry"}
	ref, prefix := "main", "gnosis"
	add := &cobra.Command{
		Use: "add REPOSITORY", Short: "Discover packages/registry in a public, private or local Git repository",
		Args: arguments(cobra.ExactArgs(1)),
		RunE: func(cmd *cobra.Command, args []string) error {
			repository := args[0]
			if !strings.Contains(repository, ":") {
				var err error
				repository, err = filepath.Abs(filepath.Join(a.cwd, repository))
				if filepath.IsAbs(args[0]) {
					repository = args[0]
				}
				if err != nil {
					return err
				}
			}
			s := knowledge.Source{Type: "git", Repository: repository, Path: prefix, DefaultBranch: ref}
			if err := s.Validate(); err != nil {
				return &usageError{err: err}
			}
			return a.withWorkspace(cmd.Context(), true, func(w workspace.Workspace) error {
				if err := w.NoPending(); err != nil {
					return err
				}
				r, err := workspace.DiscoverRegistry(cmd.Context(), s)
				if err != nil {
					return err
				}
				return workspace.MergeRegistry(w, r)
			})
		},
	}
	add.Flags().StringVar(&ref, "ref", "main", "Source default branch")
	add.Flags().StringVar(&prefix, "path", "gnosis", "Repository-relative bundle root or package parent")
	importCmd := &cobra.Command{
		Use: "import FILE", Short: "Import a YAML or JSON registry mapping",
		Args: arguments(cobra.ExactArgs(1)),
		RunE: func(cmd *cobra.Command, args []string) error {
			file := args[0]
			if !filepath.IsAbs(file) {
				file = filepath.Join(a.cwd, file)
			}
			r := knowledge.Registry{}
			if err := knowledge.ReadData(file, &r); err != nil {
				return err
			}
			if r == nil {
				return errors.New("registry must be a mapping")
			}
			return a.withWorkspace(cmd.Context(), true, func(w workspace.Workspace) error {
				if err := w.NoPending(); err != nil {
					return err
				}
				return workspace.MergeRegistry(w, r)
			})
		},
	}
	parent.AddCommand(add, importCmd)
	return parent
}
