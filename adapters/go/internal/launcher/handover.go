package launcher

import (
	"encoding/json"
	"os"
	"os/exec"
	"path/filepath"
	"regexp"
	"strings"
)

// writers are the commands that write the docattest command down; only for them is it
// worked out.
var writers = map[string]bool{"hooks": true, "agents": true}

var plain = regexp.MustCompile(`^[A-Za-z0-9_@%+=:,./-]+$`)

// adapterCommand is how to start this package's adapter: the program installed beside
// the launcher, else the one on the PATH. It is nil when there is neither, and then the
// core says where it looked.
func adapterCommand(m Machine) []string {
	beside := filepath.Join(filepath.Dir(m.Executable), m.executable("docattest-adapter-go"))
	if isFile(beside) {
		return []string{beside}
	}
	if found, err := exec.LookPath("docattest-adapter-go"); err == nil {
		return []string{found}
	}
	return nil
}

// docattestCommand is a shell command that runs docattest: `docattest` when that is
// what a shell finds on the PATH, else this command by its full path.
func docattestCommand(m Machine) string {
	if found, err := exec.LookPath("docattest"); err == nil && sameFile(found, m.Executable) {
		return "docattest"
	}
	return quote(m.Executable)
}

func sameFile(a, b string) bool {
	first, errA := os.Stat(a)
	second, errB := os.Stat(b)
	return errA == nil && errB == nil && os.SameFile(first, second)
}

// quote writes a word as a POSIX shell reads it back.
func quote(word string) string {
	if plain.MatchString(word) {
		return word
	}
	return "'" + strings.ReplaceAll(word, "'", `'\''`) + "'"
}

// Environment is the core's environment: this process's, with the hand-over added
// where it is not already set, so that a developer's own adapter and a wrapper's own
// command stand.
func Environment(m Machine, args, environ []string) []string {
	out := append([]string{}, environ...)
	if adapter := adapterCommand(m); m.Env(AdapterVariable) == "" && adapter != nil {
		text, _ := json.Marshal(adapter)
		out = append(out, AdapterVariable+"="+string(text))
	}
	if m.Env(CommandVariable) == "" && writes(args) {
		out = append(out, CommandVariable+"="+docattestCommand(m))
	}
	return out
}

func writes(args []string) bool {
	for _, arg := range args {
		if writers[arg] {
			return true
		}
	}
	return false
}
