package endtoend

import (
	"os"
	"os/exec"
	"path/filepath"
	"runtime"
	"strings"
	"testing"

	"github.com/docattest/docattest/adapters/go/internal/coremodule"
)

// standIn is where the test's core module claims to be fetched from. Nothing is fetched:
// the launcher's go.mod is told to read it from a directory.
const standIn = "example.com/docattest-cores"

// run runs a command in a directory and fails the test when it fails.
func run(t *testing.T, dir string, env []string, name string, args ...string) string {
	t.Helper()
	cmd := exec.Command(name, args...)
	cmd.Dir, cmd.Env = dir, append(os.Environ(), env...)
	out, err := cmd.CombinedOutput()
	if err != nil {
		t.Fatalf("%s %v: %v\n%s", name, args, err, out)
	}
	return string(out)
}

// TestALauncherBuiltWithItsCoreNeedsNothingElse builds the launcher as a release's
// `go install` builds it, with this platform's core in a module of its own, and runs it
// on a machine that has no core anywhere: it unpacks its own into the cache and runs it.
func TestALauncherBuiltWithItsCoreNeedsNothingElse(t *testing.T) {
	core := os.Getenv("DOCATTEST_CORE")
	if core == "" {
		core, _ = filepath.Abs(filepath.Join(repository, "core", "target", "release", "docattest"))
	}
	if _, err := os.Stat(core); err != nil {
		t.Skip("no docattest core to carry: set DOCATTEST_CORE, or build it (cargo build --release in core/)")
	}
	arch := map[string]string{"amd64": "x64"}[runtime.GOARCH]
	if arch == "" {
		arch = runtime.GOARCH
	}
	platform := coremodule.Platform(runtime.GOOS + "-" + arch)
	work := t.TempDir()
	source := filepath.Join(work, "go")
	if err := os.CopyFS(source, os.DirFS("..")); err != nil {
		t.Fatal(err)
	}
	modules := filepath.Join(work, "cores")
	run(t, source, nil, "go", "run", "./tools/coremodule", "-base", standIn, "-out", modules,
		"-link", filepath.Join(source, "internal", "bundled"), string(platform)+"="+core)
	module := coremodule.ModulePath(standIn, platform)
	run(t, source, nil, "go", "mod", "edit", "-require", module+"@v0.0.0",
		"-replace", module+"="+filepath.Join(modules, "core-"+string(platform)))
	bin := filepath.Join(work, "bin")
	run(t, source, nil, "go", "build", "-o", bin+string(filepath.Separator), "./cmd/docattest")

	cache := filepath.Join(work, "cache")
	clean := []string{"DOCATTEST_CORE=", "DOCATTEST_CACHE=" + cache, "DOCATTEST_TRACE=1"}
	for range 2 {
		out := run(t, work, clean, filepath.Join(bin, "docattest"), "--version")
		if !strings.Contains(out, "docattest ") || !strings.Contains(out, "[trace launcher] core "+cache) {
			t.Fatalf("got %q", out)
		}
	}
	unpacked, _ := filepath.Glob(filepath.Join(cache, "core", "*", string(platform), "docattest*"))
	if len(unpacked) != 1 {
		t.Fatalf("the cache holds %v", unpacked)
	}
}
