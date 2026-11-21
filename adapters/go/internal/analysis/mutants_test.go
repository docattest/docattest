package analysis

import (
	"go/parser"
	"go/token"
	"path/filepath"
	"reflect"
	"strings"
	"testing"

	"github.com/docattest/docattest/adapters/go/internal/protocol"
)

// mutable declares a value of each kind that `mutants` changes, and of each it leaves
// alone. A non-ASCII character comes before every literal, so that an offset in runes
// would not be one in bytes.
var mutable = map[string]string{
	"go.mod": "module example.com/mutable\n\ngo 1.21\n",
	"values.go": `// Package mutable is a café of values.
package mutable

import "time"

const Port = 8080

const (
	Mask      = 0x10
	Ratio     = 1.5
	Big       = 1e3
	Low, High = 1, 2
)

const Greeting = "a\tb\"c"

const Raw = ` + "`x\\n`" + `

const On = true

const Initial = 'a'

const Max uint8 = 255

const Timeout = 5 * time.Second

const Period = time.Second

const Zero = time.Duration(0) - time.Duration(0)

const lower = "quiet"

const Alias = lower

const AlsoOn = On

var Levels = []string{"DEBUG", "LOG_LEVEL"}

var One = []int{7}

var Hosts = []string{
	"a",
	"b",
}

var Solo = []string{
	"only",
}

var Pair = [2]int{1, 2}

var Trio = [...]int{1, 2}

var Grid = [][]int{{1}, {2, 3}}

var Limits = map[string]int{"small": 1}

var Flags = map[int]bool{1: true, 2: false}

var Server = struct {
	Host string
	TLS  bool
}{Host: "local", TLS: true}

var Wrapped = wrap(3, func() int { return 4 })

var Buffer = [8]byte([]byte("12345678"))

var Started = time.Now()

var Copy = Port

var Nothing *int = nil

var Unset int

const (
	A = iota
	B
)

func wrap(n int, f func() int) int { return n + f() }

func Add(a, b int) int { return a + b }

type Money int
`,
	"shadow/shadow.go": `package shadow

const true = false

const Flag = true
`,
}

// mutantsCase is a symbol and what `mutants` answers of it: its edits' descriptions,
// or why it has none.
type mutantsCase struct {
	symbol protocol.SymbolRef
	edits  []string
	why    protocol.MutantsWhy
}

func of(name string) protocol.SymbolRef { return protocol.SymbolRef{Name: name, Surface: "default"} }

func TestMutantsChangeTheLiteralsOfAValue(t *testing.T) {
	cases := []mutantsCase{
		{symbol: of("Port"), edits: []string{"8080 → 8081"}},
		{symbol: of("Mask"), edits: []string{"0x10 → 17"}},
		{symbol: of("Ratio"), edits: []string{"1.5 → 2.5"}},
		{symbol: of("Big"), edits: []string{"1e3 → 1001.0"}},
		{symbol: of("High"), edits: []string{"2 → 3"}},
		{symbol: of("Greeting"), edits: []string{`"a\tb\"c" → "a\tb\"cx"`}},
		{symbol: of("Raw"), edits: []string{"`x\\n` → `x\\nx`"}},
		{symbol: of("On"), edits: []string{"true → false"}},
		{symbol: of("Timeout"), edits: []string{"5 → 6"}},
		{symbol: of("Period"), edits: []string{"time.Second → 2 * time.Second"}},
		{symbol: of("Alias"), edits: []string{`lower → lower + "x"`}},
		{symbol: of("AlsoOn"), edits: []string{"On → !On"}},
		{symbol: of("Copy"), edits: []string{"Port → 2 * Port"}},
		{symbol: of("lower"), edits: []string{`"quiet" → "quietx"`}},
		{symbol: of("Levels"), edits: []string{`"LOG_LEVEL" removed`, `"DEBUG" → "DEBUGx"`, `"LOG_LEVEL" → "LOG_LEVELx"`}},
		{symbol: of("One"), edits: []string{"7 removed", "7 → 8"}},
		{symbol: of("Hosts"), edits: []string{`"b" removed`, `"a" → "ax"`, `"b" → "bx"`}},
		{symbol: of("Pair"), edits: []string{"1 → 2", "2 → 3"}},
		{symbol: of("Trio"), edits: []string{"2 removed", "1 → 2", "2 → 3"}},
		{symbol: of("Grid"), edits: []string{"{…} removed", "1 removed", "1 → 2", "3 removed", "2 → 3", "3 → 4"}},
		{symbol: of("Limits"), edits: []string{`"small" → "smallx"`, "1 → 2"}},
		// 1 → 2 would make two keys the same.
		{symbol: of("Flags"), edits: []string{"true → false", "2 → 3", "false → true"}},
		{symbol: of("Server"), edits: []string{`"local" → "localx"`, "true → false"}},
		{symbol: of("Wrapped"), edits: []string{"3 → 4"}},
		// The 8 is in a type.
		{symbol: of("Buffer"), edits: []string{`"12345678" → "12345678x"`}},
	}
	checkMutants(t, cases, 8)
}

func TestMutantsSayWhyAValueHasNone(t *testing.T) {
	cases := []mutantsCase{
		{symbol: of("Initial"), why: protocol.WhyNoLiteral},
		// 256 is no uint8.
		{symbol: of("Max"), why: protocol.WhyNoLiteral},
		{symbol: of("Started"), why: protocol.WhyNoLiteral},
		{symbol: of("Nothing"), why: protocol.WhyNoLiteral},
		{symbol: of("Unset"), why: protocol.WhyNoLiteral},
		{symbol: of("A"), why: protocol.WhyNoLiteral},
		{symbol: of("B"), why: protocol.WhyNoLiteral},
		{symbol: protocol.SymbolRef{Name: "Flag", Surface: "shadow"}, edits: []string{"true → !true"}},
		{symbol: of("Add"), why: protocol.WhyFunction},
		{symbol: of("Money"), why: protocol.WhyFunction},
		{symbol: of("Missing"), why: protocol.WhyNotFound},
		{symbol: protocol.SymbolRef{Name: "Port", Surface: "gone"}, why: protocol.WhyNotFound},
	}
	checkMutants(t, cases, 8)
}

func TestMutantsOfASymbolWithoutASurfaceAreOfAValueReadFromSource(t *testing.T) {
	cases := []mutantsCase{
		{symbol: protocol.SymbolRef{Name: "Port"}, edits: []string{"8080 → 8081"}},
		{symbol: protocol.SymbolRef{Name: "lower"}, edits: []string{`"quiet" → "quietx"`}},
		{symbol: protocol.SymbolRef{Name: "Levels"}, edits: []string{`"LOG_LEVEL" removed`, `"DEBUG" → "DEBUGx"`, `"LOG_LEVEL" → "LOG_LEVELx"`}},
		// No fact reads a function, or a variable that is no literal variable, from source.
		{symbol: protocol.SymbolRef{Name: "Started"}, why: protocol.WhyNotFound},
		{symbol: protocol.SymbolRef{Name: "Add"}, why: protocol.WhyNotFound},
		{symbol: protocol.SymbolRef{Name: "Missing"}, why: protocol.WhyNotFound},
	}
	checkMutants(t, cases, 8)
}

func TestMutantsAreTheFirstUpToTheLimit(t *testing.T) {
	checkMutants(t, []mutantsCase{
		{symbol: of("Levels"), edits: []string{`"LOG_LEVEL" removed`, `"DEBUG" → "DEBUGx"`}},
		{symbol: of("Grid"), edits: []string{"{…} removed", "1 removed"}},
	}, 2)
}

func TestARemovedItemLeavesAListGoReads(t *testing.T) {
	answer := mutantsOfMutable(t, []protocol.SymbolRef{of("Levels"), of("One"), of("Hosts"), of("Solo")}, 1)
	want := []string{
		"var Levels = []string{\"DEBUG\"}\n", "var One = []int{}\n", "var Hosts = []string{\n\t\"a\",\n}\n",
		"var Solo = []string{}\n",
	}
	for k, symbol := range answer {
		if edited := applied(symbol.Edits[0]); !strings.Contains(edited, want[k]) {
			t.Errorf("no %q in\n%s", want[k], edited)
		}
	}
}

// checkMutants checks what `mutants` answers of each case, and that each edit replaces
// what its description says it changes, in a file Go still parses.
func checkMutants(t *testing.T, cases []mutantsCase, limit int) {
	t.Helper()
	symbols := make([]protocol.SymbolRef, len(cases))
	for k, c := range cases {
		symbols[k] = c.symbol
	}
	answer := mutantsOfMutable(t, symbols, limit)
	for k, c := range cases {
		got, descriptions := answer[k], []string{}
		for _, edit := range got.Edits {
			descriptions = append(descriptions, edit.Description)
			checkEdit(t, edit)
		}
		if c.edits == nil {
			c.edits = []string{}
		}
		if !reflect.DeepEqual(descriptions, c.edits) || got.Why != c.why {
			t.Errorf("%v: got %q, why %q; want %q, why %q", c.symbol, descriptions, got.Why, c.edits, c.why)
		}
	}
}

// checkEdit checks that an edit replaces what its description says, and leaves its file
// one Go parses.
func checkEdit(t *testing.T, edit protocol.MutantEdit) {
	t.Helper()
	old, _, changes := strings.Cut(edit.Description, " → ")
	if replaced := mutable[edit.Path][edit.Start:edit.End]; changes && replaced != old {
		t.Errorf("%q replaces %q", edit.Description, replaced)
	}
	if _, err := parser.ParseFile(token.NewFileSet(), edit.Path, applied(edit), 0); err != nil {
		t.Errorf("%q leaves %s unparsed: %v", edit.Description, edit.Path, err)
	}
}

// applied is the text of an edit's file with the edit made.
func applied(edit protocol.MutantEdit) string {
	text := mutable[edit.Path]
	return text[:edit.Start] + edit.Replacement + text[edit.End:]
}

func mutantsOfMutable(t *testing.T, symbols []protocol.SymbolRef, limit int) []protocol.SymbolMutants {
	t.Helper()
	surfaces := entries("default", ".", "shadow", "./shadow", "gone", "./no/such/package")
	project := read(t, Config{Root: written(t, mutable), InMemory: true, Surfaces: surfaces})
	answer, err := project.Mutants(symbols, limit)
	if err != nil {
		t.Fatal(err)
	}
	if len(answer) != len(symbols) {
		t.Fatalf("%d answers for %d symbols", len(answer), len(symbols))
	}
	return answer
}

// A surface may be a package of the module outside the project, which no edit of the
// project's files changes.
func TestMutantsAreOfTheProjectsOwnFiles(t *testing.T) {
	module := written(t, map[string]string{
		"go.mod":       "module example.com/outer\n\ngo 1.21\n",
		"lib/lib.go":   "package lib\n\nconst Outside = 1\n",
		"site/site.go": "package site\n\nconst Inside = 1\n",
	})
	surfaces := entries("lib", "../lib", "site", ".")
	project := read(t, Config{Root: filepath.Join(module, "site"), Surfaces: surfaces})
	answer, err := project.Mutants([]protocol.SymbolRef{{Name: "Outside", Surface: "lib"}, {Name: "Inside", Surface: "site"}}, 8)
	if err != nil {
		t.Fatal(err)
	}
	inside := []protocol.MutantEdit{{Path: "site.go", Start: 29, End: 30, Replacement: "2", Description: "1 → 2"}}
	if answer[0].Why != protocol.WhyNotFound || !reflect.DeepEqual(answer[1].Edits, inside) {
		t.Fatalf("got %+v", answer)
	}
}
