package running

import (
	"reflect"
	"testing"
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
	want := map[string][]string{"guard": {
		`go vet -tags docattest: guard/guard_docattest.go:7: fmt.Sprintf format %d has arg forbidden[0] of wrong type string`,
	}}
	if got := r.VetDocsHelpers(); !reflect.DeepEqual(got, want) {
		t.Fatalf("got %q", got)
	}
}

func TestCleanDocsHelpersSayNothing(t *testing.T) {
	r := open(t, opening{surfaces: entries("sandbox", "./sandbox")})
	if got := r.VetDocsHelpers(); len(got) != 0 {
		t.Fatalf("got %q", got)
	}
}
