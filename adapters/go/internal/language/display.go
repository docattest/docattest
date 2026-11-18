package language

import (
	"bytes"
	"encoding/json"
	"fmt"

	"github.com/docattest/docattest/adapters/go/internal/harness/program"
	"github.com/docattest/docattest/adapters/go/internal/protocol"
)

// display shows each value of a data block as a fact's value of the same value shows.
func display(raw json.RawMessage) (any, error) {
	params, err := protocol.Decode[struct {
		Values []json.RawMessage `json:"values"`
	}](raw)
	if err != nil {
		return nil, err
	}
	out := make([]string, len(params.Values))
	for k, value := range params.Values {
		if out[k], err = shown(value); err != nil {
			return nil, err
		}
	}
	return out, nil
}

// shown is fmt.Sprint of what encoding/json reads a value as, its numbers as a fact's
// program reads those of a snapshot or a manifest (program.DocattestNumbers).
func shown(value json.RawMessage) (string, error) {
	decoder := json.NewDecoder(bytes.NewReader(value))
	decoder.UseNumber()
	var read any
	if err := decoder.Decode(&read); err != nil {
		return "", err
	}
	read, err := program.DocattestNumbers(read)
	return fmt.Sprint(read), err
}
