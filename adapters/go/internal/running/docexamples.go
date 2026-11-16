package running

import (
	"fmt"
	"path/filepath"
	"regexp"
	"strings"
	"sync"

	"github.com/docattest/docattest/adapters/go/internal/analysis"
	"github.com/docattest/docattest/adapters/go/internal/protocol"
	"github.com/docattest/docattest/adapters/go/internal/toolchain"
)

// testProgram is the test program of one package, which holds its examples: built once.
type testProgram struct {
	once     sync.Once
	binary   string
	problems []toolchain.Problem
}

// testPrograms are the test programs of a session, by package directory.
type testPrograms struct {
	lock  sync.Mutex
	byDir map[string]*testProgram
}

// docExample is a testable example of a source file, with the test program it is in.
type docExample struct {
	*analysis.SourceExample
	program *testProgram
}

var gotWant = regexp.MustCompile(`(?s)\ngot:\n(.*?)\nwant:\n(.*?)\nFAIL`)

// testProgramOf builds, once, the test program of the package in a directory. A file
// placed among its tests refuses the network.
func (r *Runner) testProgramOf(dir, pkg string) *testProgram {
	r.tests.lock.Lock()
	if r.tests.byDir == nil {
		r.tests.byDir = map[string]*testProgram{}
	}
	program := r.tests.byDir[dir]
	if program == nil {
		program = &testProgram{}
		r.tests.byDir[dir] = program
	}
	r.tests.lock.Unlock()
	program.once.Do(func() {
		binary, err := r.buildTestProgram(dir, pkg)
		if err != nil {
			program.problems = []toolchain.Problem{{Message: err.Error()}}
			return
		}
		if program.problems = r.gotool.TestBinary(binary, r.project.InModule(dir)); len(program.problems) == 0 {
			program.binary = binary
		}
	})
	return program
}

// buildTestProgram places the network guard among a package's tests and returns the
// file its test program is to be built into.
func (r *Runner) buildTestProgram(dir, pkg string) (binary string, err error) {
	name := fmt.Sprintf("t%d", r.serial.take())
	generated, err := r.work("generated", name)
	if err != nil {
		return "", err
	}
	guard := filepath.Join(generated, "guard.go")
	if err := writeFile(guard, guardSource(pkg)); err != nil {
		return "", fmt.Errorf("cannot write the network guard: %w", err)
	}
	r.gotool.Place(filepath.Join(r.project.InModule(dir), "docattest_guard_test.go"), guard)
	bin, err := r.work("bin")
	if err != nil {
		return "", err
	}
	return filepath.Join(bin, toolchain.Executable(name+".test")), nil
}

// docExampleOf finds an example attestation's function again; nil when it is gone.
func (r *Runner) docExampleOf(example protocol.Example) (*docExample, error) {
	found, err := r.project.SourceExampleAt(example.Page, example.Line)
	if found == nil {
		return nil, err
	}
	return &docExample{SourceExample: found, program: r.testProgramOf(found.Dir, found.Package)}, nil
}

// docExampleProblems are why an example of a source file does not compile.
func (r *Runner) docExampleProblems(example protocol.Example) []problem {
	_, problems := r.compiledDocExample(example)
	return problems
}

// compiledDocExample finds an example of a source file again and builds its package's
// test program; the problems are why it cannot be run: it is gone, or the compiler's
// complaints inside it, else that its package's tests do not compile at all.
func (r *Runner) compiledDocExample(example protocol.Example) (*docExample, []problem) {
	found, err := r.docExampleOf(example)
	if err != nil {
		return nil, []problem{{1, err.Error()}}
	}
	if found == nil {
		return nil, []problem{{1, fmt.Sprintf("no example function is declared at %s:%d", example.Page, example.Line)}}
	}
	inside := []problem{}
	for _, p := range found.program.problems {
		if filepath.Base(p.File) == found.File && p.Line >= found.First && p.Line <= found.Last {
			inside = append(inside, problem{p.Line - found.First + 1, p.Message})
		}
	}
	if len(inside) == 0 && len(found.program.problems) > 0 {
		first := found.program.problems[0]
		where := ""
		if first.File != "" {
			where = fmt.Sprintf("%s:%d: ", first.File, first.Line)
		}
		inside = []problem{{1, "the tests of its package do not compile: " + where + first.Message}}
	}
	return found, inside
}

// runDocExample runs one testable example as `go test` runs it: by its name, in its
// package's test program, in its package's directory.
func (r *Runner) runDocExample(example protocol.Example) verdict {
	found, problems := r.compiledDocExample(example)
	if len(problems) > 0 {
		return fail(fmt.Sprintf("does not compile: %s (block line %d)", problems[0].Message, problems[0].Line))
	}
	release := r.running()
	outcome := toolchain.Run(toolchain.Command{
		Path:    found.program.binary,
		Args:    []string{"-test.run", "^" + found.Name + "$", "-test.v"},
		Dir:     found.Dir,
		Timeout: r.timeout(),
	})
	release()
	if sides := gotWant.FindStringSubmatch(outcome.Stdout); outcome.Code != 0 && sides != nil {
		return differs(strings.TrimSpace(sides[2]), strings.TrimSpace(sides[1]))
	}
	if ended := r.ended(outcome, found.File, found.First); ended != nil {
		return *ended
	}
	if !strings.Contains(outcome.Stdout, "--- PASS: "+found.Name) {
		return pass("compiles; it states no output, so `go test` does not run it")
	}
	return pass("printed the output it states")
}
