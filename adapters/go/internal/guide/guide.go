// Package guide is the setup of this package's README and CONTRIBUTING, which docattest
// checks with this adapter (../../docattest.config.json): the runners of the README's cases
// tables, and what a fact of either reads by name.
package guide

import (
	"fmt"
	"os"
	"path/filepath"
	"regexp"
	"strings"

	"github.com/docattest/docattest/adapters/go/internal/language"
	"github.com/docattest/docattest/adapters/go/internal/launcher"
	"github.com/docattest/docattest/adapters/go/internal/platform"
	"github.com/docattest/docattest/adapters/go/internal/protocoldata"
)

// Cases are the runners of the README's cases tables, by name.
var Cases = map[string]func(row map[string]string) error{
	"cache": cache,
	"weak":  weak,
}

// Columns are the columns each runner's table must have.
var Columns = map[string][]string{
	"cache": {"system", "environment", "cache"},
	"weak":  {"block", "weak"},
}

// Packages are the packages of the module's internal/ directory, the map the contributing
// guide draws. A fact runs in the project root, where internal/ is.
func Packages() []string {
	entries, err := os.ReadDir("internal")
	if err != nil {
		panic(err)
	}
	names := []string{}
	for _, entry := range entries {
		if entry.IsDir() {
			names = append(names, entry.Name())
		}
	}
	return names
}

// home stands for the user's home directory, written `~` in the table.
const home = "/home/u"

// systems are the systems a row of the cache table names, as Go names them.
var systems = map[string][]string{
	"macOS":   {"darwin"},
	"Linux":   {"linux"},
	"Windows": {"windows"},
	"any":     {"darwin", "linux", "windows"},
}

// lookedIn finds where the launcher says it looked for a cached core, when it finds none.
var lookedIn = regexp.MustCompile(`the shared cache \(([^)]*)\)`)

// cache holds a row of the cache table: with that environment, on that system, a launcher
// that finds no core says it looked in `<cache>/core/<version>/<platform>/docattest`. What
// it says is what a user sees, so the row is held to the message, not to the internals.
func cache(row map[string]string) error {
	goNames, ok := systems[row["system"]]
	if !ok {
		return fmt.Errorf("no system %q", row["system"])
	}
	env := environment(row["environment"])
	pin := protocoldata.Core()
	for _, goos := range goNames {
		machine := launcher.Machine{
			Env:        func(name string) string { return env[name] },
			OS:         goos,
			Arch:       "amd64",
			Home:       home,
			Executable: filepath.Join(home, "no-such-directory", "docattest"),
		}
		_, err := launcher.FindCore(machine, pin)
		found := lookedIn.FindStringSubmatch(fmt.Sprint(err))
		if found == nil {
			return fmt.Errorf("on %s the launcher does not say where it looked: %v", goos, err)
		}
		want := filepath.Join(strings.Replace(row["cache"], "~", home, 1), "core", pin.Version,
			platform.Name(goos, "amd64"), platform.Executable(goos, "docattest"))
		if found[1] != want {
			return fmt.Errorf("on %s: expected %s, got %s", goos, want, found[1])
		}
	}
	return nil
}

// environment reads a cell of `NAME=value` settings, separated by spaces.
func environment(cell string) map[string]string {
	env := map[string]string{}
	for _, setting := range strings.Fields(cell) {
		name, value, _ := strings.Cut(setting, "=")
		env[name] = value
	}
	return env
}

// answers are the words a table writes for yes and no.
var answers = map[string]bool{"yes": true, "no": false}

// weak holds a row of the table of calls that end a program: a block of that one call
// states no output, so it is weak unless the call ends the program.
func weak(row map[string]string) error {
	want, ok := answers[row["weak"]]
	if !ok {
		return fmt.Errorf("weak is yes or no, not %q", row["weak"])
	}
	block := strings.Trim(row["block"], "`")
	if got := !language.Asserts(block); got != want {
		return fmt.Errorf("expected weak %v, got %v", want, got)
	}
	return nil
}
