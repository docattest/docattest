package running

import (
	"fmt"
	"regexp"
	"sort"
	"strings"
	"sync"

	"github.com/docattest/docattest/adapters/go/internal/language"
	"github.com/docattest/docattest/adapters/go/internal/protocol"
	"github.com/docattest/docattest/adapters/go/internal/toolchain"
)

// networkSkipped is the verdict on a block tagged `network` when the network is off.
const networkSkipped = "network example skipped: the network is off (set DOCATTEST_NETWORK=1 to run it)"

// verdict is what running something proved.
type verdict struct {
	Status           protocol.Status
	Message          string
	Expected, Actual string
}

func pass(message string) verdict { return verdict{Status: protocol.StatusPass, Message: message} }
func fail(message string) verdict { return verdict{Status: protocol.StatusFail, Message: message} }

// result is a verdict as the protocol's Result, at a line of a page.
func (v verdict) result(page string, line int, kind protocol.Kind) protocol.Result {
	return protocol.Result{
		Page: page, Line: line, Kind: kind, Status: v.Status, Symbols: []string{},
		Expected: v.Expected, Actual: v.Actual, Message: v.Message,
	}
}

// Run answers `run`: a result for each example, in order, then for each row of each
// table. Everything runs at once, as far as the runner's slots allow.
func (r *Runner) Run(examples []protocol.Example, tables []protocol.Cases) []protocol.Result {
	built := r.blocks.ensure(r, examples)
	results := make([][]protocol.Result, len(examples)+len(tables))
	var all sync.WaitGroup
	for k, example := range examples {
		all.Add(1)
		go func() {
			defer all.Done()
			found := r.runExample(example, built[example.ID])
			results[k] = []protocol.Result{found.result(example.Page, example.Line, protocol.KindExample)}
		}()
	}
	for k, table := range tables {
		all.Add(1)
		go func() {
			defer all.Done()
			results[len(examples)+k] = r.runCases(table)
		}()
	}
	all.Wait()
	out := []protocol.Result{}
	for _, group := range results {
		out = append(out, group...)
	}
	return out
}

func (r *Runner) runExample(example protocol.Example, made *block) verdict {
	switch {
	case example.Network && !r.options.Network:
		return verdict{Status: protocol.StatusSkipped, Message: networkSkipped}
	case language.IsSource(example.Page):
		return r.runDocExample(example)
	case made == nil:
		return fail("the block was not built")
	case len(made.problems) > 0:
		first := made.problems[0]
		return fail(fmt.Sprintf("does not compile: %s (block line %d)", first.Message, max(first.Line, 1)))
	case made.binary == "":
		return fail("the block builds no program: a block that is a whole file must be package main")
	}
	return r.runBlock(made)
}

// runBlock runs a block's program in the project root and judges how it ended.
func (r *Runner) runBlock(made *block) verdict {
	release := r.running()
	outcome := toolchain.Run(toolchain.Command{Path: made.binary, Dir: r.project.Root, Timeout: r.timeout()})
	release()
	if ended := r.ended(outcome, made.label, 1); ended != nil {
		return *ended
	}
	wanted, unordered, states := language.Output(made.form.Fset, made.form.File, made.form.Main)
	switch {
	case states:
		return printed(outcome.Stdout, wanted, unordered)
	case language.Asserts(made.example.Code):
		return pass("ran to its end without panicking")
	}
	return pass("ran to its end, but asserts nothing (weak)")
}

// ended is the verdict on a program that did not run to its end, or nil when it did.
// `label` names the attested code in a stack trace, where its first line is `first`.
func (r *Runner) ended(outcome toolchain.Outcome, label string, first int) *verdict {
	var found verdict
	switch {
	case outcome.Err != nil:
		found = fail(fmt.Sprintf("could not be run: %v", outcome.Err))
	case outcome.TimedOut:
		found = fail(r.timeoutText())
	case outcome.Code != 0:
		found = fail(crash(outcome, label, first))
	default:
		return nil
	}
	return &found
}

var fatal = regexp.MustCompile(`(?m)^(panic: .*|fatal error: .*)$`)

// crash says why a program ended with a failure: its panic and the line of the attested
// code the panic passed through, else the last thing it wrote to its standard error.
func crash(outcome toolchain.Outcome, label string, first int) string {
	output := outcome.Stderr + outcome.Stdout
	if found := fatal.FindString(output); found != "" {
		message := strings.TrimSuffix(strings.TrimSpace(found), " [recovered]")
		frame := regexp.MustCompile(`(?m)^\s+(?:\S*[/\\])?` + regexp.QuoteMeta(label) + `:(\d+)`)
		if at := frame.FindStringSubmatch(output); at != nil {
			var line int
			_, _ = fmt.Sscanf(at[1], "%d", &line)
			message += fmt.Sprintf(" (block line %d)", line-first+1)
		}
		return message
	}
	lines := strings.Split(strings.TrimSpace(outcome.Stderr), "\n")
	if last := strings.TrimSpace(lines[len(lines)-1]); last != "" {
		return fmt.Sprintf("exited with status %d: %s", outcome.Code, last)
	}
	return fmt.Sprintf("exited with status %d", outcome.Code)
}

// printed compares what a program printed with what its Output comment says, as
// `go test` compares an example's: surrounding space is trimmed, and lines are sorted
// when the comment says the output is unordered.
func printed(got, wanted string, unordered bool) verdict {
	got, wanted = strings.TrimSpace(got), strings.TrimSpace(wanted)
	if unordered {
		got, wanted = sortedLines(got), sortedLines(wanted)
	}
	if got == wanted {
		return pass("printed the output it states")
	}
	return differs(wanted, got)
}

func differs(wanted, got string) verdict {
	return verdict{
		Status:   protocol.StatusFail,
		Message:  fmt.Sprintf("expected output %q, got %q", wanted, got),
		Expected: wanted,
		Actual:   got,
	}
}

func sortedLines(text string) string {
	lines := strings.Split(text, "\n")
	sort.Strings(lines)
	return strings.Join(lines, "\n")
}
