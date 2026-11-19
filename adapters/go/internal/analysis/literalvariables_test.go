package analysis

import (
	"fmt"
	"testing"
)

// literals is a package main whose variables are read, changed and declared in every way
// the rule of literal variables tells apart.
const literals = `package main

import (
	"fmt"
	"os"
	"sort"
	"time"
)

type capability struct {
	Name  string
	Level int
}

const port = 8080

var (
	env       = []string{"LANG=C", "TZ=UTC"}
	caps      = []capability{{Name: "CAP_SYS_ADMIN", Level: 2}, {"CAP_NET_RAW", port}}
	limits    = map[string]time.Duration{"read": 5 * time.Second}
	grid      = [2][2]int{{1, 2}, {3, 4}}
	settings  = capability{Name: "x"}
	nested    = map[string][]string{"a": {"b"}}

	started   = time.Now()
	declared  []string
	computed  = []string{os.Getenv("HOME")}
	pointers  = []*capability{{Name: "x"}}

	reassigned = []string{"a"}
	appended   = []string{"a"}
	indexed    = []string{"a"}
	addressed  = capability{Name: "x"}
	shared     = []string{"b", "a"}
	aliased    = map[string]int{"a": 1}
	sharedRange = [][]string{{"a"}}
	incremented = [1]int{1}
	pointerCall = capability{Name: "x"}
)

func (c *capability) raise() { c.Level++ }

func (c capability) String() string { return c.Name }

func main() {
	for _, c := range caps {
		fmt.Println(c.Name, c.String(), len(env), grid[1][0], settings)
	}
	_ = append(os.Environ(), env...)
	copy(make([]string, 1), env)
	for key, _ := range nested {
		fmt.Println(key)
	}
	if d, ok := limits["read"]; ok {
		fmt.Println(d, started, declared, computed, pointers, nested["a"][0])
	}

	reassigned = nil
	appended = append(appended, "b")
	indexed[0] = "b"
	fmt.Println(&addressed)
	sort.Strings(shared)
	more := aliased
	more["b"] = 2
	for _, row := range sharedRange {
		row[0] = "b"
	}
	incremented[0]++
	pointerCall.raise()
}
`

func TestALiteralVariableIsOneItsPackageOnlyReads(t *testing.T) {
	p := read(t, Config{Root: written(t, map[string]string{"go.mod": "module example.com/m\n\ngo 1.22\n", "main.go": literals}), InMemory: true, Surfaces: entries("default", ".")})
	of := p.SurfaceNamed("default")
	cases := map[string]string{
		"env": "", "caps": "", "limits": "", "grid": "", "settings": "", "nested": "",
		"started":     "its value, time.Now(), is not a composite literal",
		"declared":    "it is declared without a value",
		"computed":    "os.Getenv(\"HOME\") in its value is not a constant (main.go:27)",
		"pointers":    "its value holds a pointer (main.go:28)",
		"reassigned":  "the package may change it (main.go:58 assigns to it)",
		"appended":    "the package may change it (main.go:59 assigns to it)",
		"indexed":     "the package may change it (main.go:60 assigns to it)",
		"addressed":   "the package may change it (main.go:61 takes its address)",
		"shared":      "the package may change it (main.go:62 lets its storage be shared)",
		"aliased":     "the package may change it (main.go:63 lets its storage be shared)",
		"sharedRange": "the package may change it (main.go:65 ranges over it with items that share its storage)",
		"incremented": "the package may change it (main.go:68 assigns to it)",
		"pointerCall": "the package may change it (main.go:69 takes its address to call a method)",
	}
	for name, want := range cases {
		_, err := p.LiteralVariable(of, name)
		if got := fmt.Sprint(err); (want == "" && err != nil) || (want != "" && got != want) {
			t.Errorf("%s: got %v, want %q", name, err, want)
		}
	}
}
