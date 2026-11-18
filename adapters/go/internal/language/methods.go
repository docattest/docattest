package language

import (
	"encoding/json"

	"github.com/docattest/docattest/adapters/go/internal/protocol"
)

// Methods are the questions about text, by method name.
func Methods() map[string]protocol.Handler {
	return map[string]protocol.Handler{
		"assertions":  assertions,
		"docComments": docComments,
		"docProse":    docProse,
		"mentions":    mentions,
		"expressions": expressions,
		"display":     display,
	}
}

type sourceFiles struct {
	Files []protocol.SourceText `json:"files"`
}

func assertions(raw json.RawMessage) (any, error) {
	params, err := protocol.Decode[struct {
		Examples []struct {
			Code string `json:"code"`
		} `json:"examples"`
	}](raw)
	out := make([]bool, len(params.Examples))
	for k, example := range params.Examples {
		out[k] = Asserts(example.Code)
	}
	return out, err
}

func docComments(raw json.RawMessage) (any, error) {
	params, err := protocol.Decode[sourceFiles](raw)
	out := []protocol.Example{}
	for _, file := range params.Files {
		out = append(out, docExamplesOf(file)...)
	}
	return out, err
}

func docProse(raw json.RawMessage) (any, error) {
	params, err := protocol.Decode[sourceFiles](raw)
	out := make([][]protocol.DocProse, len(params.Files))
	for k, file := range params.Files {
		out[k] = docProseOf(file)
	}
	return out, err
}

func mentions(raw json.RawMessage) (any, error) {
	params, err := protocol.Decode[struct {
		Spans   []string `json:"spans"`
		Symbols []string `json:"symbols"`
	}](raw)
	return mentioned(params.Spans, params.Symbols), err
}

func expressions(raw json.RawMessage) (any, error) {
	params, err := protocol.Decode[struct {
		Expressions []string `json:"expressions"`
	}](raw)
	out := make([]protocol.ExpressionShape, len(params.Expressions))
	for k, expression := range params.Expressions {
		out[k] = shapeOf(expression)
	}
	return out, err
}
