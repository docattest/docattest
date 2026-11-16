// Package launcher is the `docattest` command of the Go package: it finds the docattest
// core, tells it how to start the Go adapter and what command runs docattest here, and
// becomes it (PROTOCOL.md, section 1, "Launchers").
package launcher

import (
	"encoding/json"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"sort"
	"strings"

	"github.com/docattest/docattest/adapters/go/internal/platform"
	"github.com/docattest/docattest/adapters/go/internal/protocoldata"
)

// The variables of the hand-over.
const (
	CoreVariable    = "DOCATTEST_CORE"
	AdapterVariable = "DOCATTEST_ADAPTER_GO"
	CommandVariable = "DOCATTEST_COMMAND"
	// TraceVariable is the core's own trace, which the launcher answers too.
	TraceVariable = "DOCATTEST_TRACE"
)

// Traced reports whether a trace is asked for: the variable is set to anything but nothing,
// as the core reads it.
func Traced(env func(name string) string) bool {
	return env(TraceVariable) != ""
}

// Machine is what finding the core reads of the machine it runs on. The tests describe
// other machines.
type Machine struct {
	// Env reads a variable of the environment.
	Env func(name string) string
	// OS and Arch are the platform, as Go names it (GOOS, GOARCH).
	OS, Arch string
	Home     string
	// Executable is the path of the launcher itself.
	Executable string
	// Ask runs `core --version --json` and returns what it prints; "" when it does not.
	Ask func(core string) string
	// Carried is the core the launcher carries inside itself; nil when it carries none.
	Carried []byte
}

// platformName is the machine's platform in the names every package uses: darwin-arm64,
// linux-x64.
func (m Machine) platformName() string {
	return platform.Name(m.OS, m.Arch)
}

// executable is the file name of a program on the machine.
func (m Machine) executable(base string) string {
	return platform.Executable(m.OS, base)
}

// bundled is where a release of this package carries the core: beside the launcher, as
// docattest-core.
func (m Machine) bundled() string {
	return filepath.Join(filepath.Dir(m.Executable), m.executable("docattest-core"))
}

// cacheDirectory is docattest's own cache directory: DOCATTEST_CACHE, else `docattest`
// in the user's cache directory.
func (m Machine) cacheDirectory() string {
	if given := m.Env("DOCATTEST_CACHE"); given != "" {
		return given
	}
	switch m.OS {
	case "windows":
		if local := m.Env("LOCALAPPDATA"); local != "" {
			return filepath.Join(local, "docattest")
		}
		return filepath.Join(m.Home, "AppData", "Local", "docattest")
	case "darwin":
		return filepath.Join(m.Home, "Library", "Caches", "docattest")
	}
	if xdg := m.Env("XDG_CACHE_HOME"); filepath.IsAbs(xdg) {
		return filepath.Join(xdg, "docattest")
	}
	return filepath.Join(m.Home, ".cache", "docattest")
}

// cached is where a core of the pinned version lies in the cache every language
// package of one user shares.
func (m Machine) cached(pin protocoldata.Pin) string {
	return filepath.Join(m.cacheDirectory(), "core", pin.Version, m.platformName(), m.executable("docattest"))
}

// FindCore finds the core, in order of preference: DOCATTEST_CORE; the binary a release
// carries beside the launcher; the shared cache; and the core the launcher carries inside
// itself, which is written to the shared cache and run from there. The cache and what the
// package brought are trusted without being run: a cache path names the exact version.
// A core someone points at is asked which protocol it speaks, and refused when it is not
// this package's.
func FindCore(m Machine, pin protocoldata.Pin) (string, error) {
	if given := m.Env(CoreVariable); given != "" {
		return checked(m, pin, given)
	}
	bundled, cached := m.bundled(), m.cached(pin)
	for _, candidate := range []string{bundled, cached} {
		if isFile(candidate) {
			return candidate, nil
		}
	}
	if len(m.Carried) > 0 {
		return cached, unpack(m.Carried, cached)
	}
	return "", fmt.Errorf("docattest: no docattest core %s for %s. Looked in: %s (not set); beside this command (%s); the shared cache (%s)\n%s",
		pin.Version, m.platformName(), CoreVariable, bundled, cached, remedy(m, pin))
}

// unpack writes the core the launcher carries to its place in the cache. It is written
// beside its place and renamed into it, so that a launcher started at the same moment
// finds either no core or a whole one.
func unpack(core []byte, into string) error {
	fail := func(err error) error {
		return fmt.Errorf("docattest: cannot write the core to the cache (%s): %w", into, err)
	}
	if err := os.MkdirAll(filepath.Dir(into), 0o755); err != nil {
		return fail(err)
	}
	partial, err := os.CreateTemp(filepath.Dir(into), "docattest-*.partial")
	if err != nil {
		return fail(err)
	}
	defer os.Remove(partial.Name())
	_, err = partial.Write(core)
	if closed := partial.Close(); err == nil {
		err = closed
	}
	if err == nil {
		err = os.Chmod(partial.Name(), 0o755)
	}
	if err == nil {
		err = os.Rename(partial.Name(), into)
	}
	if err != nil {
		return fail(err)
	}
	return nil
}

func isFile(path string) bool {
	info, err := os.Stat(path)
	return err == nil && info.Mode().IsRegular()
}

// checked asks a core someone named what it is.
func checked(m Machine, pin protocoldata.Pin, core string) (string, error) {
	var told struct {
		Version  *string  `json:"version"`
		Protocol *float64 `json:"protocol"`
	}
	if json.Unmarshal([]byte(m.Ask(core)), &told) != nil || told.Version == nil || told.Protocol == nil {
		return "", fmt.Errorf("docattest: %s (%s) is not a docattest core: it does not answer --version --json", CoreVariable, core)
	}
	if *told.Protocol != float64(pin.Protocol) {
		return "", fmt.Errorf("docattest: %s (%s) is core %s, which speaks adapter protocol %v; this package speaks protocol %d (it was built for core %s)",
			CoreVariable, core, *told.Version, *told.Protocol, pin.Protocol, pin.Version)
	}
	return core, nil
}

// remedy is what a user with no core is told: why there is none, and how to get one.
func remedy(m Machine, pin protocoldata.Pin) string {
	build := "build one from docattest's source (cargo build --release in its core/ directory) and set " + CoreVariable + " to it"
	known := protocoldata.Platforms()
	if _, ok := known[m.platformName()]; !ok {
		listed := make([]string, 0, len(known))
		for name := range known {
			listed = append(listed, name)
		}
		sort.Strings(listed)
		return fmt.Sprintf("docattest brings a core for %s; there is none for %s. To get one, %s.",
			strings.Join(listed, ", "), m.platformName(), build)
	}
	return fmt.Sprintf("This command was built without a core inside it (from a checkout, not from a release). "+
		"A docattest release for %s carries core %s as %s beside this command; or %s.",
		m.platformName(), pin.Version, m.executable("docattest-core"), build)
}

// AskCore runs `core --version --json`.
func AskCore(core string) string {
	out, err := exec.Command(core, "--version", "--json").Output()
	if err != nil {
		return ""
	}
	return string(out)
}
