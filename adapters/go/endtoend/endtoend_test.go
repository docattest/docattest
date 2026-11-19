// Package endtoend tests the Go package as it is used: its two commands, built, with the
// protocol's conformance suite and the real docattest core.
//
// The conformance suite is the core's, and the tests of the core need one: DOCATTEST_CORE,
// else core/target/release/docattest of this repository. Without one they are skipped, and
// say so.
package endtoend

import (
	"encoding/json"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"reflect"
	"slices"
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
	"docs/sandbox.md": {
		{3, "fact", "pass"}, {7, "fact", "pass"}, {11, "fact", "pass"}, {13, "fact", "pass"}, {17, "fact", "pass"},
		{19, "fact", "pass"}, {23, "fact", "pass"}, {25, "fact", "pass"}, {27, "fact", "pass"},
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

func TestAVetFindingInADocsHelperFailsTheCheck(t *testing.T) {
	root := t.TempDir()
	if err := os.CopyFS(root, os.DirFS("../testdata/fixture")); err != nil {
		t.Fatal(err)
	}
	helper := "//go:build docattest\n\npackage sandbox\n\nimport \"fmt\"\n\n" +
		"func FirstCap() string { return fmt.Sprintf(\"%d\", forbiddenCaps[0].Name) }\n"
	if err := os.WriteFile(filepath.Join(root, "sandbox", "vetted_docattest.go"), []byte(helper), 0o600); err != nil {
		t.Fatal(err)
	}
	out, code := docattest(t, root, "check", "docs/sandbox.md", "--json")
	var report struct {
		Results []struct {
			Page, Message string
			verdict
		}
	}
	if err := json.Unmarshal([]byte(out), &report); err != nil || code != 1 {
		t.Fatalf("exit %d, %v\n%s", code, err, out)
	}
	failed := []string{}
	for _, result := range report.Results {
		if result.Status != "pass" {
			failed = append(failed, fmt.Sprintf("%s:%d %s %s: %s", result.Page, result.Line, result.Kind, result.Status, result.Message))
		}
	}
	want := []string{"sandbox/vetted_docattest.go:7 problem fail: go vet -tags docattest: " +
		"fmt.Sprintf format %d has arg forbiddenCaps[0].Name of wrong type string"}
	if !reflect.DeepEqual(failed, want) {
		t.Fatalf("got %q", failed)
	}
}

func TestTheIndexAndTheAudit(t *testing.T) {
	// ErrorCodes is read by a helper of the setup package that a fact calls, Shell and
	// MaxOpenFiles only by the values that facts read, and the sandbox's docs helpers are
	// no exports.
	if out, _ := docattest(t, "../testdata/fixture", "uncovered"); strings.TrimSpace(out) != "every public symbol is attested" {
		t.Errorf("uncovered: %q", out)
	}
	// forbiddenCaps is read by the docs helper that the fact at L13 calls.
	where := map[string]string{"Quote": "docs/shipping.md:10 example", "forbiddenCaps": "docs/sandbox.md:13 fact",
		"Shell": "docs/sandbox.md:25 fact", "MaxOpenFiles": "docs/sandbox.md:27 fact"}
	for symbol, want := range where {
		if out, _ := docattest(t, "../testdata/fixture", "where", symbol); !strings.Contains(out, want) {
			t.Errorf("where %s: %q", symbol, out)
		}
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
	root := committedFixture(t)
	edit(t, root, "shipping/shipping.go", `"DE": 2`, `"DE": 3`)
	out, _ := docattest(t, root, "impact")
	// days is a literal variable, which a fact could read from source.
	for _, want := range []string{"affect 2 symbols", "Quote", "docs/shipping.md:10 proven example", "\ndays\n"} {
		if !strings.Contains(out, want) {
			t.Errorf("impact: no %q in %q", want, out)
		}
	}
}

func TestImpactOfAConstantReadFromSource(t *testing.T) {
	root := committedFixture(t)
	page := "Refused: `no carrier serves \"XX\"`. <!-- fact @shipping: fmt.Sprintf(unserved, \"XX\") -->\n\n" +
		"The port is `8080`. <!-- fact @shopd: listenPort -->\n"
	if err := os.WriteFile(filepath.Join(root, "docs", "constants.md"), []byte(page), 0o600); err != nil {
		t.Fatal(err)
	}
	edit(t, root, "shipping/shipping.go", "serves %q", "delivers to %q")
	edit(t, root, "cmd/shopd/main.go", "listenPort = 8080", "listenPort = 8081")
	out, _ := docattest(t, root, "impact", "--json")
	var impact struct {
		Symbols    []string
		References []struct{ Symbol, Page, Kind string }
	}
	if err := json.Unmarshal([]byte(out), &impact); err != nil {
		t.Fatalf("%v\n%s", err, out)
	}
	facts := []string{}
	for _, r := range impact.References {
		if r.Page == "docs/constants.md" {
			facts = append(facts, r.Symbol+" "+r.Kind)
		}
	}
	if got := strings.Join(facts, ", "); got != "listenPort fact, unserved fact" {
		t.Errorf("got %q in %s", got, out)
	}
}

func TestAFactThroughAHelperAttestsWhatTheHelperUses(t *testing.T) {
	root := committedFixture(t)
	if out, _ := docattest(t, root, "where", "ErrorCodes"); !strings.Contains(out, "docs/pricing.md:77 fact") {
		t.Errorf("where: %q", out)
	}
	edit(t, root, "shop.go", "No carrier serves", "Nobody serves")
	edit(t, root, "sandbox/sandbox.go", "forges packets", "fakes packets")
	out, _ := docattest(t, root, "impact", "--json")
	got := references(t, out)
	// The fact at L13 calls a docs helper, which reads forbiddenCaps; L17 and L19 read it,
	// and L23 calls Refuses, which reads it.
	for _, want := range []string{"ErrorCodes docs/pricing.md:77", "ForbiddenCapNames docs/sandbox.md:13",
		"forbiddenCaps docs/sandbox.md:13", "forbiddenCaps docs/sandbox.md:17", "forbiddenCaps docs/sandbox.md:19",
		"Refuses docs/sandbox.md:23"} {
		if !slices.Contains(got, want) {
			t.Errorf("no %q in %s", want, out)
		}
	}
}

func TestImpactOfANameAValueIsBuiltFrom(t *testing.T) {
	root := committedFixture(t)
	edit(t, root, "sandbox/sandbox.go", `Shell = "/bin/sh"`, `Shell = "/bin/bash"`)
	edit(t, root, "sandbox/sandbox.go", "MaxOpenFiles = 64", "MaxOpenFiles = 128")
	out, _ := docattest(t, root, "impact", "--json")
	got := references(t, out)
	// The fact at L25 reads ShellEnv, built from Shell; the one at L27 reads limits, built
	// from MaxOpenFiles.
	for _, want := range []string{"Shell docs/sandbox.md:25", "ShellEnv docs/sandbox.md:25",
		"MaxOpenFiles docs/sandbox.md:27", "limits docs/sandbox.md:27"} {
		if !slices.Contains(got, want) {
			t.Errorf("no %q in %s", want, out)
		}
	}
}

// references are the references of `impact --json`, each as its symbol, its page and its
// line.
func references(t *testing.T, out string) []string {
	t.Helper()
	var impact struct {
		References []struct {
			Symbol, Page string
			Line         int
		}
	}
	if err := json.Unmarshal([]byte(out), &impact); err != nil {
		t.Fatalf("%v\n%s", err, out)
	}
	found := []string{}
	for _, r := range impact.References {
		found = append(found, fmt.Sprintf("%s %s:%d", r.Symbol, r.Page, r.Line))
	}
	return found
}

// committedFixture is a copy of the fixture, committed to a git repository of its own.
func committedFixture(t *testing.T) string {
	t.Helper()
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
	return root
}

// edit replaces the first `was` in a file of the project with `now`.
func edit(t *testing.T, root, file, was, now string) {
	t.Helper()
	path := filepath.Join(root, file)
	text, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(path, []byte(strings.Replace(string(text), was, now, 1)), 0o600); err != nil {
		t.Fatal(err)
	}
}
