package running

import (
	"cmp"
	"encoding/json"
	"os"
	"path/filepath"
	"reflect"
	"strings"
	"testing"

	"github.com/docattest/docattest/adapters/go/internal/analysis"
	"github.com/docattest/docattest/adapters/go/internal/protocol"
)

// opening is the project a test opens.
type opening struct {
	// files are the project, written for the test; nil opens the fixture,
	// adapters/go/testdata/fixture.
	files map[string]string
	// surfaces are the fixture's when nil.
	surfaces []protocol.ConfiguredSurface
	setup    string
	noSetup  bool
	network  bool
	// timeoutMs is how long a row or a block may run; defaultTestTimeoutMs when zero.
	timeoutMs float64
}

// defaultTestTimeoutMs is how long a row or a block of a test may run, unless it says.
const defaultTestTimeoutMs = 2000

func open(t *testing.T, how opening) *Runner {
	t.Helper()
	config := analysis.Config{Root: fixture(t), Surfaces: how.surfaces, Setup: "docs/docattest_setup.go", Work: t.TempDir()}
	if how.files != nil {
		config.Root, config.InMemory = written(t, how.files), true
	}
	if how.surfaces == nil {
		config.Surfaces = entries("default", ".", "shipping", "./shipping", "gone", "./no/such/package")
	}
	if how.setup != "" {
		config.Setup = how.setup
	}
	if how.noSetup {
		config.Setup = ""
	}
	project, err := analysis.Read(config)
	if err != nil {
		t.Fatal(err)
	}
	options := Options{
		Work: t.TempDir(), TimeoutMs: cmp.Or(how.timeoutMs, defaultTestTimeoutMs), Workers: 4, Network: how.network,
		Snapshots: Snapshots{Dir: "docs/.snapshots", Data: json.RawMessage(`{"regions": ["NL", "DE"], "sizes": [3000000, 1.5]}`)},
		Manifests: json.RawMessage(`{"go.mod": {"module": "example.com/shop", "go": "1.23"}, "config.json": {"limits": {"bytes": 1048576}, "ratio": 1.0}}`),
		Texts:     json.RawMessage(`{"Dockerfile": "FROM golang:1.23\nUSER 10001\n"}`),
		Matches:   json.RawMessage(`{"Dockerfile": {"^USER (\\d+)$": "10001"}}`),
		Fields:    json.RawMessage(`{"ci.yml": {"deploy.variables.VERSION": "0.2.2", "deploy.replicas": 3, "deploy.ratio": 1.5, "deploy.debug": null}}`),
	}
	runner := New(project, options, nil)
	t.Cleanup(runner.Wait)
	return runner
}

func fixture(t *testing.T) string {
	t.Helper()
	root, err := filepath.Abs("../../testdata/fixture")
	if err != nil {
		t.Fatal(err)
	}
	return root
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

// entries are configured surfaces from pairs of a name and an entry.
func entries(pairs ...string) []protocol.ConfiguredSurface {
	out := []protocol.ConfiguredSurface{}
	for k := 0; k < len(pairs); k += 2 {
		entry, _ := json.Marshal(pairs[k+1])
		out = append(out, protocol.ConfiguredSurface{Name: pairs[k], Entry: entry})
	}
	return out
}

func onPage(id, code string) protocol.Example {
	return protocol.Example{ID: id, Page: "docs/page.md", Line: 10, Kind: protocol.KindExample, Lang: "go", Mode: "check", Code: code}
}

const imports = "import (\n\t\"fmt\"\n\n\t\"example.com/shop\"\n)\n\n"

func TestBlocksCompileAndRun(t *testing.T) {
	r := open(t, opening{})
	examples := []protocol.Example{
		onPage("passes", imports+"price, _ := shop.ApplyDiscount(shop.EUR(1000), 25)\nfmt.Println(price)\n// Output: 7.50 EUR"),
		onPage("differs", imports+"fmt.Println(shop.MaxDiscount)\n// Output: 60"),
		onPage("panics", "var m map[string]int\nm[\"a\"] = 1"),
		onPage("broken", "import \"example.com/shop\"\n\nvar _ = shop.AplyDiscount"),
		onPage("weak", imports+"fmt.Println(shop.EUR(5))"),
		onPage("slow", "for {\n}"),
		onPage("offline", "import \"net\"\n\nif _, err := net.LookupHost(\"example.com\"); err != nil {\n\tpanic(err)\n}"),
		onPage("library", "package notmain\n\nfunc F() {}\n"),
	}
	diagnostics := r.Compile(examples)
	if len(diagnostics) != 1 || diagnostics[0] != (protocol.Diagnostic{ID: "broken", Message: "undefined: shop.AplyDiscount", Line: 3}) {
		t.Fatalf("got %+v", diagnostics)
	}
	network := onPage("network", "println(1)")
	network.Network = true
	results := r.Run(append(examples, network), nil)
	want := []struct {
		status  protocol.Status
		message string
	}{
		{"pass", "printed the output it states"},
		{"fail", `expected output "60", got "50"`},
		{"fail", "panic: assignment to entry in nil map (block line 2)"},
		{"fail", "does not compile: undefined: shop.AplyDiscount (block line 3)"},
		{"pass", "ran to its end, but asserts nothing (weak)"},
		{"fail", "timed out after 2000 ms"},
		{"fail", "the network is off"},
		{"fail", "the block builds no program"},
		{"skipped", "network example skipped"},
	}
	for k, w := range want {
		if results[k].Status != w.status || !strings.Contains(results[k].Message, w.message) {
			t.Errorf("%s: got %s: %s", append(examples, network)[k].ID, results[k].Status, results[k].Message)
		}
	}
	if results[1].Expected != "60" || results[1].Actual != "50" || results[0].Line != 10 || results[0].Kind != protocol.KindExample {
		t.Errorf("got %+v, %+v", results[0], results[1])
	}
}

func TestDocExamplesRunAsGoTestRunsThem(t *testing.T) {
	r := open(t, opening{})
	stated := protocol.Example{ID: "a", Page: "shop_test.go", Line: 10, Kind: protocol.KindExample, Mode: "check"}
	silent := protocol.Example{ID: "b", Page: "shop_test.go", Line: 17, Kind: protocol.KindExample, Mode: "compile"}
	gone := protocol.Example{ID: "c", Page: "shop_test.go", Line: 3, Kind: protocol.KindExample, Mode: "check"}
	if diagnostics := r.Compile([]protocol.Example{stated, silent}); len(diagnostics) != 0 {
		t.Fatalf("got %+v", diagnostics)
	}
	results := r.Run([]protocol.Example{stated, silent, gone}, nil)
	if results[0].Status != "pass" || results[0].Message != "printed the output it states" {
		t.Errorf("got %+v", results[0])
	}
	if results[1].Status != "pass" || !strings.Contains(results[1].Message, "states no output") {
		t.Errorf("got %+v", results[1])
	}
	if results[2].Status != "fail" || !strings.Contains(results[2].Message, "no example function is declared at shop_test.go:3") {
		t.Errorf("got %+v", results[2])
	}
}

func TestADocExampleThatFailsSaysBothSides(t *testing.T) {
	files := map[string]string{
		"go.mod":        "module example.com/m\n\ngo 1.21\n",
		"m.go":          "package m\n\nfunc Two() int { return 2 }\n",
		"m_test.go":     "package m\n\nimport \"fmt\"\n\nfunc ExampleTwo() {\n\tfmt.Println(Two())\n\t// Output: 3\n}\n\nfunc ExampleTwo_panics() {\n\tvar m map[int]int\n\tm[1] = Two()\n\t// Output: 3\n}\n",
		"bad/b.go":      "package bad\n",
		"bad/b_test.go": "package bad\n\nfunc ExampleX() {\n\tundefinedName()\n\t// Output: 1\n}\n",
	}
	r := open(t, opening{files: files, surfaces: entries("default", "."), noSetup: true})
	at := func(page string, line int) protocol.Example {
		return protocol.Example{ID: page, Page: page, Line: line, Kind: protocol.KindExample, Mode: "check"}
	}
	results := r.Run([]protocol.Example{at("m_test.go", 5), at("m_test.go", 10), at("bad/b_test.go", 3)}, nil)
	if results[0].Status != "fail" || results[0].Expected != "3" || results[0].Actual != "2" {
		t.Errorf("got %+v", results[0])
	}
	if !strings.Contains(results[1].Message, "panic: assignment to entry in nil map") || !strings.Contains(results[1].Message, "(block line 3)") {
		t.Errorf("got %+v", results[1])
	}
	if results[2].Message != "does not compile: undefined: undefinedName (block line 2)" {
		t.Errorf("got %+v", results[2])
	}
}

func fact(id, expression string) protocol.Fact {
	return protocol.Fact{ID: id, Page: "docs/page.md", Line: 1, Expression: expression, Surface: "default"}
}

func TestEvaluateShowsValuesAsGoPrintsThem(t *testing.T) {
	r := open(t, opening{})
	elsewhere := fact("elsewhere", "Carriers")
	elsewhere.Surface = "shipping"
	got := r.Evaluate([]protocol.Fact{
		fact("scalar", "MaxDiscount"), fact("text", "DefaultCurrency"), fact("keys", "TaxRates"),
		fact("call", `Rates("NL")`), fact("standard", "strings.ToLower(DefaultCurrency)"),
		fact("snapshot", `snapshot("regions")`), fact("manifest", `manifest("go.mod")["go"]`), fact("plain", "1 + 1"), elsewhere,
		fact("file", "regexp.MustCompile(`USER (\\d+)`).FindStringSubmatch(file(\"Dockerfile\"))[1]"),
	})
	values := map[string]string{"scalar": "50", "text": "EUR", "keys": "map[DE:19 FR:20 NL:21]", "call": "[21%]",
		"standard": "eur", "snapshot": "[NL DE]", "manifest": "1.23", "plain": "2", "elsewhere": "[post courier drone]", "file": "10001"}
	for id, text := range values {
		if !got[id].OK || got[id].Value.Text != text {
			t.Errorf("%s: got %+v", id, got[id])
		}
	}
	if !reflect.DeepEqual(got["keys"].Value.Items, []string{"DE", "FR", "NL"}) || got["text"].Value.Kind != "text" || got["scalar"].Value.Kind != "" {
		t.Errorf("got %+v, %+v", got["keys"].Value, got["text"].Value)
	}
}

func TestNumbersOfSnapshotsAndManifestsAreAsGoHoldsThem(t *testing.T) {
	r := open(t, opening{})
	got := r.Evaluate([]protocol.Fact{
		fact("integer", `manifest("config.json")["limits"].(map[string]any)["bytes"]`),
		fact("integerType", `fmt.Sprintf("%T", manifest("config.json")["limits"].(map[string]any)["bytes"])`),
		fact("float", `manifest("config.json")["ratio"]`),
		fact("floatType", `fmt.Sprintf("%T", manifest("config.json")["ratio"])`),
		fact("snapshot", `snapshot("sizes")`),
	})
	values := map[string]string{"integer": "1048576", "integerType": "int64", "float": "1", "floatType": "float64", "snapshot": "[3000000 1.5]"}
	for id, text := range values {
		if !got[id].OK || got[id].Value.Text != text {
			t.Errorf("%s: got %+v", id, got[id])
		}
	}
}

func TestMatchedAndFieldAreWhatTheCoreWorkedOutHeldAsGoHoldsThem(t *testing.T) {
	r := open(t, opening{})
	got := r.Evaluate([]protocol.Fact{
		fact("matched", "matched(\"Dockerfile\", `^USER (\\d+)$`)"),
		fact("string", `manifestField("ci.yml", "deploy.variables.VERSION")`),
		fact("concat", `manifestField("ci.yml", "deploy.variables.VERSION") + "-alpine"`),
		fact("asserted", `manifestField("ci.yml", "deploy.variables.VERSION").(string) + "-ok"`),
		fact("added", `manifestField("ci.yml", "deploy.replicas") + 1`),
		fact("integerType", `fmt.Sprintf("%T", manifestField("ci.yml", "deploy.replicas"))`),
		fact("floatType", `fmt.Sprintf("%T", manifestField("ci.yml", "deploy.ratio"))`),
		fact("null", `manifestField("ci.yml", "deploy.debug")`),
		fact("unreadMatch", `matched("Dockerfile", "USER")`),
		fact("unreadField", `manifestField("ci.yml", "deploy.image")`),
	})
	values := map[string]string{
		"matched": "10001", "string": "0.2.2", "concat": "0.2.2-alpine", "asserted": "0.2.2-ok",
		"added": "4", "integerType": "int64", "floatType": "float64", "null": "<nil>",
	}
	for id, text := range values {
		if !got[id].OK || got[id].Value.Text != text {
			t.Errorf("%s: got %+v", id, got[id])
		}
	}
	messages := map[string]string{
		"unreadMatch": `panicked: matched("Dockerfile", "USER") was not read: write both arguments out in the fact itself, as string literals`,
		"unreadField": `panicked: manifestField("ci.yml", "deploy.image") was not read`,
	}
	for id, message := range messages {
		if got[id].OK || !strings.Contains(got[id].Message, message) {
			t.Errorf("%s: got %+v", id, got[id])
		}
	}
}

func TestEvaluateSaysWhyAFactHasNoValue(t *testing.T) {
	r := open(t, opening{})
	unknown := fact("unknown", "X")
	unknown.Surface = "nowhere"
	got := r.Evaluate([]protocol.Fact{
		fact("missing", `snapshot("none")`), fact("unread", `manifest("go.sum")`), fact("unreadFile", `file("Dockerfile.dev")`),
		fact("private", "round(50)"), fact("syntax", "MaxDiscount +"),
		fact("panics", `TaxRates["NL"] / TaxRates["XX"]`), fact("typed", `MaxDiscount + "x"`), fact("fine", "MaxDiscount"), unknown,
	})
	messages := map[string]string{
		"missing":    "fact `snapshot(\"none\")` panicked: snapshot \"none\" not found in docs/.snapshots",
		"unread":     "panicked: manifest \"go.sum\" was not read: name it in the fact itself, as manifest(\"go.sum\")",
		"unreadFile": "panicked: file \"Dockerfile.dev\" was not read: name it in the fact itself, as file(\"Dockerfile.dev\")",
		"private":    "fact `round(50)`: round is a function that surface \"default\" does not export; of its unexported names, a fact reads only constants",
		"syntax":     "cannot parse fact expression `MaxDiscount +`",
		"panics":     "panicked: runtime error: integer divide by zero",
		"typed":      "fact `MaxDiscount + \"x\"`: ",
		"unknown":    `unknown surface "nowhere" (configured: default, shipping, gone)`,
	}
	for id, message := range messages {
		if got[id].OK || !strings.Contains(got[id].Message, message) {
			t.Errorf("%s: got %+v", id, got[id])
		}
	}
	if !got["fine"].OK || len(got) != 9 {
		t.Errorf("one wrong fact costs the others nothing: got %+v", got)
	}
}

func TestCases(t *testing.T) {
	r := open(t, opening{})
	table := func(runner string, header []string, rows ...map[string]string) protocol.Cases {
		out := protocol.Cases{ID: runner, Page: "docs/page.md", Line: 20, Kind: protocol.KindCases, Runner: runner, Header: header}
		for k, values := range rows {
			out.Rows = append(out.Rows, protocol.CasesRow{Line: 22 + k, Name: values["case"], Values: values})
		}
		return out
	}
	discount := []string{"case", "price", "percent", "result"}
	results := r.Run(nil, []protocol.Cases{
		table("discount", discount,
			map[string]string{"case": "a quarter", "price": "1000", "percent": "25", "result": "750"},
			map[string]string{"case": "wrong", "price": "1000", "percent": "25", "result": "800"},
			map[string]string{"case": "too much", "price": "1000", "percent": "90", "result": "100"}),
		table("nope", []string{"case"}, map[string]string{"case": "a"}),
		table("discount", []string{"case", "price"}, map[string]string{"case": "a"}),
		table("discount", discount),
	})
	got := []string{}
	for _, result := range results {
		got = append(got, strings.Join([]string{string(result.Status), string(result.Kind), result.Message}, "|")+"@"+string(rune('0'+result.Line-20)))
	}
	want := []string{
		"pass|cases|docs/page.md:22 a quarter@2",
		"fail|cases|docs/page.md:23 wrong: expected 800, got 750@3",
		"fail|cases|docs/page.md:24 too much: The discount is outside the allowed range@4",
		`error|cases|cases runner "nope" at L20 is not in Cases of docs/docattest_setup.go (known: delivery, discount, total)@0`,
		`error|cases|cases "discount" at L20: the table is missing columns "percent", "result" declared by the runner@0`,
	}
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("got\n%s", strings.Join(got, "\n"))
	}
}

func TestFactsRunOfflineEvenWhenTheNetworkIsOn(t *testing.T) {
	// Only a block tagged `check network` may reach the network; a fact cannot be tagged.
	// The harness refuses by swapping in a resolver whose Dial refuses every lookup.
	refusing := fact("refusing", "net.DefaultResolver.Dial != nil")
	for _, network := range []bool{false, true} {
		got := open(t, opening{network: network}).Evaluate([]protocol.Fact{refusing})["refusing"]
		if !got.OK || got.Value.Text != "true" {
			t.Errorf("network %v: got %+v", network, got)
		}
	}
}

func TestObservables(t *testing.T) {
	r := open(t, opening{})
	found, err := r.Observables("default")
	if err != nil {
		t.Fatal(err)
	}
	got := map[string]string{}
	for _, o := range found {
		got[o.Expression] = o.Value.Text
		if o.Surface != "default" {
			t.Errorf("got %+v", o)
		}
	}
	want := map[string]string{"MaxDiscount": "50", "DefaultCurrency": "EUR", "TaxRates": "map[DE:19 FR:20 NL:21]",
		`TaxRates["NL"]`: "21", "ErrBadDiscount": "The discount is outside the allowed range",
		// The setup package's helpers that take no argument they must be given, called.
		"ErrorMessages()": "[No carrier serves the region The discount is outside the allowed range The item is not available]",
		"Rates()":         "[]"}
	for expression, text := range want {
		if got[expression] != text {
			t.Errorf("%s: got %q", expression, got[expression])
		}
	}
	if _, called := got["Codes()"]; called {
		t.Error("a helper that takes an argument was called")
	}
	shipping, err := r.Observables("shipping")
	if err != nil {
		t.Fatal(err)
	}
	for _, o := range shipping {
		if strings.HasSuffix(o.Expression, "()") {
			t.Errorf("a setup helper is offered on a surface other than the default: %+v", o)
		}
	}
	if _, err := r.Observables("gone"); err == nil || !strings.HasPrefix(err.Error(), `docattest: surface "gone"`) {
		t.Fatalf("got %v", err)
	}
	if _, isUser := err.(protocol.UserError); err != nil && !isUser {
		t.Fatalf("got %T", err)
	}
}

func TestObservablesIncludesDocsHelpersAndSourceReadableValues(t *testing.T) {
	runner := open(t, opening{
		files: map[string]string{
			"go.mod": "module example.com/svc\n\ngo 1.21\n",
			"guard/guard.go": `package guard

const defaultTimeout = 30
var allowedEnv = []string{"PATH", "HOME"}
var mutableCounter = 0

func Bump() { mutableCounter++ }
`,
			"guard/docs.go": `//go:build docattest

package guard

const DocsHelperConst = "doc-only"

func ForbiddenNames() []string { return []string{"cap_sys_admin", "cap_net_raw"} }
func NeedsArg(x int) int { return x }
`,
			"cmd/svc/main.go": `package main

const defaultListen = "127.0.0.1:8080"
var defaultPorts = []int{8080, 9090}

func main() {}
`,
		},
		surfaces: entries("default", "./guard", "cmd", "./cmd/svc"),
		noSetup:  true,
	})

	guardObs, err := runner.Observables("default")
	if err != nil {
		t.Fatal(err)
	}
	gotGuard := map[string]string{}
	for _, o := range guardObs {
		gotGuard[o.Expression] = o.Value.Text
	}
	for expr, want := range map[string]string{
		"defaultTimeout":   "30",
		"allowedEnv":       "[PATH HOME]",
		"DocsHelperConst":  "doc-only",
		"ForbiddenNames()": "[cap_sys_admin cap_net_raw]",
	} {
		if gotGuard[expr] != want {
			t.Errorf("default %s: got %q, want %q (all: %v)", expr, gotGuard[expr], want, gotGuard)
		}
	}
	if _, ok := gotGuard["mutableCounter"]; ok {
		t.Errorf("mutableCounter should not be observable: %v", gotGuard)
	}
	if _, ok := gotGuard["NeedsArg()"]; ok {
		t.Errorf("NeedsArg() takes an arg and should not be called: %v", gotGuard)
	}

	cmdObs, err := runner.Observables("cmd")
	if err != nil {
		t.Fatal(err)
	}
	gotCmd := map[string]string{}
	for _, o := range cmdObs {
		gotCmd[o.Expression] = o.Value.Text
	}
	for expr, want := range map[string]string{
		"defaultListen": "127.0.0.1:8080",
		"defaultPorts":  "[8080 9090]",
	} {
		if gotCmd[expr] != want {
			t.Errorf("cmd %s: got %q, want %q (all: %v)", expr, gotCmd[expr], want, gotCmd)
		}
	}
}
