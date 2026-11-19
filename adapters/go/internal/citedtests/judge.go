package citedtests

import (
	"fmt"
	"time"

	"github.com/docattest/docattest/adapters/go/internal/protocol"
	"github.com/docattest/docattest/adapters/go/internal/toolchain"
)

// invocation is one `go test` run: of the package in a directory, for the tests named.
type invocation struct {
	// dir is the package's directory, relative to the project root with `/` separators.
	dir string
	// names are the tests' reported names; tests are their places in the request.
	names []string
	tests []int
}

// pattern is the -run pattern of the invocation's tests.
func (in invocation) pattern() string {
	return runPattern(in.names)
}

// judged is how each test of an invocation ran, in the order of its names, from how the
// go command ended and the events it reported. A run that did not end in time fails every
// test of it; a test with no event of its own failed when the package failed before any
// test could (it does not build, its TestMain exits), and else was not run.
func judged(in invocation, outcome toolchain.Outcome, timeout time.Duration) []protocol.TestRun {
	out := make([]protocol.TestRun, len(in.names))
	read := readReport(outcome.Stdout)
	packageBroke := !read.anyTestFailed() && (read.packageFailed || outcome.Code != 0)
	for k, name := range in.names {
		switch {
		case outcome.Err != nil:
			out[k] = notRun(fmt.Sprintf("the go command could not be run: %v", outcome.Err))
		case outcome.TimedOut:
			out[k] = failed(fmt.Sprintf("go test did not finish within %g s", timeout.Seconds()))
		case read.ended[name] == actionPass:
			out[k] = protocol.TestRun{Status: protocol.TestPassed}
		case read.ended[name] == actionFail:
			out[k] = failed(orSaying(read.explanation(name), "go test reports that %s failed, and nothing it wrote", name))
		case read.ended[name] == actionSkip:
			out[k] = skipped(orSaying(read.explanation(name), "go test skipped %s", name))
		case packageBroke:
			out[k] = failed(orSaying(read.packageFailure(outcome.Stderr), "go test failed with status %d", outcome.Code))
		default:
			out[k] = notRun(fmt.Sprintf("go test -run '%s' in %s ran no test %s", runPattern([]string{name}), in.dir, name))
		}
	}
	return out
}

func failed(message string) protocol.TestRun {
	return protocol.TestRun{Status: protocol.TestFailed, Message: message}
}

func skipped(message string) protocol.TestRun {
	return protocol.TestRun{Status: protocol.TestSkipped, Message: message}
}

func notRun(message string) protocol.TestRun {
	return protocol.TestRun{Status: protocol.TestNotRun, Message: message}
}
