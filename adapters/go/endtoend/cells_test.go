package endtoend

import (
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/docattest/docattest/adapters/go/cells"
)

// cellsAsWritten are cells as a page writes them, each with at least one code span: a cell
// with none is compared by its text, which CodeSpans does not give.
var cellsAsWritten = []string{
	"`FOO=1` `BAR=2`",
	"`fmt.Println(1)`",
	"``a`b``",
	"`` `x` ``",
	"```a``b```",
	"`  padded  `",
	"`  `x",
	"text `a` more `b` end",
	"`a\\|b`",
	"\\`x` `y`",
	"\\\\`x`",
	"`x\\`y`",
	"``a` `b`",
	"`a`` `b`",
	"\t`\u00a0tab and no-break\u00a0` ",
	"`\ufeffbyte order mark\u0085`",
	"`a` <!-- `b` --> `c`",
	"<span title=\"`\">`x`</span>",
	"<https://example.com/`a`> `b`",
	"<a@b.c> `d`",
	"a < b `c` > d",
	"**`bold`** _`em`_",
	"[`a`](b) `c`",
	"[a](`b`) `c`",
	"[a](u \"`t`\") `c`",
	"[a](u '`t`') `c`",
	"[a](u (`t`)) `c`",
	"[a](<`b` c>) `d`",
	"[a](b(`c`)) `d`",
	"[a] (`b`)",
	"a](`b`)",
	"\\[a](`b`)",
	"[[a](b)](`c`) `z`",
	"![[a](b)](`c`) `z`",
	"[![a](b)](`c`) `z`",
	"[a [b] c](`d`) `z`",
	"[a][`b`]",
	"[a`](b)` `c`",
	"[a](b `c`) `d`",
	"<!-->`a`",
	"<?x `a` ?> `b`",
	"<!X `a`> `b`",
	"<![CDATA[`a`]]> `b`",
	"</a `b`> `c`",
}

// TestCodeSpansAreWhatAColumnFactCompares holds CodeSpans to the core: each cell is the one
// row of a table whose column fact lists what CodeSpans gives for the cell as a runner gets
// it, so a fact that fails is a cell the two read differently.
func TestCodeSpansAreWhatAColumnFactCompares(t *testing.T) {
	root := t.TempDir()
	page := strings.Builder{}
	spans := make([][]string, len(cellsAsWritten))
	for i, cell := range cellsAsWritten {
		spans[i] = cells.CodeSpans(strings.ReplaceAll(cell, `\|`, "|"))
		fmt.Fprintf(&page, "<!-- fact column=cell: %#v -->\n| cell |\n|---|\n| %s |\n\n", spans[i], cell)
	}
	files := map[string]string{
		"go.mod":                "module example.com/cells\n\ngo 1.23\n",
		"cells.go":              "package cells\n",
		"cells.md":              page.String(),
		"docattest.config.json": `{"pages": ["cells.md"], "surfaces": {"default": "."}, "adapter": "go"}`,
	}
	for name, text := range files {
		if err := os.WriteFile(filepath.Join(root, name), []byte(text), 0o600); err != nil {
			t.Fatal(err)
		}
	}
	out, _ := docattest(t, root, "check", "--json")
	var report struct {
		Results []struct{ Status, Message string }
	}
	if err := json.Unmarshal([]byte(out), &report); err != nil || len(report.Results) != len(cellsAsWritten) {
		t.Fatalf("%v\n%s", err, out)
	}
	for i, result := range report.Results {
		if result.Status != "pass" {
			t.Errorf("%s: CodeSpans gives %q; %s: %s", cellsAsWritten[i], spans[i], result.Status, result.Message)
		}
	}
}

// TestCellsImportsOnlyTheStandardLibrary keeps a module that imports the package light: what
// it imports is all the go command fetches for it, and the cores are reached only through
// the launcher.
func TestCellsImportsOnlyTheStandardLibrary(t *testing.T) {
	out := run(t, "..", nil, "go", "list", "-deps", "-f", "{{if not .Standard}}{{.ImportPath}}{{end}}", "./cells")
	if got := strings.TrimSpace(out); got != "github.com/docattest/docattest/adapters/go/cells" {
		t.Fatalf("cells imports %s", got)
	}
}
