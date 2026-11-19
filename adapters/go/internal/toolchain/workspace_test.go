package toolchain

import (
	"os"
	"path/filepath"
	"reflect"
	"runtime"
	"strings"
	"testing"

	"golang.org/x/mod/modfile"
)

func TestAProgramGivenAnEnvironmentKeepsItsDirectory(t *testing.T) {
	if runtime.GOOS == "windows" {
		t.Skip("uses sh")
	}
	dir := t.TempDir()
	done := Run(Command{Path: "sh", Args: []string{"-c", `echo "$ADDED $PWD"`}, Dir: dir, Env: []string{"ADDED=yes"}})
	if got := strings.TrimSpace(done.Stdout); got != "yes "+dir {
		t.Fatalf("got %q", got)
	}
}

func TestAWorkspaceIsWrittenWhereItIsAsked(t *testing.T) {
	// GOWORK=off leaves the user's go.work out, which the go command would otherwise write to.
	t.Setenv("GOWORK", "off")
	root := t.TempDir()
	module := filepath.Join(root, "m")
	write(t, filepath.Join(module, "go.mod"), "module example.com/m\n\ngo 1.21\n")
	write(t, filepath.Join(root, "go.work"), "go 1.21\n\nuse ./m\n")
	work := filepath.Join(root, "adapter")
	if err := os.Mkdir(work, 0o700); err != nil {
		t.Fatal(err)
	}
	workspace, err := NewWorkspace(work, []string{module})
	if err != nil {
		t.Fatal(err)
	}
	text, err := os.ReadFile(string(workspace))
	if err != nil || !strings.Contains(string(text), "use "+module) {
		t.Fatalf("got %q, %v", text, err)
	}
	if got := workspace.Environment(); len(got) != 1 || got[0] != "GOWORK="+filepath.Join(work, "go.work") {
		t.Fatalf("got %q", got)
	}
}

func TestAWorkspaceKeepsTheUsersOwn(t *testing.T) {
	root := t.TempDir()
	write(t, filepath.Join(root, "go.mod"), "module example.com/shop\n\ngo 1.21\n")
	write(t, filepath.Join(root, "money", "go.mod"), "module example.com/money\n\ngo 1.21\n")
	write(t, filepath.Join(root, "docs", "go.mod"), "module example.com/shop/docs\n\ngo 1.23\n")
	users := "go 1.21\n\nuse (\n\t.\n\t./money\n)\n\nreplace example.com/x => ./x\n"
	write(t, filepath.Join(root, "go.work"), users)
	workspace, err := NewWorkspace(t.TempDir(), []string{root, filepath.Join(root, "docs")})
	if err != nil {
		t.Fatal(err)
	}
	text, err := os.ReadFile(string(workspace))
	if err != nil {
		t.Fatal(err)
	}
	parsed, err := modfile.ParseWork("go.work", text, nil)
	if err != nil {
		t.Fatal(err)
	}
	used := []string{}
	for _, use := range parsed.Use {
		used = append(used, use.Path)
	}
	// The go command sorts what a workspace uses.
	want := []string{root, filepath.Join(root, "docs"), filepath.Join(root, "money")}
	if !reflect.DeepEqual(used, want) || parsed.Go.Version != "1.23" {
		t.Fatalf("uses %q at go %s", used, parsed.Go.Version)
	}
	if len(parsed.Replace) != 1 || parsed.Replace[0].New.Path != filepath.Join(root, "x") {
		t.Fatalf("got %s", text)
	}
	if after, _ := os.ReadFile(filepath.Join(root, "go.work")); string(after) != users {
		t.Fatalf("the user's go.work was written: %s", after)
	}
}

func write(t *testing.T, path, text string) {
	t.Helper()
	if err := os.MkdirAll(filepath.Dir(path), 0o700); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(path, []byte(text), 0o600); err != nil {
		t.Fatal(err)
	}
}
