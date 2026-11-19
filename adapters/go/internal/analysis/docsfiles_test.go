package analysis

import (
	"go/parser"
	"go/token"
	"strings"
	"testing"

	"github.com/docattest/docattest/adapters/go/internal/protocol"
)

func TestADocsHelperFileHoldsOnlyWithTheDocsTag(t *testing.T) {
	cases := map[string]bool{
		"//go:build docattest":              true,
		"//go:build docattest && linux":     true,
		"//go:build (docattest || a) && !a": true,
		"//go:build docattest || linux":     false,
		"//go:build !docattest":             false,
		"//go:build linux":                  false,
		"//go:build !windows":               false,
		"// docattest":                      false,
	}
	for line, want := range cases {
		file, err := parser.ParseFile(token.NewFileSet(), "f.go", line+"\n\npackage p\n", parser.ParseComments)
		if err != nil {
			t.Fatal(err)
		}
		if got := isDocsFile(file); got != want {
			t.Errorf("%s: got %v", line, got)
		}
	}
}

func TestADocsHelperIsNoExportAndAttestsWhatItUses(t *testing.T) {
	p := readFixture(t, entries("sandbox", "./sandbox")...)
	of := p.SurfaceNamed("sandbox")
	if exports, _ := p.ExportsOf(of); strings.Join(exports, " ") != "Shell ShellEnv MaxOpenFiles Refuses Env" {
		t.Errorf("exports: got %v", exports)
	}
	if importable := p.Importable(of); !importable["ForbiddenCapNames"] || !importable["Refuses"] {
		t.Errorf("importable: got %v", importable)
	}
	cases := map[string]string{
		// A docs helper is attested itself, and what it uses: forbiddenCaps and sessionEnv
		// are no exports, and are attested as a fact that reads them attests them.
		"ForbiddenCapNames()": "sandbox:ForbiddenCapNames sandbox:forbiddenCaps",
		"len(forbiddenCaps) + len(SessionEnvNames()) + len(Env(nil))": "sandbox:forbiddenCaps sandbox:SessionEnvNames sandbox:sessionEnv sandbox:Env",
	}
	for expression, want := range cases {
		found, err := symbolsOf(p, protocol.Attestation{Kind: protocol.KindFact, Surface: "sandbox", Expression: expression})
		if got := names(found); err != nil || got != want {
			t.Errorf("%s: got %q, %v, want %q", expression, got, err, want)
		}
	}
}

func TestADocsHelperCreditsWhatItReads(t *testing.T) {
	root := written(t, map[string]string{
		"go.mod": "module example.com/tool\n\ngo 1.21\n",
		"guard/guard.go": `package guard

type Capability struct{ Name string }

var forbidden = []Capability{{Name: "SYS_ADMIN"}}

const Limit = 3

const Prefix = "CAP_"

func Allowed(name string) bool { return len(name) < Limit }
`,
		// A docs helper's own code is followed into the helpers of its docs files, and not
		// into the functions of the surface's ordinary files.
		"guard/docs.go": `//go:build docattest

package guard

func Names() (out []string) {
	for _, c := range forbidden {
		out = append(out, name(c))
	}
	return out
}

func name(c Capability) string { return Prefix + c.Name }

func AllowedNames() (out []string) {
	for _, n := range Names() {
		if Allowed(n) {
			out = append(out, n)
		}
	}
	return out
}
`,
	})
	p := read(t, Config{Root: root, InMemory: true, Surfaces: entries("guard", "./guard")})
	cases := map[string]string{
		"Names()":        "guard:Names guard:forbidden guard:Capability guard:Prefix",
		"AllowedNames()": "guard:AllowedNames guard:Allowed guard:forbidden guard:Capability guard:Prefix",
	}
	for expression, want := range cases {
		found, err := symbolsOf(p, protocol.Attestation{Kind: protocol.KindFact, Surface: "guard", Expression: expression})
		if got := names(found); err != nil || got != want {
			t.Errorf("%s: got %q, %v, want %q", expression, got, err, want)
		}
	}
}
