package running

import (
	"testing"

	"github.com/docattest/docattest/adapters/go/internal/protocol"
	"github.com/docattest/docattest/adapters/go/internal/toolchain"
)

// startingProject is a module whose package `starting` takes 1500 ms to initialise, longer
// than startingTimeoutMs: a program that imports it starts slowly.
var startingProject = map[string]string{
	"go.mod":               "module example.com/m\n\ngo 1.21\n",
	"m.go":                 "package m\n",
	"starting/starting.go": "package starting\n\nimport \"time\"\n\nfunc init() { time.Sleep(1500 * time.Millisecond) }\n\nfunc Ready() bool { return true }\n",
}

const startingTimeoutMs = 1000

func TestABlocksTimeCountsFromWhenItsOwnCodeStarts(t *testing.T) {
	r := open(t, opening{files: startingProject, surfaces: entries("default", "."), noSetup: true, timeoutMs: startingTimeoutMs})
	examples := []protocol.Example{
		onPage("slow start", "import \"example.com/m/starting\"\n\nif !starting.Ready() {\n\tpanic(\"not ready\")\n}"),
		onPage("slow run", "import \"time\"\n\ntime.Sleep(1500 * time.Millisecond)"),
		onPage("slow variable", "import \"time\"\n\nvar slept = sleep()\n\nfunc sleep() bool {\n\ttime.Sleep(1500 * time.Millisecond)\n\treturn true\n}\n\nfunc main() { println(slept) }"),
		onPage("exits", "import \"os\"\n\nos.Exit(3)"),
	}
	want := []string{
		"pass: ran to its end without panicking",
		"fail: timed out after 1000 ms",
		"fail: timed out after 1000 ms",
		"fail: exited with status 3",
	}
	for k, result := range r.Run(examples, nil) {
		if got := string(result.Status) + ": " + result.Message; got != want[k] {
			t.Errorf("%s: got %q, want %q", examples[k].ID, got, want[k])
		}
	}
}

func TestABlockThatNeverStartsSaysSo(t *testing.T) {
	r := open(t, opening{files: startingProject, surfaces: entries("default", "."), noSetup: true})
	found := r.ended(toolchain.Outcome{Code: -1, NeverStarted: true}, "", 1)
	if found == nil || found.Message != "did not start within 10000 ms" {
		t.Fatalf("got %+v", found)
	}
}
