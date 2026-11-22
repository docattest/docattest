package running

import (
	"strings"
	"testing"

	"github.com/docattest/docattest/adapters/go/internal/protocol"
)

// withShopd opens the fixture with its package main, cmd/shopd, as a surface.
func withShopd(t *testing.T) *Runner {
	return open(t, opening{surfaces: entries("default", ".", "shipping", "./shipping", "shopd", "./cmd/shopd")})
}

func factOn(surface, id, expression string) protocol.Fact {
	read := fact(id, expression)
	read.Surface = surface
	return read
}

// texts are what each fact showed, by id, or why it has no value.
func texts(got map[string]protocol.Evaluation) map[string]string {
	out := map[string]string{}
	for id, evaluation := range got {
		out[id] = evaluation.Message
		if evaluation.OK {
			out[id] = evaluation.Value.Text
		}
	}
	return out
}

func expect(t *testing.T, got, want map[string]string) {
	t.Helper()
	for id, text := range want {
		if got[id] != text {
			t.Errorf("%s: got %q, want %q", id, got[id], text)
		}
	}
}

func TestAFactReadsConstantsFromSource(t *testing.T) {
	got := withShopd(t).Evaluate([]protocol.Fact{
		factOn("shopd", "port", "listenPort"),
		factOn("shopd", "sum", "listenPort + 1"),
		factOn("shopd", "typed", "requestTimeout"),
		factOn("shopd", "method", "requestTimeout.Seconds()"),
		factOn("shopd", "standard", "strings.ToUpper(region)"),
		factOn("shopd", "mainType", "logLevel"),
		factOn("shipping", "unexported", `fmt.Sprintf(unserved, "XX")`),
		factOn("shipping", "mixed", `strings.Count(unserved, "%") + len(Carriers)`),
		fact("plain", "MaxDiscount"),
	})
	expect(t, texts(got), map[string]string{
		"port": "8080", "sum": "8081", "typed": "30s", "method": "30", "standard": "NL",
		// A type of a package main cannot be named elsewhere: the value is its underlying
		// int's, and verbosity's String method is not used.
		"mainType":   "2",
		"unexported": `no carrier serves "XX"`, "mixed": "4", "plain": "50",
	})
}

func TestOnlyConstantsAndLiteralVariablesAreReadFromSource(t *testing.T) {
	got := withShopd(t).Evaluate([]protocol.Fact{
		factOn("shopd", "variable", "started"),
		factOn("shopd", "function", "main"),
		factOn("shopd", "type", "verbosity(1)"),
		factOn("shipping", "literal", `days["NL"]`),
		fact("private", "round(50)"),
		factOn("shopd", "fine", "listenPort"),
	})
	expect(t, texts(got), map[string]string{
		"variable": "fact `started`: started is a variable of surface \"shopd\" that a fact cannot read from source: its value, time.Now(), is not a composite literal",
		"function": "fact `main`: main is a function of surface \"shopd\", a package main; of a package main, a fact reads only constants and literal variables",
		"type":     "fact `verbosity(1)`: verbosity is a type of surface \"shopd\", a package main; of a package main, a fact reads only constants and literal variables",
		"literal":  "1",
		"private":  "fact `round(50)`: round is a function that surface \"default\" does not export; of its unexported names, a fact reads only constants and literal variables",
		"fine":     "8080",
	})
}

// kinds is a package main whose constants are of every kind, typed by packages a
// generated program can import and by one it cannot.
var kinds = map[string]string{
	"go.mod": "module example.com/tool\n\ngo 1.21\n",
	"cmd/tool/main.go": `package main

import (
	"example.com/tool/cmd/tool/internal/level"
	"example.com/tool/internal/units"
)

const third = 1.0 / 3
const letter = 'a'
const huge = 1 << 100
const wave = 2i
const limit units.Size = 2048
const start level.Level = 1

func main() {}
`,
	"internal/units/units.go":          "package units\n\nimport \"fmt\"\n\ntype Size int\n\nfunc (s Size) String() string { return fmt.Sprintf(\"%d KiB\", s/1024) }\n",
	"cmd/tool/internal/level/level.go": "package level\n\ntype Level int\n\nfunc (l Level) String() string { return \"named\" }\n",
}

func TestAConstantKeepsItsKindAndItsType(t *testing.T) {
	r := open(t, opening{files: kinds, surfaces: entries("default", "./cmd/tool"), noSetup: true})
	got := r.Evaluate([]protocol.Fact{
		fact("float", "third * 3"), fact("floatType", `fmt.Sprintf("%T", third)`),
		fact("rune", `fmt.Sprintf("%T %c", letter, letter)`), fact("huge", "huge >> 98"),
		fact("complex", "imag(wave)"),
		// A type in internal/ at the module's root can be imported, and prints as it does;
		// one deeper in cannot, and the value is its underlying int's.
		fact("visible", "limit"), fact("hidden", "start"),
	})
	expect(t, texts(got), map[string]string{
		"float": "1", "floatType": "float64", "rune": "int32 a", "huge": "4", "complex": "2",
		"visible": "2 KiB", "hidden": "1",
	})
}

func TestNothingOfThePackageRuns(t *testing.T) {
	files := map[string]string{
		"go.mod":  "module example.com/m\n\ngo 1.21\n",
		"main.go": "package main\n\nconst port = 8080\n\nfunc init() { panic(\"the package ran\") }\n\nfunc main() {}\n",
	}
	r := open(t, opening{files: files, surfaces: entries("default", "."), noSetup: true})
	expect(t, texts(r.Evaluate([]protocol.Fact{fact("port", "port")})), map[string]string{"port": "8080"})
}

func TestAPackageThatNamesAStandardPackageKeepsIt(t *testing.T) {
	files := map[string]string{
		"go.mod":  "module example.com/m\n\ngo 1.21\n",
		"main.go": "package main\n\nvar path = \"/v1\"\n\nconst strings = 3\n\nfunc main() { _ = path }\n",
	}
	r := open(t, opening{files: files, surfaces: entries("default", "."), noSetup: true})
	got := r.Evaluate([]protocol.Fact{fact("package", `path.Join("a", strings.ToUpper("b"))`)})
	expect(t, texts(got), map[string]string{"package": "a/B"})
}

func TestAPackageThatDoesNotCompileFailsOnlyTheFactsThatReadIt(t *testing.T) {
	files := map[string]string{
		"go.mod":  "module example.com/m\n\ngo 1.21\n",
		"main.go": "package main\n\nconst port = 8080\n\nfunc main() { undefinedName() }\n",
	}
	r := open(t, opening{files: files, surfaces: entries("default", "."), noSetup: true})
	got := r.Evaluate([]protocol.Fact{fact("reads", "port"), fact("alone", "1 + 1")})
	if got["reads"].OK || !strings.Contains(got["reads"].Message, `surface "default" failed to load: `) ||
		!strings.Contains(got["reads"].Message, "undefinedName") {
		t.Errorf("got %+v", got["reads"])
	}
	expect(t, texts(got), map[string]string{"alone": "2"})
}

func TestAPackageMainOffersItsConstantsAndLiteralVariables(t *testing.T) {
	files := map[string]string{
		"go.mod":  "module example.com/m\n\ngo 1.21\n",
		"main.go": "package main\n\nimport \"time\"\n\nconst Version = \"1.4.0\"\n\nconst Timeout = 5 * time.Second\n\nvar Started = time.Now()\n\nconst port = 8080\n\nfunc main() {}\n",
	}
	r := open(t, opening{files: files, surfaces: entries("default", "."), noSetup: true})
	found, err := r.Observables("default")
	if err != nil {
		t.Fatal(err)
	}
	got := []string{}
	for _, o := range found {
		got = append(got, o.Expression+"="+o.Value.Text)
	}
	if strings.Join(got, " ") != "Version=1.4.0 Timeout=5s port=8080" {
		t.Fatalf("got %v", got)
	}
}
