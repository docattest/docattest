package analysis

import (
	"go/constant"
	"go/types"
	"os"
	"os/exec"
	"path/filepath"
	"runtime"
	"strings"
	"testing"

	"golang.org/x/tools/go/packages"
)

// withoutExportData puts first on the PATH a go command that refuses to write export
// data, as one whose format this adapter cannot read is of no use, and runs everything
// else as the go command on the PATH does.
func withoutExportData(t *testing.T) {
	t.Helper()
	if runtime.GOOS == "windows" {
		t.Skip("the stand-in go command is a shell script")
	}
	real, err := exec.LookPath("go")
	if err != nil {
		t.Fatal(err)
	}
	dir := t.TempDir()
	script := "#!/bin/sh\nfor arg in \"$@\"; do\n\tcase \"$arg\" in -export | -export=true)\n\t\techo 'export data of an unknown format' >&2\n\t\texit 1\n\tesac\ndone\nexec " + real + " \"$@\"\n"
	if err := os.WriteFile(filepath.Join(dir, "go"), []byte(script), 0o700); err != nil {
		t.Fatal(err)
	}
	t.Setenv("PATH", dir+string(os.PathListSeparator)+os.Getenv("PATH"))
}

func TestPackagesAreCheckedFromSourceWithoutExportData(t *testing.T) {
	withoutExportData(t)
	// net/http imports vendored packages of the standard library; limits is the module's.
	p := readWritten(t, map[string]string{
		"go.mod":           "module example.com/m\n\ngo 1.21\n",
		"limits/limits.go": "package limits\n\nconst Most = 3\n",
		"main.go":          "package main\n\nimport (\n\t\"net/http\"\n\n\t\"example.com/m/limits\"\n)\n\nconst status = http.StatusTeapot\n\nconst twice = limits.Most * 2\n\nfunc main() {}\n",
	})
	pkg, err := p.Checked(p.SurfaceNamed("default"))
	if err != nil {
		t.Fatal(err)
	}
	for name, want := range map[string]int64{"status": 418, "twice": 6} {
		if got, _ := constant.Int64Val(pkg.Scope().Lookup(name).(*types.Const).Val()); got != want {
			t.Errorf("%s: got %d, want %d", name, got, want)
		}
	}
}

func TestConstantsAreSizedForWhatTheGoCommandBuildsFor(t *testing.T) {
	t.Setenv("GOOS", "linux")
	t.Setenv("GOARCH", "arm")
	p := readWritten(t, map[string]string{
		"go.mod":  "module example.com/m\n\ngo 1.21\n",
		"main.go": "package main\n\nimport \"unsafe\"\n\nconst wordBits = 8 * unsafe.Sizeof(uintptr(0))\n\nfunc main() {}\n",
	})
	pkg, err := p.Checked(p.SurfaceNamed("default"))
	if err != nil {
		t.Fatal(err)
	}
	if got, _ := constant.Int64Val(pkg.Scope().Lookup("wordBits").(*types.Const).Val()); got != 32 {
		t.Errorf("got %d bits, want 32", got)
	}
}

func TestAnErrorOfThePackageIsWhyItIsNotChecked(t *testing.T) {
	p := readWritten(t, map[string]string{
		"go.mod":  "module example.com/m\n\ngo 1.21\n",
		"main.go": "package main\n\nimport \"example.com/m/missing\"\n\nconst port = 8080\n\nvar _ = missing.Name\n\nfunc main() {}\n",
	})
	if _, err := p.Checked(p.SurfaceNamed("default")); err == nil || !strings.Contains(err.Error(), "example.com/m/missing") {
		t.Fatalf("got %v", err)
	}
}

func TestAGoNewerThanTheAdapterIsNoErrorOfThePackage(t *testing.T) {
	// What go/types says when a module on Go 1.26 is checked by an adapter built with Go
	// 1.23.
	newer := []string{
		"package requires newer Go version go1.26 (application built with go1.23)",
		"file requires newer Go version go1.26 (application built with go1.23)",
	}
	for _, message := range newer {
		if !aboutNewerGo(packages.Error{Msg: message}) {
			t.Errorf("an error of the package: %s", message)
		}
	}
	if aboutNewerGo(packages.Error{Msg: "undefined: undefinedName"}) {
		t.Error("an error in the code is about the version of Go")
	}
}
