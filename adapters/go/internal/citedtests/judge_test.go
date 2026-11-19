package citedtests

import (
	"errors"
	"reflect"
	"testing"
	"time"

	"github.com/docattest/docattest/adapters/go/internal/protocol"
	"github.com/docattest/docattest/adapters/go/internal/toolchain"
)

// ranOne is what go 1.24 reported of `go test -json -count=1 -run '^TestAdds$|…'` on a
// package with a passing, a failing and a skipped test and a table-driven one, timestamps
// left out.
const ranOne = `{"Action":"start","Package":"example.com/calc"}
{"Action":"run","Package":"example.com/calc","Test":"TestAdds"}
{"Action":"output","Package":"example.com/calc","Test":"TestAdds","Output":"=== RUN   TestAdds\n"}
{"Action":"output","Package":"example.com/calc","Test":"TestAdds","Output":"--- PASS: TestAdds (0.00s)\n"}
{"Action":"pass","Package":"example.com/calc","Test":"TestAdds","Elapsed":0}
{"Action":"run","Package":"example.com/calc","Test":"TestSubtracts"}
{"Action":"output","Package":"example.com/calc","Test":"TestSubtracts","Output":"=== RUN   TestSubtracts\n"}
{"Action":"output","Package":"example.com/calc","Test":"TestSubtracts","Output":"    calc_test.go:14: two minus one is not zero\n"}
{"Action":"output","Package":"example.com/calc","Test":"TestSubtracts","Output":"--- FAIL: TestSubtracts (0.00s)\n"}
{"Action":"fail","Package":"example.com/calc","Test":"TestSubtracts","Elapsed":0}
{"Action":"run","Package":"example.com/calc","Test":"TestMultiplies"}
{"Action":"output","Package":"example.com/calc","Test":"TestMultiplies","Output":"=== RUN   TestMultiplies\n"}
{"Action":"output","Package":"example.com/calc","Test":"TestMultiplies","Output":"    calc_test.go:19: not yet written\n"}
{"Action":"output","Package":"example.com/calc","Test":"TestMultiplies","Output":"--- SKIP: TestMultiplies (0.00s)\n"}
{"Action":"skip","Package":"example.com/calc","Test":"TestMultiplies","Elapsed":0}
{"Action":"run","Package":"example.com/calc","Test":"TestRounding"}
{"Action":"output","Package":"example.com/calc","Test":"TestRounding","Output":"=== RUN   TestRounding\n"}
{"Action":"run","Package":"example.com/calc","Test":"TestRounding/rounds_up"}
{"Action":"output","Package":"example.com/calc","Test":"TestRounding/rounds_up","Output":"=== RUN   TestRounding/rounds_up\n"}
{"Action":"output","Package":"example.com/calc","Test":"TestRounding/rounds_up","Output":"    calc_test.go:30: 5 cents is too many\n"}
{"Action":"output","Package":"example.com/calc","Test":"TestRounding/rounds_up","Output":"--- FAIL: TestRounding/rounds_up (0.00s)\n"}
{"Action":"fail","Package":"example.com/calc","Test":"TestRounding/rounds_up","Elapsed":0}
{"Action":"output","Package":"example.com/calc","Test":"TestRounding","Output":"--- FAIL: TestRounding (0.00s)\n"}
{"Action":"fail","Package":"example.com/calc","Test":"TestRounding","Elapsed":0}
{"Action":"output","Package":"example.com/calc","Output":"FAIL\n"}
{"Action":"output","Package":"example.com/calc","Output":"FAIL\texample.com/calc\t0.244s\n"}
{"Action":"fail","Package":"example.com/calc","Elapsed":0.245}
`

func calc(names ...string) invocation {
	return invocation{dir: "calc", names: names}
}

func TestEachTestIsJudgedByItsOwnEvents(t *testing.T) {
	in := calc("TestAdds", "TestSubtracts", "TestMultiplies", "TestRounding/rounds_up", "TestRounding", "TestDivides")
	got := judged(in, toolchain.Outcome{Stdout: ranOne, Code: 1}, time.Minute)
	want := []protocol.TestRun{
		{Status: protocol.TestPassed},
		failed("calc_test.go:14: two minus one is not zero"),
		skipped("calc_test.go:19: not yet written"),
		failed("calc_test.go:30: 5 cents is too many"),
		// A test whose subtest failed fails with what the subtest wrote.
		failed("calc_test.go:30: 5 cents is too many"),
		notRun("go test -run '^TestDivides$' in calc ran no test TestDivides"),
	}
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("got  %+v\nwant %+v", got, want)
	}
}

// noneSelected is what go 1.21 reported when -run selected nothing.
const noneSelected = `{"Action":"start","Package":"example.com/calc"}
{"Action":"output","Package":"example.com/calc","Output":"testing: warning: no tests to run\n"}
{"Action":"output","Package":"example.com/calc","Output":"PASS\n"}
{"Action":"output","Package":"example.com/calc","Output":"ok  \texample.com/calc\t0.264s [no tests to run]\n"}
{"Action":"pass","Package":"example.com/calc","Elapsed":0.264}
`

func TestATestWithNoEventWasNotRun(t *testing.T) {
	got := judged(calc("TestRounding/rounds_over"), toolchain.Outcome{Stdout: noneSelected}, time.Minute)
	want := []protocol.TestRun{notRun("go test -run '^TestRounding$/^rounds_over$' in calc ran no test TestRounding/rounds_over")}
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("got %+v", got)
	}
}

// Before go 1.24 the errors of a package that does not build go to the standard error, and
// the standard output says only that the build failed; from 1.24 they are events.
const (
	buildFailedBefore124 = "FAIL\texample.com/calc [build failed]\n"
	buildFailedStderr    = "# example.com/calc [example.com/calc.test]\n./calc_test.go:5:28: undefined: undefinedThing\n"
	buildFailed124       = `{"ImportPath":"example.com/calc [example.com/calc.test]","Action":"build-output","Output":"# example.com/calc [example.com/calc.test]\n"}
{"ImportPath":"example.com/calc [example.com/calc.test]","Action":"build-output","Output":"./calc_test.go:5:28: undefined: undefinedThing\n"}
{"ImportPath":"example.com/calc [example.com/calc.test]","Action":"build-fail"}
{"Action":"start","Package":"example.com/calc"}
{"Action":"output","Package":"example.com/calc","Output":"FAIL\texample.com/calc [build failed]\n"}
{"Action":"fail","Package":"example.com/calc","Elapsed":0,"FailedBuild":"example.com/calc [example.com/calc.test]"}
`
)

func TestEveryTestOfAPackageThatDoesNotBuildFailsWithTheBuildErrors(t *testing.T) {
	want := []protocol.TestRun{
		failed("./calc_test.go:5:28: undefined: undefinedThing"),
		failed("./calc_test.go:5:28: undefined: undefinedThing"),
	}
	outcomes := map[string]toolchain.Outcome{
		"before 1.24": {Stdout: buildFailedBefore124, Stderr: buildFailedStderr, Code: 1},
		"1.24":        {Stdout: buildFailed124, Code: 1},
	}
	for version, outcome := range outcomes {
		if got := judged(calc("TestAdds", "TestSubtracts"), outcome, time.Minute); !reflect.DeepEqual(got, want) {
			t.Errorf("%s: got %+v", version, got)
		}
	}
}

func TestARunThatEndsBeforeItsTestsFailsOrDoesNotRunThemAll(t *testing.T) {
	in := calc("TestAdds", "TestSubtracts")
	timedOut := judged(in, toolchain.Outcome{Stdout: ranOne, Code: -1, TimedOut: true}, 90*time.Second)
	for _, run := range timedOut {
		if run != failed("go test did not finish within 90 s") {
			t.Errorf("timed out: got %+v", run)
		}
	}
	noGo := judged(in, toolchain.Outcome{Code: -1, Err: errors.New(`exec: "go": executable file not found in $PATH`)}, time.Minute)
	for _, run := range noGo {
		if run != notRun(`the go command could not be run: exec: "go": executable file not found in $PATH`) {
			t.Errorf("no go: got %+v", run)
		}
	}
}

func TestATestThatSaysNothingStillHasAMessage(t *testing.T) {
	quiet := `{"Action":"run","Test":"TestQuiet"}
{"Action":"output","Test":"TestQuiet","Output":"=== RUN   TestQuiet\n"}
{"Action":"output","Test":"TestQuiet","Output":"--- FAIL: TestQuiet (0.00s)\n"}
{"Action":"fail","Test":"TestQuiet"}
{"Action":"run","Test":"TestHushed"}
{"Action":"skip","Test":"TestHushed"}
{"Action":"fail"}
`
	got := judged(calc("TestQuiet", "TestHushed"), toolchain.Outcome{Stdout: quiet, Code: 1}, time.Minute)
	want := []protocol.TestRun{
		failed("go test reports that TestQuiet failed, and nothing it wrote"),
		skipped("go test skipped TestHushed"),
	}
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("got %+v", got)
	}
}
