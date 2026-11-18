package analysis

import (
	"strings"
	"testing"
)

func TestAffectedFollowsWhatTheBaseImported(t *testing.T) {
	// At the base, b's file imported a, whose Helper is gone; now it imports c under the
	// same name, so Uses is the same code but reaches what changed only at the base.
	now := "package b\n\nimport h \"example.com/m/c\"\n\nfunc Uses() int { return h.Helper() }\n"
	was := strings.Replace(now, "example.com/m/c", "example.com/m/a", 1)
	p := read(t, Config{Root: written(t, map[string]string{
		"go.mod": "module example.com/m\n\ngo 1.21\n",
		"a/a.go": "package a\n\nfunc Other() int { return 0 }\n",
		"b/b.go": now,
		"c/c.go": "package c\n\nfunc Helper() int { return 1 }\n",
	}), Surfaces: entries("b", "./b")})
	aWas := "package a\n\nfunc Helper() int { return 1 }\n"
	found, err := p.Affected(map[string]*string{"b/b.go": &was, "a/a.go": &aWas})
	if got := names(found); err != nil || got != "b:Uses" {
		t.Errorf("got %q, %v", got, err)
	}
}

func TestAffectedFollowsWhatNamesResolveTo(t *testing.T) {
	code := `package m

func helper() int { return 1 }

func Uses() int { return helper() }

func Shadows() int {
	helper := 2
	return helper
}

type Store interface{ Save() error }

type disk struct{}

func (disk) Save() error { return nil }

func Open() Store { return disk{} }

func Run(s Store) error { return s.Save() }
`
	p := readWritten(t, map[string]string{"go.mod": "module example.com/m\n\ngo 1.21\n", "m.go": code})
	cases := []struct{ name, now, was, want string }{
		{"a local name is not the function it shadows", "return 1", "return 0", "default:Uses"},
		// Run calls Save through Store, and is not affected; Open makes a disk, and is.
		{"a method called through an interface is the interface's", "error { return nil }", "error { panic(0) }", "default:Open"},
	}
	for _, c := range cases {
		was := strings.Replace(code, c.now, c.was, 1)
		found, err := p.Affected(map[string]*string{"m.go": &was})
		if got := names(found); err != nil || got != c.want {
			t.Errorf("%s: got %q, %v", c.name, got, err)
		}
	}
}
