package language

import (
	"reflect"
	"strings"
	"testing"

	"github.com/docattest/docattest/adapters/go/internal/protocol"
)

func TestProgramForms(t *testing.T) {
	cases := []struct {
		name, code string
		// wants are pieces the compiled file must hold, in order.
		wants []string
	}{
		{"statements become the body of main", "import \"fmt\"\n\nfmt.Println(1)", []string{"import \"fmt\"", "func main() {", "//line L:1\n\n\nfmt.Println(1)", "}"}},
		{"declarations are given an empty main", "func double(n int) int { return 2 * n }", []string{"//line L:1", "func double", "func main() {}"}},
		{"declarations with a main are left alone", "func main() { println(1) }", []string{"//line L:1\nfunc main() { println(1) }\n"}},
		{"a whole file is used as it is", "package main\n\nfunc main() {}\n", []string{"//line L:1\npackage main"}},
	}
	for _, c := range cases {
		t.Run(c.name, func(t *testing.T) {
			form := Program(c.code, "L")
			rest := form.Source
			for _, want := range c.wants {
				at := strings.Index(rest, want)
				if at < 0 {
					t.Fatalf("no %q (in order) in:\n%s", want, form.Source)
				}
				rest = rest[at+len(want):]
			}
			if form.Main == nil {
				t.Fatalf("no main in:\n%s", form.Source)
			}
		})
	}
	if strings.Count(Program("func main() {}", "L").Source, "func main") != 1 {
		t.Fatal("a block with a main was given a second one")
	}
}

func TestProgramKeepsTheFormThatParsesFurthest(t *testing.T) {
	// The mistake is on line 3 of statements; read as declarations it would be on line 1.
	form := Program("x := 1\n_ = x\ny := := 2", "L")
	if !strings.Contains(form.Source, "func main() {\n//line L:1\nx := 1") {
		t.Fatalf("not read as statements:\n%s", form.Source)
	}
}

func TestOutput(t *testing.T) {
	read := func(code string) (string, bool, bool) {
		form := Program(code, "L")
		return Output(form.Fset, form.File, form.Main)
	}
	if text, unordered, ok := read("println(1)\n// Output: 1"); text != "1\n" || unordered || !ok {
		t.Fatalf("got %q %v %v", text, unordered, ok)
	}
	if text, unordered, ok := read("println(1)\n// Unordered output:\n// b\n// a"); text != "b\na\n" || !unordered || !ok {
		t.Fatalf("got %q %v %v", text, unordered, ok)
	}
	if _, _, ok := read("// Output: 1\nprintln(1)\n// a remark"); ok {
		t.Fatal("only the last comment of the body states the output")
	}
}

func TestAsserts(t *testing.T) {
	asserting := []string{
		"println(1)\n// Output: 1",
		"if 1 > 2 {\n\tpanic(\"no\")\n}",
		"import \"log\"\n\nif 1 > 2 {\n\tlog.Fatalf(\"no\")\n}",
		"import \"os\"\n\nif 1 > 2 {\n\tos.Exit(1)\n}",
		"this is not Go at all {{{",
	}
	for _, code := range asserting {
		if !Asserts(code) {
			t.Errorf("should assert:\n%s", code)
		}
	}
	for _, code := range []string{"println(1)", "import \"fmt\"\n\nfmt.Println(1)\n// prints 1"} {
		if Asserts(code) {
			t.Errorf("asserts nothing:\n%s", code)
		}
	}
}

const testFile = `package kit

import "fmt"

// Adding gives the <!-- a note --> sum,
// whatever the order.
func ExampleAdd() {
	fmt.Println(Add(1, 2))
	// Output: 3
}

func ExampleClient_Get() {}

func Example_whole() {}

func Examples() {}

func ExampleAdd_params(n int) {}
`

func TestDocExamples(t *testing.T) {
	found := docExamplesOf(protocol.SourceText{Page: "kit/kit_test.go", Text: testFile})
	got := [][]string{}
	for _, e := range found {
		got = append(got, []string{e.ID, e.Mode, e.Owner, e.Claim})
	}
	want := [][]string{
		{"kit/kit_test.go#ExampleAdd", "check", "Add", "Adding gives the sum, whatever the order."},
		{"kit/kit_test.go#ExampleClient_Get", "compile", "Client", ""},
		{"kit/kit_test.go#Example_whole", "compile", "", ""},
	}
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("got  %v\nwant %v", got, want)
	}
	if found[0].Line != 7 || !strings.HasPrefix(found[0].Code, "func ExampleAdd() {") {
		t.Fatalf("got line %d, code %q", found[0].Line, found[0].Code)
	}
	for _, page := range []string{"kit/kit.go", "docs/notes.txt"} {
		if len(docExamplesOf(protocol.SourceText{Page: page, Text: testFile})) != 0 {
			t.Errorf("%s is not a test file and has no examples", page)
		}
	}
}

func TestDocProse(t *testing.T) {
	prose := docProseOf(protocol.SourceText{Page: "a.go", Text: "// Package a is small.\npackage a\n\n// N is\n// a number.\nconst N = 1\n\nvar (\n\t// M is another.\n\tM = 2\n)\n"})
	want := []protocol.DocProse{
		{Text: "Package a is small.", Start: 1, End: 1},
		{Text: "N is a number.", Start: 4, End: 5},
		{Text: "M is another.", Start: 9, End: 9},
	}
	if !reflect.DeepEqual(prose, want) {
		t.Fatalf("got %v", prose)
	}
	if len(docProseOf(protocol.SourceText{Page: "a.txt", Text: "// x\npackage a"})) != 0 {
		t.Fatal("a file that is not Go has no doc comments")
	}
}

func TestMentionsAndShape(t *testing.T) {
	named := mentioned([]string{"kit.Add(1, 2)", "Added", "add", "x.Keys[0]"}, []string{"Add", "Keys"})
	got := []string{}
	for _, name := range named {
		if name == nil {
			got = append(got, "-")
		} else {
			got = append(got, *name)
		}
	}
	if want := []string{"Add", "-", "-", "Keys"}; !reflect.DeepEqual(got, want) {
		t.Fatalf("got %v", got)
	}
	one, renamed, other := shapeOf(`Limits["small"]`), shapeOf(`Bounds [ "small" ]`), shapeOf(`Limits["small"] + 1`)
	if one.Shape != renamed.Shape || one.Shape == other.Shape {
		t.Fatalf("shapes: %q %q %q", one.Shape, renamed.Shape, other.Shape)
	}
	if !reflect.DeepEqual(one.Names, []string{"Limits"}) || !reflect.DeepEqual(renamed.Names, []string{"Bounds"}) {
		t.Fatalf("names: %v %v", one.Names, renamed.Names)
	}
	if !reflect.DeepEqual(Qualifiers("strings.ToUpper(a.b.c) + x"), []string{"strings", "a"}) {
		t.Fatalf("qualifiers: %v", Qualifiers("strings.ToUpper(a.b.c) + x"))
	}
}
