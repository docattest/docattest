package toolchain

import (
	"os"
	"reflect"
	"runtime"
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
