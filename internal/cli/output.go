package cli

import (
	"encoding/json"
	"io"
)

func jsonOutput(w io.Writer, value any) error {
	encoder := json.NewEncoder(w)
	encoder.SetIndent("", "  ")
	return encoder.Encode(value)
}
