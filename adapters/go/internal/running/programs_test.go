package running

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestReadLines(t *testing.T) {
	dir := t.TempDir()
	if lines, err := readLines(filepath.Join(dir, "none")); err != nil || len(lines) != 0 {
		t.Errorf("a program that wrote nothing has no results: got %v, %v", lines, err)
	}
	results := filepath.Join(dir, "results")
	write := func(text string) {
		if err := os.WriteFile(results, []byte(text), 0o600); err != nil {
			t.Fatal(err)
		}
	}
	write(`{"item":0,"value":1}` + "\n" + `{"item":1,"panic":"no"}` + "\n")
	if lines, err := readLines(results); err != nil || len(lines) != 2 || lines[1].Panic != "no" {
		t.Errorf("got %+v, %v", lines, err)
	}
	write(`{"item":0,"value":1}` + "\n" + `{"item":1,` + "\n")
	if _, err := readLines(results); err == nil || !strings.Contains(err.Error(), "line 2") {
		t.Errorf("a line that is not JSON is an error: got %v", err)
	}
	write(`{"item":0,"failed":"no setup"}` + "\n")
	if _, err := readLines(results); err == nil || err.Error() != "no setup" {
		t.Errorf("got %v", err)
	}
}
