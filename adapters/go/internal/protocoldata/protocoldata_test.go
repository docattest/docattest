package protocoldata

import (
	"bytes"
	"io/fs"
	"os"
	"path/filepath"
	"testing"
)

// source is the repository's protocol/ directory, from this package's directory.
const source = "../../../../protocol"

func TestTheCopyIsTheSource(t *testing.T) {
	if _, err := os.Stat(filepath.Join(source, "core.json")); err != nil {
		t.Skip("not in a checkout of the repository: there is no source to compare with")
	}
	copied := 0
	err := fs.WalkDir(Files, "protocol", func(path string, entry fs.DirEntry, err error) error {
		if err != nil || entry.IsDir() {
			return err
		}
		copied++
		mine, _ := Files.ReadFile(path)
		rel, _ := filepath.Rel("protocol", path)
		theirs, err := os.ReadFile(filepath.Join(source, rel))
		if err != nil || !bytes.Equal(mine, theirs) {
			t.Errorf("%s differs from protocol/%s: run node protocol/sync.ts adapters/go/internal/protocoldata/protocol", path, rel)
		}
		return nil
	})
	if err != nil || copied < 6 {
		t.Fatalf("walked %d files: %v", copied, err)
	}
	for _, dir := range []string{"schema", "agents"} {
		theirs, _ := filepath.Glob(filepath.Join(source, dir, "*.json"))
		mine, _ := fs.Glob(Files, "protocol/"+dir+"/*.json")
		if len(theirs) != len(mine) {
			t.Errorf("protocol/%s has %d files, the copy %d", dir, len(theirs), len(mine))
		}
	}
}

func TestPin(t *testing.T) {
	if pin := Core(); pin.Version == "" || pin.Protocol != 1 {
		t.Fatalf("got %+v", pin)
	}
	if known := Platforms(); known["linux-x64"].Binary != "docattest" || known["windows-x64"].Binary != "docattest.exe" {
		t.Fatalf("got %v", known)
	}
}
