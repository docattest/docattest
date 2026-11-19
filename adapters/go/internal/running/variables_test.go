package running

import (
	"testing"

	"github.com/docattest/docattest/adapters/go/internal/protocol"
)

// literalVariables is a package main whose literal variables are of every shape a fact
// copies, with a type of its own, one of a package a generated program can import, and
// one of a package it cannot.
var literalVariables = map[string]string{
	"go.mod": "module example.com/tool\n\ngo 1.22\n",
	"cmd/tool/main.go": `package main

import (
	"fmt"
	"os"
	"time"

	"example.com/tool/cmd/tool/internal/level"
)

const listenPort = 8080

const letter = 'x'

// capability has a String method, which a copy does not have.
type capability struct {
	Name  string
	Level level.Level
}

func (c capability) String() string { return "a capability" }

type settings struct {
	Port  int
	Hosts []string
}

var sessionEnv = []string{"LANG=C.UTF-8", "TZ=UTC"}

var forbiddenCaps = []capability{{Name: "CAP_SYS_ADMIN", Level: 2}, {"CAP_NET_RAW", 1}}

var moreCaps = []capability{{Name: "CAP_CHOWN"}}

var limits = map[string]time.Duration{"read": 5 * time.Second}

var grid = [2][2]int{{1, 2}, {3, 4}}

var mixed = []any{1, "a", 2.5, time.Second, letter}

var config = settings{Port: listenPort, Hosts: []string{"a", "b"}}

var handlers = []func(){}

var reassigned = []string{"a"}

var appended = []string{"a"}

func main() {
	for _, c := range forbiddenCaps {
		fmt.Println(c, len(moreCaps), limits["read"], grid[0][1], len(mixed), config.Port, len(handlers))
	}
	fmt.Println(append(os.Environ(), sessionEnv...))
	reassigned = nil
	appended = append(appended, "b")
}
`,
	"cmd/tool/internal/level/level.go": "package level\n\ntype Level int\n\nfunc (l Level) String() string { return \"named\" }\n",
}

func TestAFactReadsLiteralVariablesFromSource(t *testing.T) {
	r := open(t, opening{files: literalVariables, surfaces: entries("default", "./cmd/tool"), noSetup: true})
	got := r.Evaluate([]protocol.Fact{
		fact("strings", "sessionEnv"),
		// A copied type prints as the struct it is: neither its String method nor that
		// of level.Level, which the program cannot import, is there.
		fact("structs", "forbiddenCaps"),
		fact("typeName", `fmt.Sprintf("%T", forbiddenCaps)`),
		fact("mapped", "func() (names []string) { for _, c := range forbiddenCaps { names = append(names, c.Name) }; return }()"),
		fact("twoOfOneType", "len(forbiddenCaps) + len(moreCaps)"),
		fact("importedType", "limits"),
		fact("array", "grid[1][0]"),
		fact("interfaces", `fmt.Sprintf("%T %T %T %T %T", mixed...)`),
		fact("nested", "config"),
		fact("shadowed", "func() int { sessionEnv := 3; return sessionEnv }()"),
	})
	expect(t, texts(got), map[string]string{
		"strings": "[LANG=C.UTF-8 TZ=UTC]", "structs": "[{CAP_SYS_ADMIN 2} {CAP_NET_RAW 1}]",
		"typeName": "[]main.capability", "mapped": "[CAP_SYS_ADMIN CAP_NET_RAW]", "twoOfOneType": "3",
		"importedType": "map[read:5s]", "array": "3", "interfaces": "int string float64 time.Duration int32",
		"nested": "{8080 [a b]}", "shadowed": "3",
	})
}

func TestAVariableThatIsNoLiteralVariableIsRefused(t *testing.T) {
	r := open(t, opening{files: literalVariables, surfaces: entries("default", "./cmd/tool"), noSetup: true})
	got := r.Evaluate([]protocol.Fact{
		fact("reassigned", "reassigned"),
		fact("appended", "appended"),
		fact("function", "handlers"),
		fact("fine", "len(sessionEnv)"),
	})
	expect(t, texts(got), map[string]string{
		"reassigned": "fact `reassigned`: reassigned is a variable of surface \"default\" that a fact cannot read from source: the package may change it (main.go:53 assigns to it)",
		"appended":   "fact `appended`: appended is a variable of surface \"default\" that a fact cannot read from source: the package may change it (main.go:54 assigns to it)",
		"function":   "fact `handlers`: handlers is a variable of surface \"default\" that a fact cannot read from source: its type holds func(), which a fact cannot copy",
		"fine":       "2",
	})
}
