package cells

import (
	"encoding/json"
	"os"
	"reflect"
	"testing"
)

// sharedCases is the table every language's cell helper and the core are tested against.
const sharedCases = "../../../protocol/test-data/code-spans.json"

func TestCodeSpans(t *testing.T) {
	text, err := os.ReadFile(sharedCases)
	if err != nil {
		t.Fatal(err)
	}
	var table struct {
		Cases []struct {
			Name  string
			Cell  string
			Spans []string
		}
	}
	if err := json.Unmarshal(text, &table); err != nil {
		t.Fatal(err)
	}
	for _, test := range table.Cases {
		t.Run(test.Name, func(t *testing.T) {
			got := CodeSpans(test.Cell)
			if len(got) == 0 && len(test.Spans) == 0 {
				return
			}
			if !reflect.DeepEqual(got, test.Spans) {
				t.Errorf("CodeSpans(%q) = %q, want %q", test.Cell, got, test.Spans)
			}
		})
	}
}
