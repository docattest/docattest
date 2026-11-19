package analysis

import (
	"encoding/json"
	"os"
	"path/filepath"
	"reflect"
	"strings"
	"testing"

	"github.com/docattest/docattest/adapters/go/internal/protocol"
)

// fixture is the sample project the tests read: adapters/go/testdata/fixture.
func fixture(t *testing.T) string {
	t.Helper()
	root, err := filepath.Abs("../../testdata/fixture")
	if err != nil {
		t.Fatal(err)
	}
	return root
}

// readFixture reads the fixture with its surfaces, or with the surfaces given.
func readFixture(t *testing.T, surfaces ...protocol.ConfiguredSurface) *Project {
	t.Helper()
	if len(surfaces) == 0 {
		surfaces = entries("default", ".", "shipping", "./shipping", "gone", "./no/such/package")
	}
	return read(t, Config{Root: fixture(t), Surfaces: surfaces, Setup: "docs/docattest_setup.go"})
}

// readWritten writes a project into a directory of the test's own and reads it.
func readWritten(t *testing.T, files map[string]string) *Project {
	t.Helper()
	return read(t, Config{Root: written(t, files), InMemory: true, Surfaces: entries("default", ".")})
}

// written writes files into a directory of the test's own, and is that directory.
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

func read(t *testing.T, config Config) *Project {
	t.Helper()
	project, err := Read(config)
	if err != nil {
		t.Fatal(err)
	}
	return project
}

// entries are configured surfaces from pairs of a name and an entry.
func entries(pairs ...string) []protocol.ConfiguredSurface {
	out := []protocol.ConfiguredSurface{}
	for k := 0; k < len(pairs); k += 2 {
		entry, _ := json.Marshal(pairs[k+1])
		out = append(out, protocol.ConfiguredSurface{Name: pairs[k], Entry: entry})
	}
	return out
}

// symbolsOf are the symbols one attestation uses.
func symbolsOf(p *Project, a protocol.Attestation) ([]protocol.SymbolRef, error) {
	a.ID = "only"
	found, err := p.Symbols([]protocol.Attestation{a})
	return found["only"], err
}

func names(refs []protocol.SymbolRef) string {
	out := []string{}
	for _, ref := range refs {
		out = append(out, ref.Surface+":"+ref.Name)
	}
	return strings.Join(out, " ")
}

func TestSymbols(t *testing.T) {
	p := readFixture(t)
	cases := []struct {
		name string
		a    protocol.Attestation
		want string
	}{
		{"a block names what it selects from a surface it imports",
			protocol.Attestation{Kind: protocol.KindExample, Page: "docs/p.md", Code: "import (\n\tpricing \"example.com/shop\"\n\t\"example.com/shop/shipping\"\n)\n\n_, _ = pricing.Total(nil, \"NL\")\n_ = shipping.Carriers\n_ = pricing.private\n"},
			"default:Total shipping:Carriers"},
		{"an example of a source file names its owner and what it uses",
			protocol.Attestation{Kind: protocol.KindExample, Page: "shop_test.go", Line: 10},
			"default:ApplyDiscount default:EUR"},
		{"a fact names the exports its expression reads",
			protocol.Attestation{Kind: protocol.KindFact, Surface: "default", Expression: "len(TaxRates) + MaxDiscount + x.Total"},
			"default:TaxRates default:MaxDiscount"},
		{"a fact names the constants and literal variables it reads from source, and no other unexported name",
			protocol.Attestation{Kind: protocol.KindFact, Surface: "default", Expression: `fmt.Sprint(round(1), days["XX"]) + DefaultCurrency`},
			"default:DefaultCurrency"},
		{"a fact names the constants and literal variables it reads from source",
			protocol.Attestation{Kind: protocol.KindFact, Surface: "shipping", Expression: `fmt.Sprintf(unserved, days["XX"]) + Carriers[0]`},
			"shipping:unserved shipping:days shipping:Carriers"},
		{"a citation names what the cited test uses",
			protocol.Attestation{Kind: protocol.KindTest, File: "shop_test.go", Title: "rejects big discounts", Describes: []string{"TestTotals"}},
			"default:ApplyDiscount default:EUR default:ErrBadDiscount"},
		{"a cases table names what its runner uses",
			protocol.Attestation{Kind: protocol.KindCases, Runner: "delivery"},
			"shipping:Quote"},
		{"a surface that did not resolve has no symbols",
			protocol.Attestation{Kind: protocol.KindFact, Surface: "gone", Expression: "X"}, ""},
	}
	for _, c := range cases {
		found, err := symbolsOf(p, c.a)
		if got := names(found); err != nil || got != c.want {
			t.Errorf("%s: got %q, %v", c.name, got, err)
		}
	}
}

func TestListTests(t *testing.T) {
	p := readFixture(t)
	listing := p.ListTests("shop_test.go")
	got := testPaths(*listing.Tests)
	want := []string{"ExampleApplyDiscount", "ExampleTotal", "TestTotals", "TestTotals > adds tax", "TestTotals > rejects big discounts"}
	if !listing.OK || !reflect.DeepEqual(got, want) {
		t.Fatalf("got %v", got)
	}
	if suites := *listing.Suites; len(suites) != 1 || suites[0].Title != "BenchmarkTotal" || suites[0].Kind != "a benchmark" {
		t.Fatalf("got %+v", suites)
	}
	if missing := p.ListTests("no/such_test.go"); missing.OK || missing.Message != "no/such_test.go does not exist" {
		t.Fatalf("got %+v", missing)
	}
}

// testPaths are listed tests as their names and the names that enclose them, with "?"
// for a computed one.
func testPaths(tests []protocol.DeclaredTest) []string {
	out := []string{}
	for _, test := range tests {
		path := []string{}
		for _, name := range test.Describes {
			if name == nil {
				path = append(path, "?")
			} else {
				path = append(path, *name)
			}
		}
		out = append(out, strings.Join(append(path, test.Title), " > "))
	}
	return out
}

func TestSubtestsOfTableDrivenTests(t *testing.T) {
	files := map[string]string{
		"go.mod": "module example.com/m\n\ngo 1.21\n",
		"m.go":   "package m\n",
		"m_test.go": `package m

import "testing"

func TestKeyed(t *testing.T) {
	tests := []struct {
		name string
		in   int
	}{
		{name: "zero", in: 0},
		{in: 1, name: "one"},
		{name: "zero", in: 2},
		{name: "computed " + "name", in: 3},
	}
	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			t.Run("inner", func(t *testing.T) {})
		})
	}
}

func TestPositional(t *testing.T) {
	for _, tc := range []*struct {
		in         int
		name, want string
	}{{1, "first", "a"}, {2, "second", "b"}} {
		t.Run(tc.name, func(t *testing.T) {})
	}
}

func TestMap(t *testing.T) {
	cases := map[string]struct{ label string }{"by key": {"by field"}}
	for name, c := range cases {
		t.Run(name, func(t *testing.T) {})
		t.Run(c.label, func(t *testing.T) {})
	}
}

func TestNotTables(t *testing.T) {
	grown := []struct{ name string }{{"grown"}}
	grown = append(grown, struct{ name string }{"more"})
	for _, tc := range grown {
		t.Run(tc.name, func(t *testing.T) {})
	}
	for i := range []string{"index"} {
		t.Run(string(rune(i)), func(t *testing.T) {})
	}
	for _, tc := range cases() {
		t.Run(tc.name, func(t *testing.T) {})
	}
}

func cases() []struct{ name string } { return nil }
`,
	}
	got := testPaths(*readWritten(t, files).ListTests("m_test.go").Tests)
	want := []string{
		"TestKeyed", "TestKeyed > zero", "TestKeyed > zero > inner", "TestKeyed > one", "TestKeyed > one > inner", "TestKeyed > ? > inner",
		"TestPositional", "TestPositional > first", "TestPositional > second",
		"TestMap", "TestMap > by key", "TestMap > by field",
		"TestNotTables",
	}
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("got %q", got)
	}
}

func TestSubtestsWithComputedNames(t *testing.T) {
	files := map[string]string{
		"go.mod":    "module example.com/m\n\ngo 1.21\n",
		"m.go":      "package m\n",
		"m_test.go": "package m\n\nimport \"testing\"\n\nfunc TestAll(t *testing.T) {\n\tfor _, name := range []string{\"a\"} {\n\t\tt.Run(name, func(t *testing.T) {\n\t\t\tt.Run(\"inner\", func(t *testing.T) {})\n\t\t})\n\t}\n}\n\nfunc Testify() {}\n",
	}
	tests := *readWritten(t, files).ListTests("m_test.go").Tests
	if len(tests) != 2 || tests[1].Title != "inner" || *tests[1].Describes[0] != "TestAll" || tests[1].Describes[1] != nil {
		text, _ := json.Marshal(tests)
		t.Fatalf("got %s", text)
	}
}

func TestInCode(t *testing.T) {
	p := readFixture(t)
	got, err := p.InCode([]string{"Gone", "Quote", "delivery", "round"}, []string{"docs/docattest_setup.go"})
	if err != nil || !reflect.DeepEqual(got, []string{"Quote", "delivery", "round"}) {
		t.Fatalf("got %v, %v", got, err)
	}
}

func TestAffected(t *testing.T) {
	read := func(path string) string {
		text, err := os.ReadFile(fixture(t) + "/" + path)
		if err != nil {
			t.Fatal(err)
		}
		return string(text)
	}
	shop, shipping := read("shop.go"), read("shipping/shipping.go")
	p := readFixture(t)
	change := func(path, now, was string) string {
		text := strings.Replace(map[string]string{"shop.go": shop, "shipping/shipping.go": shipping}[path], now, was, 1)
		found, err := p.Affected(map[string]*string{path: &text})
		if err != nil {
			t.Fatal(err)
		}
		return names(found)
	}
	cases := []struct{ name, path, now, was, want string }{
		{"a helper reaches the exports that use it", "shop.go", "(hundredths + 50) / 100", "hundredths / 100", "default:ApplyDiscount default:Total"},
		{"a constant reaches what reads it", "shop.go", "MaxDiscount = 50", "MaxDiscount = 40", "default:MaxDiscount default:ApplyDiscount"},
		{"a method is part of its type", "shop.go", `"%d.%02d %s"`, `"%d,%02d %s"`, "default:Money default:EUR default:ApplyDiscount default:Total"},
		{"layout and comments change nothing", "shop.go", "// round divides", "//   round    divides", ""},
		// A fact reads a literal variable from source, so a change to it is listed.
		{"a literal variable of another surface", "shipping/shipping.go", `"DE": 2`, `"DE": 3`, "shipping:days shipping:Quote"},
		{"an export that was not there at the base", "shipping/shipping.go", "// Quote is", "func Old() {}\n\n// Quote is", "shipping:Old"},
		// A fact reads an unexported constant from source, so a change to it is listed.
		{"an unexported constant", "shipping/shipping.go", "serves %q", "delivers to %q", "shipping:unserved shipping:Quote"},
		{"an unexported constant that is gone", "shipping/shipping.go", "// Quote is", "const gone = 1\n\n// Quote is", "shipping:gone"},
	}
	for _, c := range cases {
		if got := change(c.path, c.now, c.was); got != c.want {
			t.Errorf("%s: got %q", c.name, got)
		}
	}
	if found, err := p.Affected(map[string]*string{"shipping/shipping.go": nil}); err != nil || names(found) != "shipping:Carriers shipping:days shipping:unserved shipping:Quote" {
		t.Errorf("a file the base did not have: got %q, %v", names(found), err)
	}
}

func TestAffectedListsTheConstantsOfAPackageMain(t *testing.T) {
	p := readFixture(t, entries("shopd", "./cmd/shopd")...)
	text, err := os.ReadFile(fixture(t) + "/cmd/shopd/main.go")
	if err != nil {
		t.Fatal(err)
	}
	was := strings.Replace(string(text), "listenPort = 8080", "listenPort = 80", 1)
	was = strings.Replace(was, "time.Now()", "time.Unix(0, 0)", 1)
	found, err := p.Affected(map[string]*string{"cmd/shopd/main.go": &was})
	// started is a variable whose value is no composite literal, which no fact reads: it
	// is not listed.
	if err != nil || names(found) != "shopd:listenPort" {
		t.Fatalf("got %q, %v", names(found), err)
	}
}

func TestSurfaceEntries(t *testing.T) {
	both := protocol.ConfiguredSurface{Name: "both", Entry: json.RawMessage(`{"import": "example.com/alias", "path": "shipping"}`)}
	p := readFixture(t, append(entries(
		"byImport", "example.com/shop/shipping",
		"byFile", "./shipping/shipping.go",
		"standard", "errors",
		"unknown", "example.com/elsewhere/nothing",
	), both)...)
	want := map[string][2]string{
		"byImport": {"example.com/shop/shipping", "shipping"},
		"byFile":   {"example.com/shop/shipping", "shipping"},
		"both":     {"example.com/alias", "shipping"},
	}
	described := p.DescribeSurfaces()
	for name, w := range want {
		got := described[name].Resolution
		if got == nil || got.Import != w[0] || got.Entry != w[1] {
			t.Errorf("%s: got %+v", name, got)
		}
	}
	if names, ok := p.ExportsOf(p.SurfaceNamed("standard")); !ok || !strings.Contains(strings.Join(names, " "), "New") {
		t.Errorf("a package of the standard library is found by the go command: got %v", names)
	}
	if described["unknown"].Resolution != nil || len(described["unknown"].Warnings) != 1 {
		t.Errorf("got %+v", described["unknown"])
	}
}

func TestSurfaces(t *testing.T) {
	p := readFixture(t)
	names, ok := p.ExportsOf(p.SurfaceNamed("default"))
	want := []string{"DefaultCurrency", "MaxDiscount", "TaxRates", "ErrorCodes", "ErrBadDiscount", "Money", "EUR", "ApplyDiscount", "Total"}
	if !ok || !reflect.DeepEqual(names, want) {
		t.Fatalf("got %v", names)
	}
	if _, ok := p.ExportsOf(p.SurfaceNamed("gone")); ok {
		t.Fatal("a surface that did not resolve has no exports")
	}
	described := p.DescribeSurfaces()
	if got := *described["shipping"].Resolution; got.Import != "example.com/shop/shipping" || got.Entry != "shipping" {
		t.Fatalf("got %+v", got)
	}
	if got := described["gone"]; got.Resolution != nil || !strings.Contains(got.Warnings[0], `"./no/such/package" did not resolve`) {
		t.Fatalf("got %+v", got)
	}
}

func TestAnEntryOfTheWrongShapeIsRefused(t *testing.T) {
	_, err := Read(Config{Root: fixture(t), Surfaces: []protocol.ConfiguredSurface{{Name: "odd", Entry: json.RawMessage("7")}}})
	if err == nil || !strings.Contains(err.Error(), `surface "odd": the entry 7 is neither a string nor an import and a path`) {
		t.Fatalf("got %v", err)
	}
}
