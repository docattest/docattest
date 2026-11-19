//go:build docattest

package sandbox

import "strings"

// ForbiddenCapNames are the names of the capabilities a sandboxed command is refused.
// Only docattest builds this file, so the pages call it while the package exports
// nothing for them.
func ForbiddenCapNames() []string {
	names := []string{}
	for _, c := range forbiddenCaps {
		names = append(names, c.Name)
	}
	return names
}

// SessionEnvNames are the names of the variables a sandboxed command's environment adds.
func SessionEnvNames() []string {
	names := []string{}
	for _, variable := range sessionEnv {
		name, _, _ := strings.Cut(variable, "=")
		names = append(names, name)
	}
	return names
}
