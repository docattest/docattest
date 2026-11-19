package session

import (
	"encoding/json"
	"os"
	"path/filepath"
	"testing"
)

var project = map[string]string{
	"go.mod":      "module example.com/tiny\n\ngo 1.21\n",
	"a_test.go":   "package tiny\n\nimport \"testing\"\n\nfunc TestPasses(t *testing.T) {}\n",
	"page.md":     "# Tiny\n",
	"doc.go":      "// Package tiny is tiny.\npackage tiny\n",
	"b/b_test.go": "package b\n\nimport \"testing\"\n\nfunc TestFails(t *testing.T) { t.Fatal(\"b is wrong\") }\n",
}

const runTests = `{"tests": [
	{"file": "a_test.go", "describes": [], "title": "TestPasses"},
	{"file": "b/b_test.go", "describes": [], "title": "TestFails"}
], "timeoutMs": 60000}`

func TestRunTestsAnswersATestRunForEachTest(t *testing.T) {
	root := t.TempDir()
	for rel, text := range project {
		path := filepath.Join(root, filepath.FromSlash(rel))
		if err := os.MkdirAll(filepath.Dir(path), 0o700); err != nil {
			t.Fatal(err)
		}
		if err := os.WriteFile(path, []byte(text), 0o600); err != nil {
			t.Fatal(err)
		}
	}
	opened, _ := json.Marshal(map[string]any{"root": root, "surfaces": []any{}, "workers": 2})
	got := answer(t, opened)
	want := `[{"status":"passed"},{"status":"failed","message":"b_test.go:5: b is wrong"}]`
	if got != want {
		t.Fatalf("got %s", got)
	}
}

func TestRunTestsRunsNothingInMemory(t *testing.T) {
	opened, _ := json.Marshal(map[string]any{"root": "/nowhere", "files": project, "surfaces": []any{}})
	got := answer(t, opened)
	notRun := `{"status":"notRun","message":"the project is in memory, where no test can run"}`
	if want := "[" + notRun + "," + notRun + "]"; got != want {
		t.Fatalf("got %s", got)
	}
}

// answer opens a session and is its answer to runTests, as the protocol writes it.
func answer(t *testing.T, opened []byte) string {
	t.Helper()
	s, err := Open(opened)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(s.Close)
	result, err := s.Methods()["runTests"](json.RawMessage(runTests))
	if err != nil {
		t.Fatal(err)
	}
	written, err := json.Marshal(result)
	if err != nil {
		t.Fatal(err)
	}
	return string(written)
}
