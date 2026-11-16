// Package endtoend tests the Go package as it is used: its two commands, built, with the
// protocol's conformance suite and the real docattest core.
//
// The conformance suite is the core's, and the tests of the core need one: DOCATTEST_CORE,
// else core/target/release/docattest of this repository. Without one they are skipped, and
// say so.
package endtoend

import (
	"encoding/json"
	"os"
	"os/exec"
	"path/filepath"
	"reflect"
	"strings"
	"testing"
)

// repository is the root of the repository, from this package's directory.
const repository = "../../.."

var commands string

// TestMain builds the launcher and the adapter side by side, as `go install` leaves them.
func TestMain(m *testing.M) {
	dir, err := os.MkdirTemp("", "docattest-go-commands-")
	if err != nil {
		panic(err)
	}
	build := exec.Command("go", "build", "-o", dir+string(filepath.Separator), "../cmd/...")
	if out, err := build.CombinedOutput(); err != nil {
		panic("go build: " + string(out))
	}
	commands = dir
	code := m.Run()
	os.RemoveAll(dir)
	os.Exit(code)
}

func command(name string) string {
	return filepath.Join(commands, name)
}

// core is the docattest core the tests run: DOCATTEST_CORE, else the one built in this
// repository. A test that needs one is skipped when there is none.
func core(t *testing.T) string {
	t.Helper()
	path := os.Getenv("DOCATTEST_CORE")
	if path == "" {
		path, _ = filepath.Abs(filepath.Join(repository, "core", "target", "release", "docattest"))
	}
	if _, err := os.Stat(path); err != nil {
		t.Skip("no docattest core: set DOCATTEST_CORE, or build it (cargo build --release in core/)")
	}
	return path
}

func TestConformance(t *testing.T) {
	out, err := exec.Command(core(t), "conformance", "run", "../conformance.json", "--", command("docattest-adapter-go")).CombinedOutput()
	if err != nil || !strings.Contains(string(out), "conformance (go): ") || strings.Contains(string(out), "✗") {
		t.Fatalf("%v\n%s", err, out)
	}
	t.Log(strings.TrimSpace(string(out[strings.LastIndex(string(out), "conformance (go)"):])))
}

// docattest runs the launcher in a project, as a Go user runs docattest. Nothing says
// where the adapter is: the launcher finds it beside itself.
func docattest(t *testing.T, root string, args ...string) (stdout string, code int) {
	t.Helper()
	cmd := exec.Command(command("docattest"), append(args, "--root", root)...)
	cmd.Env = []string{"DOCATTEST_CORE=" + core(t), "PATH=" + os.Getenv("PATH"), "HOME=" + os.Getenv("HOME"),
		"GIT_CONFIG_GLOBAL=" + os.DevNull, "GIT_CONFIG_SYSTEM=" + os.DevNull}
	out, err := cmd.Output()
	exit, _ := err.(*exec.ExitError)
	if err != nil && exit == nil {
		t.Fatal(err)
	}
	if exit != nil && exit.ExitCode() == 2 {
		t.Fatalf("docattest %v: an internal error\n%s", args, exit.Stderr)
	}
	return string(out), cmd.ProcessState.ExitCode()
}

type verdict struct {
	Line   int
	Kind   string
	Status string
}

// statuses is what `check` says of each attestation of the fixture, page by page.
var statuses = map[string][]verdict{
	"docs/broken.md": {
		{5, "example", "fail"}, {17, "example", "fail"}, {23, "example", "fail"}, {31, "example", "fail"},
		{36, "example", "fail"}, {44, "example", "error"}, {48, "fact", "fail"}, {50, "fact", "fail"},
		{54, "fact", "error"}, {59, "cases", "fail"}, {60, "cases", "fail"}, {62, "cases", "error"}, {67, "test", "error"},
		{69, "test", "error"}, {74, "cases", "fail"}, {75, "cases", "fail"},
	},
	"docs/pricing.md": {
		{5, "example", "pass"}, {22, "example", "pass"}, {40, "example", "pass"}, {52, "example", "pass"},
		{62, "fact", "pass"}, {63, "fact", "pass"}, {65, "fact", "pass"}, {70, "fact", "pass"}, {77, "fact", "pass"},
		{82, "fact", "pass"}, {87, "fact", "pass"}, {92, "cases", "pass"}, {93, "cases", "pass"}, {94, "cases", "pass"},
		{96, "test", "pass"},
	},
	"docs/shipping.md": {
		{3, "fact", "pass"}, {10, "example", "pass"}, {21, "example", "skipped"}, {33, "cases", "pass"}, {34, "cases", "pass"},
	},
	"shop_test.go": {{10, "example", "pass"}, {17, "example", "pass"}},
}

func TestCheckTheFixture(t *testing.T) {
	out, code := docattest(t, "../testdata/fixture", "check", "--json")
	var report struct {
		Results []struct {
			Page string
			verdict
		}
	}
	if err := json.Unmarshal([]byte(out), &report); err != nil || code != 1 {
		t.Fatalf("exit %d, %v\n%s", code, err, out)
	}
	got := map[string][]verdict{}
	for _, result := range report.Results {
		got[result.Page] = append(got[result.Page], result.verdict)
	}
	if !reflect.DeepEqual(got, statuses) {
		t.Fatalf("got %v", got)
	}
}

func TestTheIndexAndTheAudit(t *testing.T) {
	if out, _ := docattest(t, "../testdata/fixture", "uncovered"); strings.TrimSpace(out) != "ErrorCodes" {
		t.Errorf("uncovered: %q", out)
	}
	if out, _ := docattest(t, "../testdata/fixture", "where", "Quote"); !strings.Contains(out, "docs/shipping.md:10 example") {
		t.Errorf("where: %q", out)
	}
	out, _ := docattest(t, "../testdata/fixture", "audit")
	for _, want := range []string{"L5 list matches Carriers", "L11 column drifts from ErrorCodes (missing: no_carrier)"} {
		if !strings.Contains(out, want) {
			t.Errorf("audit: no %q in %q", want, out)
		}
	}
	if out, _ := docattest(t, "../testdata/fixture", "inventory"); !strings.Contains(out, "@shipping Carriers  →  [post, courier, drone]") {
		t.Errorf("inventory: %q", out)
	}
}

func TestImpactOfAChange(t *testing.T) {
	root := t.TempDir()
	if err := os.CopyFS(root, os.DirFS("../testdata/fixture")); err != nil {
		t.Fatal(err)
	}
	git := func(args ...string) {
		cmd := exec.Command("git", append([]string{"-c", "user.name=t", "-c", "user.email=t@example.invalid"}, args...)...)
		cmd.Dir = root
		if out, err := cmd.CombinedOutput(); err != nil {
			t.Skipf("git %v: %v\n%s", args, err, out)
		}
	}
	git("init", "-q", "-b", "main")
	git("add", "-A")
	git("commit", "-q", "-m", "base")
	shop := filepath.Join(root, "shipping", "shipping.go")
	text, _ := os.ReadFile(shop)
	if err := os.WriteFile(shop, []byte(strings.Replace(string(text), `"DE": 2`, `"DE": 3`, 1)), 0o600); err != nil {
		t.Fatal(err)
	}
	out, _ := docattest(t, root, "impact")
	for _, want := range []string{"affect 1 exported symbol", "Quote", "docs/shipping.md:10 proven example"} {
		if !strings.Contains(out, want) {
			t.Errorf("impact: no %q in %q", want, out)
		}
	}
}
