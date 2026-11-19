// Package sandbox runs a command with a fixed environment and without some Linux
// capabilities. Its pages read what it does not export from source, and through the docs
// helpers of sandbox_docattest.go: it exports nothing for them.
package sandbox

// capability is a Linux capability a sandboxed command is refused, and why.
type capability struct {
	Name   string
	Reason string
}

// forbiddenCaps are the capabilities a sandboxed command is refused.
var forbiddenCaps = []capability{
	{Name: "CAP_SYS_ADMIN", Reason: "mounts filesystems"},
	{Name: "CAP_NET_RAW", Reason: "forges packets"},
}

// sessionEnv is what a sandboxed command's environment adds to its parent's.
var sessionEnv = []string{"LANG=C.UTF-8", "TZ=UTC"}

// Refuses says whether a sandboxed command is refused a capability.
func Refuses(name string) bool {
	for _, c := range forbiddenCaps {
		if c.Name == name {
			return true
		}
	}
	return false
}

// Env is the environment of a sandboxed command: its parent's, then the session's.
func Env(parent []string) []string {
	return append(parent, sessionEnv...)
}
