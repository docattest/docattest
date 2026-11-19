// Package session is the one project `open` creates: it reads what `open` says, hands
// the project to the analysis, which loads and type-checks it, and to the runner, which
// builds and runs its attested code, and answers each method from them.
package session

import (
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"time"

	"github.com/docattest/docattest/adapters/go/internal/analysis"
	"github.com/docattest/docattest/adapters/go/internal/citedtests"
	"github.com/docattest/docattest/adapters/go/internal/protocol"
	"github.com/docattest/docattest/adapters/go/internal/running"
)

// params are what `open` says.
type params struct {
	Root      string                       `json:"root"`
	Files     map[string]string            `json:"files"`
	Surfaces  []protocol.ConfiguredSurface `json:"surfaces"`
	Setup     string                       `json:"setup"`
	TimeoutMs float64                      `json:"timeoutMs"`
	Workers   float64                      `json:"workers"`
	Network   bool                         `json:"network"`
	Snapshots running.Snapshots            `json:"snapshots"`
	Manifests json.RawMessage              `json:"manifests"`
	Texts     json.RawMessage              `json:"texts"`
	Matches   json.RawMessage              `json:"matches"`
	Fields    json.RawMessage              `json:"manifestFields"`
	Examples  []protocol.Example           `json:"examples"`
}

// Session is a project, for one docattest invocation.
type Session struct {
	project *analysis.Project
	runner  *running.Runner
	// runTests runs cited tests: on disk, or not at all when the project is in memory.
	runTests func(tests []protocol.CitedTest, timeout time.Duration) []protocol.TestRun
	// temp holds everything the session writes, and is removed when it closes.
	temp string
}

// Open creates the session of a process.
func Open(raw json.RawMessage) (protocol.Session, error) {
	told, err := protocol.Decode[params](raw)
	if err != nil {
		return nil, err
	}
	temp, err := os.MkdirTemp("", "docattest-go-")
	if err == nil {
		temp, err = filepath.EvalSymlinks(temp)
	}
	if err != nil {
		return nil, fmt.Errorf("no temporary directory: %w", err)
	}
	s, err := open(told, temp)
	if err != nil {
		_ = os.RemoveAll(temp)
		return nil, err
	}
	return s, nil
}

func open(told params, temp string) (*Session, error) {
	root := told.Root
	if told.Files != nil {
		var err error
		if root, err = materialise(filepath.Join(temp, "project"), told.Files); err != nil {
			return nil, err
		}
	}
	config := analysis.Config{
		Root: root, InMemory: told.Files != nil, Surfaces: told.Surfaces, Setup: told.Setup, Work: temp,
	}
	project, err := analysis.Read(config)
	if err != nil {
		return nil, err
	}
	options := running.Options{
		Work:      temp,
		TimeoutMs: told.TimeoutMs,
		Workers:   int(told.Workers),
		Network:   told.Network,
		Snapshots: told.Snapshots,
		Manifests: told.Manifests,
		Texts:     told.Texts,
		Matches:   told.Matches,
		Fields:    told.Fields,
	}
	runTests := citedtests.InMemory
	if told.Files == nil {
		runTests = citedtests.Runner{Root: root, Workspace: project.Workspace, Workers: int(told.Workers)}.Run
	}
	return &Session{
		project: project, runner: running.New(project, options, told.Examples), runTests: runTests, temp: temp,
	}, nil
}

// materialise writes a project that exists only in memory to a directory, since the go
// command reads files, and returns that directory.
func materialise(root string, files map[string]string) (string, error) {
	for rel, text := range files {
		path := filepath.Join(root, filepath.FromSlash(rel))
		if err := os.MkdirAll(filepath.Dir(path), 0o700); err != nil {
			return "", fmt.Errorf("cannot write the project: %w", err)
		}
		if err := os.WriteFile(path, []byte(text), 0o600); err != nil {
			return "", fmt.Errorf("cannot write the project: %w", err)
		}
	}
	if err := os.MkdirAll(root, 0o700); err != nil {
		return "", fmt.Errorf("cannot write the project: %w", err)
	}
	return root, nil
}

// Close removes what the session wrote, once nothing it started is still going. Nothing
// reads the directory again, so a failure to remove it is no one's to handle.
func (s *Session) Close() {
	s.runner.Wait()
	_ = os.RemoveAll(s.temp)
}
