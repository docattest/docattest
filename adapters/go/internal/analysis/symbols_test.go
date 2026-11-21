package analysis

import (
	"testing"

	"github.com/docattest/docattest/adapters/go/internal/protocol"
)

// shop is a module whose names a block and a test use through values, interfaces and
// local names.
var shop = map[string]string{
	"go.mod": "module example.com/shop\n\ngo 1.21\n",
	"shop.go": `package shop

type Money int

func (m Money) String() string { return "money" }

type Basket struct{ Total Money }

type Wallet struct{ Money }

func Fill() Wallet { return Wallet{} }

func New() Basket { return Basket{} }

type Store interface{ Save() error }

type Disk struct{}

func (Disk) Save() error { return nil }

func Open() Store { return Disk{} }

const Limit = 3
`,
	"shop_test.go": `package shop

import "testing"

func TestShadowed(t *testing.T) {
	Limit := 1
	_ = Limit
}

func runHelper(t *testing.T) {
	t.Helper()
	_ = Fill()
}

func TestViaHelper(t *testing.T) {
	runHelper(t)
}

func TestTableSubtests(t *testing.T) {
	cases := []struct {
		name string
		fn   func() Basket
	}{
		{name: "uses table", fn: New},
	}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			_ = tc.fn()
		})
	}
	t.Run("sibling", func(t *testing.T) {
		_ = Open()
	})
}
`,
	"tagged_test.go": `//go:build integration

package shop

import "testing"

func TestTagged(t *testing.T) { _ = Open() }
`,
	"export_test.go": `package shop

var Opened = Open
`,
	"external_test.go": `package shop_test

import (
	"testing"

	"example.com/shop"
)

func TestOutside(t *testing.T) { _ = shop.Opened().Save() }
`,
	"other_windows_test.go": `package shop

import "testing"

func TestElsewhere(t *testing.T) { _ = New() }
`,
	"wrap/wrap.go": `package wrap

import "example.com/shop"

func Basket() shop.Basket { return shop.New() }
`,
}

func TestSymbolsAreResolvedByType(t *testing.T) {
	p := readWritten(t, shop)
	block := func(code string) protocol.Attestation {
		return protocol.Attestation{Kind: protocol.KindExample, Page: "docs/p.md", Code: "import \"example.com/shop\"\n\n" + code}
	}
	cases := []struct {
		name string
		a    protocol.Attestation
		want string
	}{
		{"a method called on a value is its type's", block("_ = shop.New().Total.String()"), "default:New default:Basket default:Money"},
		{"a promoted method is the embedded type's", block("_ = shop.Fill().String()"), "default:Fill default:Money"},
		{"a method called through an interface is the interface's, not its implementation's",
			block("var s shop.Store = shop.Open()\n_ = s.Save()"), "default:Store default:Open"},
		{"a local name that shadows an import uses nothing of it",
			block("{\n\tshop := struct{ Limit int }{}\n\t_ = shop.Limit\n}\n_ = shop.New()"), "default:New"},
		{"a local name that shadows a name of the package uses nothing of it",
			protocol.Attestation{Kind: protocol.KindTest, File: "shop_test.go", Title: "TestShadowed"}, ""},
		{"a block names what it reaches through a package that is no surface",
			protocol.Attestation{Kind: protocol.KindExample, Page: "docs/p.md", Code: "import \"example.com/shop/wrap\"\n\n_ = wrap.Basket().Total.String()"},
			"default:Basket default:Money"},
		{"a block that reaches no surface names nothing",
			protocol.Attestation{Kind: protocol.KindExample, Page: "docs/p.md", Code: "import \"strings\"\n\n_ = strings.ToUpper(\"Limit\")"}, ""},
		{"a test behind a build tag uses what any test does",
			protocol.Attestation{Kind: protocol.KindTest, File: "tagged_test.go", Title: "TestTagged"}, "default:Open"},
		{"an external test sees the package with its own test files",
			protocol.Attestation{Kind: protocol.KindTest, File: "external_test.go", Title: "TestOutside"}, "default:Store"},
		{"a test file for another platform is not built",
			protocol.Attestation{Kind: protocol.KindTest, File: "other_windows_test.go", Title: "TestElsewhere"}, ""},
		{"a test follows local test helpers",
			protocol.Attestation{Kind: protocol.KindTest, File: "shop_test.go", Title: "TestViaHelper"}, "default:Fill"},
		{"a subtest follows the table it ranges over without leaking sibling subtests",
			protocol.Attestation{Kind: protocol.KindTest, File: "shop_test.go", Title: "uses table", Describes: []string{"TestTableSubtests"}}, "default:Basket default:New"},
		{"a sibling subtest does not inherit the table's symbols",
			protocol.Attestation{Kind: protocol.KindTest, File: "shop_test.go", Title: "sibling", Describes: []string{"TestTableSubtests"}}, "default:Open"},
	}
	for _, c := range cases {
		found, err := symbolsOf(p, c.a)
		if got := names(found); err != nil || got != c.want {
			t.Errorf("%s: got %q, %v", c.name, got, err)
		}
	}
}
