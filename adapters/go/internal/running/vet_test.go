package running

import (
	"reflect"
	"testing"

	"github.com/docattest/docattest/adapters/go/internal/protocol"
)

func TestVetReportsWhatItFindsInDocsHelperFiles(t *testing.T) {
	r := open(t, opening{surfaces: entries("guard", "./guard"), noSetup: true, files: map[string]string{
		"go.mod": "module example.com/tool\n\ngo 1.21\n",
		// The project's own vet judges Name, in a file every build compiles.
		"guard/guard.go": "package guard\n\nimport \"fmt\"\n\nvar forbidden = []string{\"CAP_SYS_ADMIN\"}\n\n" +
			"func Name() string { return fmt.Sprintf(\"%d\", \"x\") }\n",
		"guard/guard_docattest.go": "//go:build docattest\n\npackage guard\n\nimport \"fmt\"\n\n" +
			"func First() string { return fmt.Sprintf(\"%d\", forbidden[0]) }\n",
	}})
	want := []protocol.Problem{{
		Path: "guard/guard_docattest.go", Line: 7,
		Message: "go vet -tags docattest: fmt.Sprintf format %d has arg forbidden[0] of wrong type string",
	}}
	if got := r.VetDocsHelpers(); !reflect.DeepEqual(got, want) {
		t.Fatalf("got %+v", got)
	}
}

func TestAFindingInAPackageTwoSurfacesShareIsReportedOnce(t *testing.T) {
	r := open(t, opening{surfaces: entries("guard", "./guard", "again", "./guard"), noSetup: true, files: map[string]string{
		"go.mod":         "module example.com/tool\n\ngo 1.21\n",
		"guard/guard.go": "package guard\n\nvar forbidden = []string{\"CAP_SYS_ADMIN\"}\n",
		"guard/guard_docattest.go": "//go:build docattest\n\npackage guard\n\nimport \"fmt\"\n\n" +
			"func First() string { return fmt.Sprintf(\"%d\", forbidden[0]) }\n",
	}})
	if got := r.VetDocsHelpers(); len(got) != 1 {
		t.Fatalf("got %+v", got)
	}
}

func TestCleanDocsHelpersSayNothing(t *testing.T) {
	r := open(t, opening{surfaces: entries("sandbox", "./sandbox")})
	if got := r.VetDocsHelpers(); len(got) != 0 {
		t.Fatalf("got %+v", got)
	}
}
