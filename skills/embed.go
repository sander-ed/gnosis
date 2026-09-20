// Package skills embeds the agent skills distributed with gnosis.
package skills

import "embed"

// Files contains the canonical skill definitions installed by the CLI.
//
//go:embed */SKILL.md
var Files embed.FS
