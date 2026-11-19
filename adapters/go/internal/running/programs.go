package running

import (
	"bufio"
	"bytes"
	"encoding/json"
	"errors"
	"fmt"
	"io/fs"
	"os"
	"path/filepath"
	"strings"
	"time"

	"github.com/docattest/docattest/adapters/go/internal/harness"
	"github.com/docattest/docattest/adapters/go/internal/toolchain"
)

// A generated program serves items (fact expressions, the rows of a table, what a
// surface offers) one by one: internal/harness/program is the part of it that is the
// same every time, and a main the session writes is the rest.

// built is a generated program as far as it could be built.
type built struct {
	binary string
	// dropped are the pieces the compiler complained of, with the complaint, by index.
	dropped map[int]string
	// failure is why there is no program at all, when it is nobody's piece's fault.
	failure string
}

// buildProgram writes and builds a program whose main is made of labelled pieces of
// attested code, with the packages' docs helper files. `render` writes the main for the
// pieces still in it. A piece the
// compiler complains of is taken out, with the complaint, and the rest is built again:
// one wrong fact does not cost a page its other facts.
func (r *Runner) buildProgram(labels []string, render func(live []bool) string) built {
	name := fmt.Sprintf("p%d", r.serial.take())
	files := map[string]string{}
	for _, file := range harness.Items {
		files[file] = harness.Source(file, "main")
	}
	live := make([]bool, len(labels))
	for k := range live {
		live[k] = true
	}
	out := built{dropped: map[int]string{}}
	bin, err := r.work("bin")
	if err != nil {
		out.failure = err.Error()
		return out
	}
	for len(out.dropped) < len(labels) || len(labels) == 0 {
		files["main.go"] = render(live)
		if err := r.place(name, files); err != nil {
			out.failure = err.Error()
			return out
		}
		problems := r.gotool.BuildWithDocs(bin, []string{filepath.Join(blocksDir, name)})
		if len(problems) == 0 {
			out.binary = filepath.Join(bin, toolchain.Executable(name))
			return out
		}
		if !drop(labels, live, problems, out.dropped) {
			out.failure = problems[0].Message
			return out
		}
	}
	return out
}

// drop takes out the pieces a build complained of; false when it complained of none.
func drop(labels []string, live []bool, problems []toolchain.Problem, dropped map[int]string) bool {
	any := false
	for _, p := range problems {
		for k, label := range labels {
			if live[k] && filepath.Base(p.File) == label {
				live[k], dropped[k], any = false, p.Message, true
			}
		}
	}
	return any
}

// line is one line of a program's results (the harness's docattestLine).
type line struct {
	Item    int             `json:"item"`
	Value   json.RawMessage `json:"value"`
	Panic   string          `json:"panic"`
	Timeout bool            `json:"timeout"`
	Failed  string          `json:"failed"`
	// Lost says why an item has no answer: its program ended first.
	Lost string `json:"lost"`
}

// job is what a generated program is told (the harness's docattestJob).
type job struct {
	TimeoutMs float64         `json:"timeoutMs"`
	Snapshots Snapshots       `json:"snapshots"`
	Manifests json.RawMessage `json:"manifests"`
	Texts     json.RawMessage `json:"texts"`
	Matches   json.RawMessage `json:"matches,omitempty"`
	Fields    json.RawMessage `json:"manifestFields,omitempty"`
	Cases     *casesJob       `json:"cases,omitempty"`
}

// serve runs a program until each of its `count` items has an answer, and returns the
// answers in order; `failed` is why the program served none. A program ends at an item
// that runs out of time or ends the process, and is started again at the item after it.
func (r *Runner) serve(binary string, cases *casesJob, count int) (answers []line, failed string) {
	options := r.options
	told := job{TimeoutMs: options.TimeoutMs, Snapshots: options.Snapshots, Manifests: options.Manifests, Texts: options.Texts,
		Matches: options.Matches, Fields: options.Fields, Cases: cases}
	jobFile, results, err := r.writeJob(told)
	if err != nil {
		return nil, err.Error()
	}
	for len(answers) < count {
		release := r.running()
		outcome := toolchain.Run(toolchain.Command{
			Path:    binary,
			Args:    []string{jobFile, results, fmt.Sprint(len(answers))},
			Dir:     r.project.Root,
			Timeout: time.Duration(count-len(answers))*r.timeout() + programStartup,
		})
		release()
		before := len(answers)
		if answers, err = readLines(results); err != nil {
			return nil, err.Error()
		}
		if len(answers) >= count || (len(answers) > before && answers[len(answers)-1].Timeout) {
			continue
		}
		answers = append(answers, line{Item: len(answers), Lost: lost(outcome)})
		if err := appendLost(results, answers[len(answers)-1]); err != nil {
			return nil, err.Error()
		}
	}
	return answers, ""
}

// programStartup is the time a generated program is given to start: beyond its items'
// limits for one that serves items, and before its time limit counts for a block.
const programStartup = 10 * time.Second

// writeJob writes what a program is told to a file of its own, and returns that file and
// the one its results go to.
func (r *Runner) writeJob(told job) (jobFile, results string, err error) {
	dir, err := r.work("jobs", fmt.Sprintf("j%d", r.serial.take()))
	if err != nil {
		return "", "", err
	}
	text, err := json.Marshal(told)
	if err != nil {
		return "", "", fmt.Errorf("cannot write a program's job: %w", err)
	}
	jobFile, results = filepath.Join(dir, "job.json"), filepath.Join(dir, "results")
	if err := writeFile(jobFile, string(text)); err != nil {
		return "", "", fmt.Errorf("cannot write a program's job: %w", err)
	}
	return jobFile, results, nil
}

// lost says how a program ended that did not answer an item.
func lost(outcome toolchain.Outcome) string {
	if outcome.TimedOut || outcome.Err != nil {
		return "the program did not answer and was stopped"
	}
	return crash(outcome, "", 1)
}

// appendLost notes an item without an answer in the results, so that the next reading
// of them still has one line for each item.
func appendLost(results string, lostItem line) error {
	text, err := json.Marshal(map[string]any{"item": lostItem.Item, "lost": lostItem.Lost})
	if err == nil {
		err = appendLine(results, text)
	}
	if err != nil {
		return fmt.Errorf("cannot write to a program's results: %w", err)
	}
	return nil
}

func appendLine(path string, text []byte) error {
	file, err := os.OpenFile(path, os.O_APPEND|os.O_CREATE|os.O_WRONLY, 0o600)
	if err != nil {
		return err
	}
	_, err = file.Write(append(text, '\n'))
	if closed := file.Close(); err == nil {
		err = closed
	}
	return err
}

// maxResultLine bounds one line of a program's results: one value, however large.
const maxResultLine = 1 << 28

// readLines reads a program's results so far; none when it wrote none. A line that
// says the program could serve nothing is an error with its reason.
func readLines(results string) ([]line, error) {
	text, err := os.ReadFile(results)
	if errors.Is(err, fs.ErrNotExist) {
		return nil, nil
	}
	if err != nil {
		return nil, fmt.Errorf("cannot read a program's results: %w", err)
	}
	lines := []line{}
	scanner := bufio.NewScanner(bytes.NewReader(text))
	scanner.Buffer(make([]byte, 0, 1<<16), maxResultLine)
	for scanner.Scan() {
		var read line
		if err := json.Unmarshal(scanner.Bytes(), &read); err != nil {
			return nil, fmt.Errorf("a program's results are not JSON (line %d): %w", len(lines)+1, err)
		}
		if read.Failed != "" {
			return nil, errors.New(read.Failed)
		}
		lines = append(lines, read)
	}
	if err := scanner.Err(); err != nil {
		return nil, fmt.Errorf("cannot read a program's results: %w", err)
	}
	return lines, nil
}

func writeFile(path, text string) error {
	return os.WriteFile(path, []byte(text), 0o600)
}

// standardPackage says whether a name is a package of the standard library that is
// imported by that name alone (`strings`, `math`, not `path/filepath`).
func (r *Runner) standardPackage(name string) bool {
	if name == "" || strings.ContainsAny(name, `/\.`) {
		return false
	}
	info, err := os.Stat(filepath.Join(r.gotool.Env("GOROOT"), "src", name))
	return err == nil && info.IsDir()
}
