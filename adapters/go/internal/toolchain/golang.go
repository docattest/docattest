package toolchain

import (
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"regexp"
	"runtime"
	"strings"
	"sync"

	"github.com/docattest/docattest/adapters/go/internal/platform"
)

// Go is the go command at work in one module. Generated files are added to the module
// through a build overlay: they are compiled as if they were in the module's tree, and
// nothing is written into it.
type Go struct {
	// Module is the module's directory, where the command runs.
	Module string
	// Work is a directory of the adapter's own, for overlay files.
	Work string
	// Workspace is the go.work the command runs with; empty for none.
	Workspace Workspace

	lock    sync.Mutex
	overlay map[string]string
	serial  int
	env     map[string]string
}

// Problem is one complaint of the go command.
type Problem struct {
	// Package is the import path under whose heading the complaint came, if any.
	Package string
	// File is the file as the command named it; empty for a complaint about no file.
	File    string
	Line    int
	Message string
}

var (
	heading  = regexp.MustCompile(`^# (\S+)`)
	position = regexp.MustCompile(`^(.+?):(\d+)(?::\d+)?: (.*)$`)
)

// GoCommand is the go command to run: DOCATTEST_GO, else `go` on the PATH.
func GoCommand() string {
	if chosen := os.Getenv("DOCATTEST_GO"); chosen != "" {
		return chosen
	}
	return "go"
}

// PutChosenGoFirst puts the directory of DOCATTEST_GO first on this process's PATH. The
// package loader runs the `go` it finds on the PATH, and has to run the go command
// everything else runs; so DOCATTEST_GO names a file called go.
func PutChosenGoFirst() error {
	chosen := os.Getenv("DOCATTEST_GO")
	if chosen == "" {
		return nil
	}
	if name := strings.TrimSuffix(filepath.Base(chosen), ".exe"); name != "go" {
		return fmt.Errorf("DOCATTEST_GO names %s, not a file called go: the package loader runs the first go on the PATH", chosen)
	}
	if filepath.Base(chosen) == chosen {
		return nil
	}
	return os.Setenv("PATH", filepath.Dir(chosen)+string(os.PathListSeparator)+os.Getenv("PATH"))
}

// Executable is the file name of a program built on this platform.
func Executable(name string) string {
	return platform.Executable(runtime.GOOS, name)
}

// Place puts a file of the adapter's at a path inside the module, for every later build.
func (g *Go) Place(inModule, real string) {
	g.lock.Lock()
	defer g.lock.Unlock()
	if g.overlay == nil {
		g.overlay = map[string]string{}
	}
	g.overlay[filepath.Join(g.Module, inModule)] = real
}

// overlayFile writes the overlay as it stands and returns the file's path.
func (g *Go) overlayFile() (string, error) {
	g.lock.Lock()
	defer g.lock.Unlock()
	g.serial++
	path := filepath.Join(g.Work, fmt.Sprintf("overlay-%d.json", g.serial))
	text, err := json.Marshal(map[string]any{"Replace": g.overlay})
	if err != nil {
		return "", err
	}
	return path, os.WriteFile(path, text, 0o600)
}

// run runs `go <verb>` in the module with the overlay, and reads what it complains of.
// The problems are empty exactly when the command succeeded.
func (g *Go) run(verb string, args ...string) []Problem {
	_, problems := g.outputIn(g.Module, verb, args...)
	return problems
}

// output runs `go <verb>` as run does, and is also what it wrote to its standard output.
// Every command runs with the same flags, so that what one builds the next finds cached;
// DocsTag changes what is compiled only of a package with docs helper files.
func (g *Go) output(verb string, args ...string) (string, []Problem) {
	return g.outputIn(g.Module, verb, args...)
}

// outputIn runs `go <verb>` in `dir` with the overlay and workspace environment.
func (g *Go) outputIn(dir, verb string, args ...string) (string, []Problem) {
	overlay, err := g.overlayFile()
	if err != nil {
		return "", []Problem{{Message: err.Error()}}
	}
	full := append([]string{verb, "-trimpath", "-overlay", overlay}, args...)
	outcome := Run(Command{Path: GoCommand(), Args: full, Dir: dir, Env: g.Workspace.Environment()})
	if outcome.Err != nil {
		return "", []Problem{{Message: fmt.Sprintf("the go command could not be run: %v", outcome.Err)}}
	}
	if outcome.Code == 0 {
		return outcome.Stdout, nil
	}
	problems := problemsIn(outcome.Stderr + outcome.Stdout)
	if len(problems) == 0 {
		problems = []Problem{{Message: fmt.Sprintf("go %s failed with status %d", verb, outcome.Code)}}
	}
	return "", problems
}

// DocsTag is the build tag of docs helper files: the files of a package that only
// docattest's own programs build (generated programs, and the analysis), so that they
// may read the package's unexported names for the pages and nothing else links them.
const DocsTag = "docattest"

// Build builds packages of the module, given by their directories relative to it, as the
// go command builds them by default, and writes the program of each main package into
// `out`.
func (g *Go) Build(out string, packages []string) []Problem {
	return g.run("build", buildArgs(out, packages)...)
}

// BuildWithDocs builds packages as Build does, with the docs helper files of every
// package they import (DocsTag).
func (g *Go) BuildWithDocs(out string, packages []string) []Problem {
	return g.run("build", append([]string{"-tags=" + DocsTag}, buildArgs(out, packages)...)...)
}

// VetWithDocs runs go vet on packages of the module, given by their directories relative
// to it, with their docs helper files (DocsTag); the problems are its findings.
func (g *Go) VetWithDocs(packages []string) []Problem {
	return g.VetWithDocsIn(g.Module, packages)
}

// VetWithDocsIn runs go vet in `dir` on `packages` (paths relative to `dir`, such as
// `./...`), with docs helper files (DocsTag).
func (g *Go) VetWithDocsIn(dir string, packages []string) []Problem {
	args := []string{"-tags=" + DocsTag}
	for _, pkg := range packages {
		args = append(args, "./"+filepath.ToSlash(pkg))
	}
	_, problems := g.outputIn(dir, "vet", args...)
	return problems
}

// buildArgs are the arguments of `go build` that put the programs of packages in `out`.
func buildArgs(out string, packages []string) []string {
	args := []string{"-o", out + string(filepath.Separator)}
	for _, pkg := range packages {
		args = append(args, "./"+filepath.ToSlash(pkg))
	}
	return args
}

// TestBinary builds the test program of one package of the module into the file `out`.
func (g *Go) TestBinary(out, pkg string) []Problem {
	return g.run("test", "-vet=off", "-c", "-o", out, "./"+filepath.ToSlash(pkg))
}

// Env is one variable of `go env`, asked once.
func (g *Go) Env(name string) string {
	g.lock.Lock()
	defer g.lock.Unlock()
	if g.env == nil {
		g.env = map[string]string{}
	}
	if _, asked := g.env[name]; !asked {
		outcome := Run(Command{
			Path: GoCommand(), Args: []string{"env", name}, Dir: g.Module, Env: g.Workspace.Environment(),
		})
		g.env[name] = strings.TrimSpace(outcome.Stdout)
	}
	return g.env[name]
}

// problemsIn reads the output of a failed go command: a line `file:line: message` is a
// problem at that line, an indented line goes on from the one before, and any other
// line is a problem about no file. A `# package` line heads the problems of a package.
func problemsIn(output string) []Problem {
	problems := []Problem{}
	pkg := ""
	for _, line := range strings.Split(output, "\n") {
		text := strings.TrimRight(line, " \r")
		switch found := position.FindStringSubmatch(text); {
		case text == "" || strings.HasPrefix(text, "too many errors"):
		case heading.MatchString(text):
			pkg = heading.FindStringSubmatch(text)[1]
		case strings.HasPrefix(text, "\t") && len(problems) > 0:
			problems[len(problems)-1].Message += " " + strings.TrimSpace(text)
		case found != nil:
			var n int
			_, _ = fmt.Sscanf(found[2], "%d", &n)
			problems = append(problems, Problem{pkg, found[1], n, found[3]})
		default:
			problems = append(problems, Problem{Package: pkg, Message: strings.TrimSpace(text)})
		}
	}
	return problems
}
