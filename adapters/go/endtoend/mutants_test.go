package endtoend

import (
	"bufio"
	"encoding/json"
	"fmt"
	"os/exec"
	"reflect"
	"slices"
	"strings"
	"testing"

	"github.com/docattest/docattest/adapters/go/internal/protocoldata"
)

// The adapter lists `mutants` in its greeting and answers it for a project that exists
// only in memory, as the core asks it.
func TestTheAdapterAnswersMutants(t *testing.T) {
	files := map[string]string{
		"go.mod":    "module example.com/served\n\ngo 1.21\n",
		"served.go": "// Package served is the café's.\npackage served\n\nconst Port = 8080\n",
	}
	open, _ := json.Marshal(map[string]any{
		"root": "/in/memory", "files": files, "surfaces": []map[string]any{{"name": "default", "entry": "."}},
	})
	answers := converse(t,
		fmt.Sprintf(`{"id":1,"method":"hello","params":{"protocol":%d}}`, protocoldata.Core().Protocol),
		fmt.Sprintf(`{"id":2,"method":"open","params":%s}`, open),
		`{"id":3,"method":"mutants","params":{"symbols":[{"name":"Port","surface":"default"},{"name":"Missing"}],"limit":8}}`,
		`{"id":4,"method":"close","params":{}}`,
	)
	var hello struct{ Capabilities []string }
	if err := json.Unmarshal(answers[1], &hello); err != nil || !slices.Contains(hello.Capabilities, "mutants") {
		t.Fatalf("hello: %s (%v)", answers[1], err)
	}
	var got []map[string]any
	if err := json.Unmarshal(answers[3], &got); err != nil {
		t.Fatalf("mutants: %s (%v)", answers[3], err)
	}
	want := []map[string]any{
		{"edits": []any{map[string]any{"path": "served.go", "start": 63.0, "end": 67.0, "replacement": "8081", "description": "8080 → 8081"}}},
		{"edits": []any{}, "why": "notFound"},
	}
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("mutants: %s", answers[3])
	}
	if text := files["served.go"]; text[63:67] != "8080" {
		t.Fatalf("the offsets are not those of 8080 in %q", text)
	}
}

// converse sends requests to the adapter, one per line, and returns each result by its id.
func converse(t *testing.T, requests ...string) map[int]json.RawMessage {
	t.Helper()
	adapter := exec.Command(command("docattest-adapter-go"))
	adapter.Stdin = strings.NewReader(strings.Join(requests, "\n") + "\n")
	out, err := adapter.Output()
	if err != nil {
		t.Fatalf("%v\n%s", err, out)
	}
	results := map[int]json.RawMessage{}
	lines := bufio.NewScanner(strings.NewReader(string(out)))
	lines.Buffer(nil, 1<<24)
	for lines.Scan() {
		var answer struct {
			ID     int
			Result json.RawMessage
			Error  json.RawMessage
		}
		if err := json.Unmarshal(lines.Bytes(), &answer); err != nil || answer.Error != nil {
			t.Fatalf("answer %s (%v)", lines.Text(), err)
		}
		results[answer.ID] = answer.Result
	}
	return results
}
