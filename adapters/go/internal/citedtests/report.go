package citedtests

import (
	"encoding/json"
	"fmt"
	"regexp"
	"strings"
)

// report is what one `go test -json` invocation said, as test2json writes it.
type report struct {
	// ended is how each test that ended ended, by its reported name.
	ended map[string]action
	// order are the tests that ended, in the order they did.
	order []string
	// output is what each test wrote, by its reported name.
	output map[string]string
	// packageOutput is what was written outside any test: the package's own lines, and the
	// build's, which go 1.24 and later report as build-output events.
	packageOutput string
	// packageFailed says the package's run failed as a whole.
	packageFailed bool
}

// event is a line test2json writes.
type event struct {
	Action action
	Test   string
	Output string
}

// action is what an event says happened.
type action string

// The actions of test2json that a report reads.
const (
	actionOutput      action = "output"
	actionBuildOutput action = "build-output"
	actionPass        action = "pass"
	actionFail        action = "fail"
	actionSkip        action = "skip"
)

// readReport reads what `go test -json` wrote to its standard output. A line that is not
// an event is the package's own: before go 1.24, a package that does not build is
// reported by a plain `FAIL` line.
func readReport(stdout string) report {
	read := report{ended: map[string]action{}, output: map[string]string{}}
	for _, line := range strings.SplitAfter(stdout, "\n") {
		var e event
		if err := json.Unmarshal([]byte(line), &e); err != nil || e.Action == "" {
			read.packageOutput += line
			continue
		}
		read.add(e)
	}
	return read
}

// add takes in an event: of the package as a whole when it names no test.
func (r *report) add(e event) {
	switch {
	case e.Test == "" && (e.Action == actionOutput || e.Action == actionBuildOutput):
		r.packageOutput += e.Output
	case e.Test == "":
		r.packageFailed = r.packageFailed || e.Action == actionFail
	case e.Action == actionOutput:
		r.output[e.Test] += e.Output
	case e.Action == actionPass || e.Action == actionFail || e.Action == actionSkip:
		r.ended[e.Test] = e.Action
		r.order = append(r.order, e.Test)
	}
}

// anyTestFailed says some test of the run failed: what makes a package fail, unless
// something before any test (its build, its vet, its TestMain) did.
func (r report) anyTestFailed() bool {
	for _, ended := range r.ended {
		if ended == actionFail {
			return true
		}
	}
	return false
}

// explanation is what a test wrote, and what the subtests of it that failed wrote: the
// lines that say why it failed, or why it was skipped.
func (r report) explanation(name string) string {
	lines := ownLines(r.output[name])
	for _, test := range r.order {
		if r.ended[test] == actionFail && strings.HasPrefix(test, name+"/") {
			lines = append(lines, ownLines(r.output[test])...)
		}
	}
	return strings.Join(lines, "\n")
}

// framing are the lines the testing package writes around a test's own: where it starts,
// pauses and goes on, and how it ended.
var framing = regexp.MustCompile(`^(=== (RUN|PAUSE|CONT|NAME)\b|--- (PASS|FAIL|SKIP): )`)

// ownLines are the lines a test wrote itself, trimmed.
func ownLines(output string) []string {
	return keptLines(output, framing)
}

// packageFraming are the lines the go command writes around a package's output: its
// verdict, the heading of a build's errors, and the test program's exit status.
var packageFraming = regexp.MustCompile(`^(PASS|FAIL|ok)(\s|$)|^# |^exit status \d+$`)

// packageFailure is why a package's run failed as a whole: the build's errors, or what
// its test program wrote outside any test, with what the go command wrote to its standard
// error.
func (r report) packageFailure(stderr string) string {
	return strings.Join(keptLines(r.packageOutput+stderr, packageFraming), "\n")
}

// keptLines are the lines of a text that are not blank and not framing, trimmed.
func keptLines(text string, framing *regexp.Regexp) []string {
	kept := []string{}
	for _, line := range strings.Split(text, "\n") {
		trimmed := strings.TrimSpace(line)
		if trimmed != "" && !framing.MatchString(trimmed) {
			kept = append(kept, trimmed)
		}
	}
	return kept
}

// orSaying is a message, or what to say when it is empty: a message is never empty.
func orSaying(message, format string, args ...any) string {
	if message != "" {
		return message
	}
	return fmt.Sprintf(format, args...)
}
