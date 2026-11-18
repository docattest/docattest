package toolchain

import (
	"os"
	"path/filepath"
	"reflect"
	"runtime"
	"strings"
	"testing"
	"time"
)

func TestProblems(t *testing.T) {
	output := `# example.com/kit/docattest_blocks/e3
docattest_e3.go:3: declared and not used: x
./lib.go:12:4: cannot use a (variable of type int) as string value in argument to f:
	have (int)
	want (string)
too many errors
go: updates to go.mod needed
`
	want := []Problem{
		{"example.com/kit/docattest_blocks/e3", "docattest_e3.go", 3, "declared and not used: x"},
		{"example.com/kit/docattest_blocks/e3", "./lib.go", 12, "cannot use a (variable of type int) as string value in argument to f: have (int) want (string)"},
		{"example.com/kit/docattest_blocks/e3", "", 0, "go: updates to go.mod needed"},
	}
	if got := problemsIn(output); !reflect.DeepEqual(got, want) {
		t.Fatalf("got %+v", got)
	}
	if got := problemsIn(""); len(got) != 0 {
		t.Fatalf("got %+v", got)
	}
}

func TestRunKeepsOutput(t *testing.T) {
	if runtime.GOOS == "windows" {
		t.Skip("uses sh")
	}
	done := Run(Command{Path: "sh", Args: []string{"-c", "echo out; echo err >&2; exit 3"}})
	want := Outcome{Stdout: "out\n", Stderr: "err\n", Code: 3}
	if done != want {
		t.Fatalf("got %+v", done)
	}
	if missing := Run(Command{Path: "/no/such/program"}); missing.Err == nil {
		t.Fatalf("got %+v", missing)
	}
}

func TestRunEndsWhatOverruns(t *testing.T) {
	if runtime.GOOS == "windows" {
		t.Skip("uses sh")
	}
	started := time.Now()
	// The shell starts a child of its own; both are in the group the timeout kills.
	late := Run(Command{Path: "sh", Args: []string{"-c", "sleep 30 & wait"}, Timeout: 200 * time.Millisecond})
	if !late.TimedOut || late.Code != -1 || time.Since(started) > 5*time.Second {
		t.Fatalf("got %+v after %v", late, time.Since(started))
	}
}

// started is the start line of the programs of the tests below.
var started = &Start{Line: "started\n", Limit: 5 * time.Second}

func TestRunCountsTheTimeoutFromTheStartLine(t *testing.T) {
	if runtime.GOOS == "windows" {
		t.Skip("uses sh")
	}
	// A start-up longer than the timeout is not counted; the line is not kept.
	slowStart := Run(Command{Path: "sh", Args: []string{"-c", "sleep 0.5; echo started >&2; echo after >&2"}, Timeout: 200 * time.Millisecond, Start: started})
	if want := (Outcome{Stderr: "after\n"}); slowStart != want {
		t.Errorf("a slow start: got %+v", slowStart)
	}
	late := Run(Command{Path: "sh", Args: []string{"-c", "echo started >&2; sleep 30 & wait"}, Timeout: 200 * time.Millisecond, Start: started})
	if !late.TimedOut || late.Code != -1 {
		t.Errorf("a slow run: got %+v", late)
	}
	never := Run(Command{Path: "sh", Args: []string{"-c", "echo before >&2; sleep 30 & wait"}, Timeout: time.Minute, Start: &Start{Line: "started\n", Limit: 200 * time.Millisecond}})
	if !never.NeverStarted || never.TimedOut || never.Code != -1 || never.Stderr != "before\n" {
		t.Errorf("no start: got %+v", never)
	}
}

func TestTheStartLineIsFoundInPieces(t *testing.T) {
	var kept capped
	watch := newStartWatch("started\n", &kept)
	for _, piece := range []string{"one\nsta", "r", "ted\ntwo", "\n"} {
		watch.Write([]byte(piece))
	}
	watch.flush()
	select {
	case <-watch.started:
	default:
		t.Error("the line was not seen")
	}
	if kept.kept.String() != "one\ntwo\n" {
		t.Errorf("got %q", kept.kept.String())
	}
	unfinished := newStartWatch("started\n", &kept)
	unfinished.Write([]byte("sta"))
	unfinished.flush()
	if kept.kept.String() != "one\ntwo\nsta" {
		t.Errorf("got %q", kept.kept.String())
	}
}

func TestGoCommandCanBeChosen(t *testing.T) {
	t.Setenv("DOCATTEST_GO", "/opt/go/bin/go")
	if GoCommand() != "/opt/go/bin/go" {
		t.Fatal(GoCommand())
	}
	os.Unsetenv("DOCATTEST_GO")
	if GoCommand() != "go" {
		t.Fatal(GoCommand())
	}
}

func TestTheChosenGoIsFirstOnThePath(t *testing.T) {
	t.Setenv("PATH", "/usr/bin")
	t.Setenv("DOCATTEST_GO", filepath.FromSlash("/opt/go/bin/go"))
	if err := PutChosenGoFirst(); err != nil || os.Getenv("PATH") != filepath.FromSlash("/opt/go/bin")+string(os.PathListSeparator)+"/usr/bin" {
		t.Fatalf("got %q, %v", os.Getenv("PATH"), err)
	}
	t.Setenv("DOCATTEST_GO", filepath.FromSlash("/opt/go/bin/go1.21.0"))
	if err := PutChosenGoFirst(); err == nil || !strings.Contains(err.Error(), "not a file called go") {
		t.Fatalf("got %v", err)
	}
}
