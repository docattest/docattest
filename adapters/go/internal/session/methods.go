package session

import (
	"encoding/json"

	"github.com/docattest/docattest/adapters/go/internal/protocol"
)

// Methods are the session's methods, by name.
func (s *Session) Methods() map[string]protocol.Handler {
	return map[string]protocol.Handler{
		"compile":          s.onCompile,
		"evaluate":         s.onEvaluate,
		"run":              s.onRun,
		"symbols":          s.onSymbols,
		"exports":          s.onExports,
		"describeSurfaces": func(json.RawMessage) (any, error) { return s.project.DescribeSurfaces(), nil },
		"tests":            s.onTests,
		"observables":      s.onObservables,
		"affected":         s.onAffected,
		"inCode":           s.onInCode,
	}
}

type surfaceParams struct {
	Surface string `json:"surface"`
}

func (s *Session) onCompile(raw json.RawMessage) (any, error) {
	params, err := protocol.Decode[struct {
		Examples []protocol.Example `json:"examples"`
	}](raw)
	if err != nil {
		return nil, err
	}
	return s.runner.Compile(params.Examples), nil
}

func (s *Session) onEvaluate(raw json.RawMessage) (any, error) {
	params, err := protocol.Decode[struct {
		Facts []protocol.Fact `json:"facts"`
	}](raw)
	if err != nil {
		return nil, err
	}
	return s.runner.Evaluate(params.Facts), nil
}

func (s *Session) onRun(raw json.RawMessage) (any, error) {
	params, err := protocol.Decode[struct {
		Examples []protocol.Example `json:"examples"`
		Cases    []protocol.Cases   `json:"cases"`
	}](raw)
	if err != nil {
		return nil, err
	}
	return s.runner.Run(params.Examples, params.Cases), nil
}

func (s *Session) onSymbols(raw json.RawMessage) (any, error) {
	params, err := protocol.Decode[struct {
		Attestations []protocol.Attestation `json:"attestations"`
	}](raw)
	if err != nil {
		return nil, err
	}
	return s.project.Symbols(params.Attestations)
}

func (s *Session) onExports(raw json.RawMessage) (any, error) {
	params, err := protocol.Decode[surfaceParams](raw)
	if err != nil {
		return nil, err
	}
	if names, ok := s.project.ExportsOf(s.project.SurfaceNamed(params.Surface)); ok {
		return names, nil
	}
	return nil, nil
}

func (s *Session) onTests(raw json.RawMessage) (any, error) {
	params, err := protocol.Decode[struct {
		File string `json:"file"`
	}](raw)
	if err != nil {
		return nil, err
	}
	return s.project.ListTests(params.File), nil
}

func (s *Session) onObservables(raw json.RawMessage) (any, error) {
	params, err := protocol.Decode[surfaceParams](raw)
	if err != nil {
		return nil, err
	}
	return s.runner.Observables(params.Surface)
}

func (s *Session) onAffected(raw json.RawMessage) (any, error) {
	params, err := protocol.Decode[struct {
		Base map[string]*string `json:"base"`
	}](raw)
	if err != nil {
		return nil, err
	}
	// Every changed path has an entry in the base, so the base says what changed.
	return s.project.Affected(params.Base)
}

func (s *Session) onInCode(raw json.RawMessage) (any, error) {
	params, err := protocol.Decode[struct {
		Names []string `json:"names"`
		Files []string `json:"files"`
	}](raw)
	if err != nil {
		return nil, err
	}
	return s.project.InCode(params.Names, params.Files)
}
