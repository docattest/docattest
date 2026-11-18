// Package running builds and runs attested code: the blocks of pages, the testable
// examples of source files, and the programs it generates for facts, cases tables and
// what a surface offers. What it knows of the project it asks the analysis.
package running

import (
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"sync"
	"time"

	"github.com/docattest/docattest/adapters/go/internal/analysis"
	"github.com/docattest/docattest/adapters/go/internal/protocol"
	"github.com/docattest/docattest/adapters/go/internal/toolchain"
)

// Options are what `open` says of how attested code runs.
type Options struct {
	// Work is a directory of the runner's own; it removes nothing.
	Work      string
	TimeoutMs float64
	Workers   int
	Network   bool
	Snapshots Snapshots
	// Manifests are the data of each manifest the facts name, by the path as they write
	// it, as `open` gives them and generated programs are told them.
	Manifests json.RawMessage
	// Texts are the text of each file the facts read with `file`, by the path as they
	// write it, as `open` gives them and generated programs are told them.
	Texts json.RawMessage
}

// Snapshots are the project's snapshots, as `open` gives them and generated programs
// are told them.
type Snapshots struct {
	Dir  string          `json:"dir"`
	Data json.RawMessage `json:"data"`
}

// defaultTimeoutMs is the time a program of attested code may take when `open` gives no
// limit.
const defaultTimeoutMs = 10000

// Runner builds and runs the attested code of one project.
type Runner struct {
	project *analysis.Project
	options Options
	gotool  *toolchain.Go
	// slots bounds how many programs of attested code run at once.
	slots  chan struct{}
	blocks blocks
	tests  testPrograms
	cases  struct {
		once    sync.Once
		program built
	}
	serial serial
}

// New is the runner of a project. Building the blocks of `examples` starts at once,
// since building is most of the time a check takes: `compile` and `run` find them
// built, or wait for them.
func New(project *analysis.Project, options Options, examples []protocol.Example) *Runner {
	options.Workers = max(options.Workers, 1)
	if options.TimeoutMs <= 0 {
		options.TimeoutMs = defaultTimeoutMs
	}
	r := &Runner{
		project: project,
		options: options,
		gotool:  &toolchain.Go{Module: project.Module.Dir, Work: options.Work},
		slots:   make(chan struct{}, options.Workers),
	}
	r.blocks.start(r, examples)
	return r
}

// Wait returns once nothing the runner started in the background is still going.
func (r *Runner) Wait() { r.blocks.wait() }

// serial numbers the things a runner generates.
type serial struct {
	lock sync.Mutex
	next int
}

func (s *serial) take() int {
	s.lock.Lock()
	defer s.lock.Unlock()
	s.next++
	return s.next
}

func (r *Runner) timeout() time.Duration {
	return time.Duration(r.options.TimeoutMs * float64(time.Millisecond))
}

// timeoutText is the time limit as a message writes it.
func (r *Runner) timeoutText() string {
	return fmt.Sprintf("timed out after %v ms", r.options.TimeoutMs)
}

// work makes a directory of the runner's own.
func (r *Runner) work(parts ...string) (string, error) {
	dir := filepath.Join(append([]string{r.options.Work}, parts...)...)
	if err := os.MkdirAll(dir, 0o700); err != nil {
		return "", fmt.Errorf("cannot make a working directory: %w", err)
	}
	return dir, nil
}

// running takes one of the runner's slots for as long as a program of attested code
// runs, and returns the function that gives it back.
func (r *Runner) running() func() {
	r.slots <- struct{}{}
	return func() { <-r.slots }
}
