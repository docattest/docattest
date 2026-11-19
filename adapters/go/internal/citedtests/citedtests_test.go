package citedtests

import (
	"os"
	"path/filepath"
	"reflect"
	"strings"
	"testing"
	"time"

	"github.com/docattest/docattest/adapters/go/internal/protocol"
)

// tiny is a module whose tests pass, fail, skip, and are not there, in three packages: one
// with an external test package beside its own, one that does not build, one that hangs.
var tiny = map[string]string{
	"go.mod":       "module example.com/tiny\n\ngo 1.21\n",
	"calc/calc.go": "package calc\n\nfunc Add(a, b int) int { return a + b }\n",
	"calc/calc_test.go": `package calc

import "testing"

func TestAdds(t *testing.T) {
	if Add(1, 1) != 2 {
		t.Fatal("one plus one is not two")
	}
}

func TestSubtracts(t *testing.T) {
	if 2-1 != 0 {
		t.Error("two minus one is not zero")
	}
}

func TestMultiplies(t *testing.T) {
	t.Skip("not yet written")
}

func TestRounding(t *testing.T) {
	cases := []struct {
		name  string
		cents int
	}{{"rounds down", 1}, {"rounds up", 5}, {"rounds down again", 9}, {"a (b)|c [d] e.f*", 0}, {"a (b)|c [d] e.fff", 9}}
	for _, c := range cases {
		t.Run(c.name, func(t *testing.T) {
			if c.cents >= 5 {
				t.Errorf("%d cents is too many", c.cents)
			}
		})
	}
}
`,
	"calc/outside_test.go": `package calc_test

import "testing"

func TestFromOutside(t *testing.T) {}
`,
	"broken/broken_test.go": `package broken

import "testing"

func TestBroken(t *testing.T) { undefinedThing() }
`,
	"slow/slow_test.go": `package slow

import (
	"testing"
	"time"
)

func TestSleeps(t *testing.T) { time.Sleep(time.Minute) }
`,
}

func cite(file string, path ...string) protocol.CitedTest {
	return protocol.CitedTest{File: file, Describes: path[:len(path)-1], Title: path[len(path)-1]}
}

func TestCitedTestsRunInTheirPackages(t *testing.T) {
	root := written(t, tiny)
	tests := []protocol.CitedTest{
		cite("calc/calc_test.go", "TestAdds"),
		cite("calc/calc_test.go", "TestSubtracts"),
		cite("calc/calc_test.go", "TestMultiplies"),
		cite("calc/calc_test.go", "TestDivides"),
		cite("calc/calc_test.go", "TestRounding", "rounds down"),
		cite("calc/calc_test.go", "TestRounding", "rounds up"),
		cite("calc/calc_test.go", "TestRounding", "a (b)|c [d] e.f*"),
		cite("calc/outside_test.go", "TestFromOutside"),
		cite("broken/broken_test.go", "TestBroken"),
	}
	got := Runner{Root: root, Workers: 2}.Run(tests, time.Minute)
	statuses := []protocol.TestRunStatus{}
	for _, run := range got {
		statuses = append(statuses, run.Status)
	}
	want := []protocol.TestRunStatus{
		protocol.TestPassed, protocol.TestFailed, protocol.TestSkipped, protocol.TestNotRun, protocol.TestPassed,
		protocol.TestFailed, protocol.TestPassed, protocol.TestPassed, protocol.TestFailed,
	}
	if !reflect.DeepEqual(statuses, want) {
		t.Fatalf("got %+v", got)
	}
	messages := map[int]string{
		1: "two minus one is not zero", 2: "not yet written", 3: "ran no test TestDivides",
		5: "5 cents is too many", 8: "undefined: undefinedThing",
	}
	for k, part := range messages {
		if !strings.Contains(got[k].Message, part) {
			t.Errorf("%s: no %q in %q", tests[k].Title, part, got[k].Message)
		}
	}
}

// A run that selected anything more than the cited test would report the rows that fail:
// one whose name only begins with the cited one's, one that a pattern which did not quote
// the name would match, and the parent's own other rows.
func TestOnlyTheCitedTestRuns(t *testing.T) {
	root := written(t, tiny)
	in := invocation{dir: "calc", names: []string{"TestRounding/rounds_down", "TestRounding/a_(b)|c_[d]_e.f*"}}
	outcome := Runner{Root: root}.goTest(in, time.Minute)
	ran := readReport(outcome.Stdout)
	want := map[string]action{"TestRounding": actionPass, "TestRounding/rounds_down": actionPass, "TestRounding/a_(b)|c_[d]_e.f*": actionPass}
	if !reflect.DeepEqual(ran.ended, want) {
		t.Fatalf("got %v\n%s%s", ran.ended, outcome.Stdout, outcome.Stderr)
	}
}

func TestARunThatDoesNotEndInTimeFailsItsTests(t *testing.T) {
	root := written(t, tiny)
	began := time.Now()
	got := Runner{Root: root}.Run([]protocol.CitedTest{cite("slow/slow_test.go", "TestSleeps")}, 5*time.Second)
	if want := []protocol.TestRun{failed("go test did not finish within 5 s")}; !reflect.DeepEqual(got, want) {
		t.Fatalf("got %+v", got)
	}
	if took := time.Since(began); took > 30*time.Second {
		t.Fatalf("took %v", took)
	}
}

func TestNothingRunsInMemory(t *testing.T) {
	tests := []protocol.CitedTest{cite("a_test.go", "TestA"), cite("a_test.go", "TestA", "b")}
	for _, run := range InMemory(tests, time.Minute) {
		if run != notRun("the project is in memory, where no test can run") {
			t.Errorf("got %+v", run)
		}
	}
}

// written writes a project into a directory of the test's own.
func written(t *testing.T, files map[string]string) string {
	t.Helper()
	root := t.TempDir()
	for rel, text := range files {
		path := filepath.Join(root, filepath.FromSlash(rel))
		if err := os.MkdirAll(filepath.Dir(path), 0o700); err != nil {
			t.Fatal(err)
		}
		if err := os.WriteFile(path, []byte(text), 0o600); err != nil {
			t.Fatal(err)
		}
	}
	return root
}
