package running

import (
	"testing"

	"github.com/docattest/docattest/adapters/go/internal/protocol"
)

func TestAFactCallsTheExportedDocsHelpersOfItsSurface(t *testing.T) {
	r := open(t, opening{surfaces: entries("sandbox", "./sandbox")})
	got := r.Evaluate([]protocol.Fact{
		factOn("sandbox", "helper", "ForbiddenCapNames()"),
		factOn("sandbox", "withExport", "len(SessionEnvNames()) + len(Env(nil))"),
		factOn("sandbox", "unexported", "capability{}"),
	})
	expect(t, texts(got), map[string]string{
		"helper": "[CAP_SYS_ADMIN CAP_NET_RAW]", "withExport": "4",
		"unexported": "fact `capability{}`: capability is a type that surface \"sandbox\" does not export; of its unexported names, a fact reads only constants and literal variables",
	})
}

func TestAnUnexportedDocsHelperIsRefusedWithWhatToDo(t *testing.T) {
	files := map[string]string{
		"go.mod":         "module example.com/tool\n\ngo 1.21\n",
		"guard/guard.go": "package guard\n\nvar forbidden = []string{\"CAP_SYS_ADMIN\"}\n\nfunc Allowed(name string) bool { return name != forbidden[0] }\n",
		"guard/docs.go":  "//go:build docattest\n\npackage guard\n\nfunc first() string { return forbidden[0] }\n",
	}
	r := open(t, opening{files: files, surfaces: entries("default", "./guard"), noSetup: true})
	got := r.Evaluate([]protocol.Fact{fact("unexported", "first()")})
	expect(t, texts(got), map[string]string{
		"unexported": "fact `first()`: first is a function of a docs helper file of surface \"default\" that is not exported; export it there, since only docattest builds that file",
	})
}

func TestABlockIsBuiltWithoutTheDocsHelpers(t *testing.T) {
	r := open(t, opening{surfaces: entries("sandbox", "./sandbox")})
	block := onPage("helper", "import (\n\t\"fmt\"\n\n\t\"example.com/shop/sandbox\"\n)\n\nfmt.Println(sandbox.ForbiddenCapNames())\n")
	diagnostics := r.Compile([]protocol.Example{block})
	if len(diagnostics) != 1 || diagnostics[0].Message != "undefined: sandbox.ForbiddenCapNames" {
		t.Fatalf("got %+v", diagnostics)
	}
}
