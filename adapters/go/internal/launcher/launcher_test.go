package launcher

import (
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/docattest/docattest/adapters/go/internal/protocoldata"
)

var pin = protocoldata.Pin{Version: "1.2.3", Protocol: 1}

// machine is a Linux machine with a home and a launcher in a directory of its own.
func machine(t *testing.T, env map[string]string) Machine {
	t.Helper()
	dir := t.TempDir()
	return Machine{
		Env:        func(name string) string { return env[name] },
		OS:         "linux",
		Arch:       "amd64",
		Home:       filepath.Join(dir, "home"),
		Executable: filepath.Join(dir, "bin", "docattest"),
		Ask:        func(string) string { return `{"version":"1.2.3","protocol":1}` },
	}
}

func touch(t *testing.T, path string) string {
	t.Helper()
	if err := os.MkdirAll(filepath.Dir(path), 0o700); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(path, nil, 0o700); err != nil {
		t.Fatal(err)
	}
	return path
}

func TestPlacesTheCoreIsLookedFor(t *testing.T) {
	m := machine(t, nil)
	if got := m.cached(pin); got != filepath.Join(m.Home, ".cache", "docattest", "core", "1.2.3", "linux-x64", "docattest") {
		t.Fatal(got)
	}
	mac := Machine{Env: m.Env, OS: "darwin", Arch: "arm64", Home: "/Users/u"}
	if got := mac.cached(pin); got != "/Users/u/Library/Caches/docattest/core/1.2.3/darwin-arm64/docattest" {
		t.Fatal(got)
	}
	xdg := machine(t, map[string]string{"XDG_CACHE_HOME": "/x", "DOCATTEST_CACHE": ""})
	if got := xdg.cacheDirectory(); got != "/x/docattest" {
		t.Fatal(got)
	}
	windows := Machine{Env: m.Env, OS: "windows", Arch: "amd64", Home: "h", Executable: filepath.Join("b", "docattest.exe")}
	if !strings.HasSuffix(windows.cached(pin), "docattest.exe") || !strings.HasSuffix(windows.bundled(), "docattest-core.exe") {
		t.Fatal(windows.cached(pin), windows.bundled())
	}
}

func TestFindCoreInOrder(t *testing.T) {
	m := machine(t, nil)
	_, err := FindCore(m, pin)
	for _, want := range []string{"no docattest core 1.2.3 for linux-x64", "DOCATTEST_CORE (not set)", m.bundled(), m.cached(pin), "built without a core inside it"} {
		if err == nil || !strings.Contains(err.Error(), want) {
			t.Fatalf("no %q in: %v", want, err)
		}
	}
	cached := touch(t, m.cached(pin))
	if got, err := FindCore(m, pin); got != cached || err != nil {
		t.Fatal(got, err)
	}
	bundled := touch(t, m.bundled())
	if got, err := FindCore(m, pin); got != bundled || err != nil {
		t.Fatal(got, err)
	}
	m.Env = func(name string) string { return map[string]string{CoreVariable: "/a/core"}[name] }
	if got, err := FindCore(m, pin); got != "/a/core" || err != nil {
		t.Fatal(got, err)
	}
}

func TestACoreSomeonePointsAtIsAsked(t *testing.T) {
	m := machine(t, map[string]string{CoreVariable: "/a/core"})
	m.Ask = func(string) string { return `{"version":"9.0.0","protocol":2}` }
	if _, err := FindCore(m, pin); err == nil || !strings.Contains(err.Error(), "speaks adapter protocol 2; this package speaks protocol 1") {
		t.Fatal(err)
	}
	m.Ask = func(string) string { return "hello" }
	if _, err := FindCore(m, pin); err == nil || !strings.Contains(err.Error(), "is not a docattest core") {
		t.Fatal(err)
	}
}

func TestAPlatformWithoutACoreIsToldSo(t *testing.T) {
	m := machine(t, nil)
	m.OS, m.Arch = "plan9", "amd64"
	if _, err := FindCore(m, pin); err == nil || !strings.Contains(err.Error(), "there is none for plan9-x64") {
		t.Fatal(err)
	}
}

func TestEnvironmentHandsOverWhatIsNotSet(t *testing.T) {
	m := machine(t, nil)
	adapter := touch(t, filepath.Join(filepath.Dir(m.Executable), "docattest-adapter-go"))
	env := Environment(m, []string{"check"}, []string{"A=1"})
	if len(env) != 2 || env[0] != "A=1" || env[1] != AdapterVariable+`=["`+adapter+`"]` {
		t.Fatal(env)
	}
	env = Environment(m, []string{"init", "--hooks"}, nil)
	if len(env) != 2 || env[1] != CommandVariable+"="+m.Executable {
		t.Fatal(env)
	}
	set := machine(t, map[string]string{AdapterVariable: "mine", CommandVariable: "mine"})
	if env := Environment(set, []string{"init", "--hooks"}, []string{"A=1"}); len(env) != 1 {
		t.Fatal(env)
	}
}

func TestATraceIsAskedForByAnythingButNothing(t *testing.T) {
	env := func(value string) func(string) string {
		return func(name string) string { return map[string]string{TraceVariable: value}[name] }
	}
	if !Traced(env("1")) || !Traced(env("0")) || Traced(env("")) {
		t.Fatal("a trace is asked for by any value but the empty one")
	}
}

func TestQuote(t *testing.T) {
	for word, want := range map[string]string{"/usr/bin/docattest": "/usr/bin/docattest", "/my tools/it's": `'/my tools/it'\''s'`} {
		if got := quote(word); got != want {
			t.Errorf("quote(%q) = %s", word, got)
		}
	}
}

func TestACarriedCoreIsWrittenToTheCacheOnce(t *testing.T) {
	m := machine(t, nil)
	m.Carried = []byte("the core")
	got, err := FindCore(m, pin)
	if err != nil || got != m.cached(pin) {
		t.Fatal(got, err)
	}
	info, err := os.Stat(got)
	if text, _ := os.ReadFile(got); err != nil || string(text) != "the core" || info.Mode().Perm()&0o100 == 0 {
		t.Fatalf("got %q, mode %v, %v", text, info.Mode(), err)
	}
	left, _ := filepath.Glob(filepath.Join(filepath.Dir(got), "*.partial"))
	if len(left) != 0 {
		t.Fatalf("left behind: %v", left)
	}
	// What is in the cache already is used as it is, and not written again.
	m.Carried = []byte("another")
	if _, err := FindCore(m, pin); err != nil {
		t.Fatal(err)
	}
	if text, _ := os.ReadFile(got); string(text) != "the core" {
		t.Fatalf("the cached core was overwritten: %q", text)
	}
}
