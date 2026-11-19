// Package citedtests runs the tests that citations name, for `check --run-cited`: the
// tests of each package in one `go test -json` invocation, selected by a -run pattern that
// matches them and no other, and judged by the events test2json reports for each.
package citedtests

import (
	"path"
	"path/filepath"
	"sync"
	"time"

	"github.com/docattest/docattest/adapters/go/internal/protocol"
	"github.com/docattest/docattest/adapters/go/internal/toolchain"
)

// Runner runs the cited tests of a project on disk.
type Runner struct {
	// Root is the project's directory, which a cited test's file is relative to.
	Root string
	// Workspace is the go.work every go command of the project runs with.
	Workspace toolchain.Workspace
	// Workers bounds how many go test invocations run at once.
	Workers int
}

// Run answers `runTests`: how each test ran, in order. `timeout` bounds each invocation.
func (r Runner) Run(tests []protocol.CitedTest, timeout time.Duration) []protocol.TestRun {
	out := make([]protocol.TestRun, len(tests))
	slots := make(chan struct{}, max(r.Workers, 1))
	var all sync.WaitGroup
	for _, in := range byPackage(tests) {
		all.Add(1)
		go func() {
			defer all.Done()
			slots <- struct{}{}
			outcome := r.goTest(in, timeout)
			<-slots
			for k, run := range judged(in, outcome, timeout) {
				out[in.tests[k]] = run
			}
		}()
	}
	all.Wait()
	return out
}

// goTest runs `go test` on an invocation's package, in its directory, as a user runs it
// there: the test program runs in that directory too, and the go command finds the
// package's module and the user's go.work, or uses the project's workspace.
func (r Runner) goTest(in invocation, timeout time.Duration) toolchain.Outcome {
	return toolchain.Run(toolchain.Command{
		Path:    toolchain.GoCommand(),
		Args:    []string{"test", "-json", "-count=1", "-run", in.pattern(), "."},
		Dir:     filepath.Join(r.Root, filepath.FromSlash(in.dir)),
		Env:     r.Workspace.Environment(),
		Timeout: timeout,
	})
}

// InMemory answers `runTests` for a project that is only what `open` gave: nothing there
// can run.
func InMemory(tests []protocol.CitedTest, _ time.Duration) []protocol.TestRun {
	out := make([]protocol.TestRun, len(tests))
	for k := range tests {
		out[k] = notRun("the project is in memory, where no test can run")
	}
	return out
}

// byPackage groups tests by the package of their file, in the order the packages first
// come. A test file's package is its directory: the go command builds the package's own
// test files and its external test package (`p_test`) into one test program.
func byPackage(tests []protocol.CitedTest) []invocation {
	out := []invocation{}
	at := map[string]int{}
	for k, test := range tests {
		dir := path.Dir(test.File)
		if _, seen := at[dir]; !seen {
			at[dir] = len(out)
			out = append(out, invocation{dir: dir})
		}
		in := &out[at[dir]]
		in.names = append(in.names, reportedName(test))
		in.tests = append(in.tests, k)
	}
	return out
}
