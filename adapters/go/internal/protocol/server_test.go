package protocol

import (
	"bytes"
	"encoding/json"
	"errors"
	"strings"
	"testing"
	"time"

	"github.com/docattest/docattest/adapters/go/internal/protocoldata"
)

type fakeSession struct{ closed *bool }

func (f fakeSession) Methods() map[string]Handler {
	return map[string]Handler{
		"slow": func(json.RawMessage) (any, error) {
			time.Sleep(100 * time.Millisecond)
			return "slow", nil
		},
		"broken": func(json.RawMessage) (any, error) { panic("oh") },
		"wrong":  func(json.RawMessage) (any, error) { return nil, UserError("docattest: no such thing") },
		"none":   func(json.RawMessage) (any, error) { return nil, nil },
	}
}

func (f fakeSession) Close() { *f.closed = true }

// talk sends lines to a server and returns its answers by id, and the order they came in.
func talk(t *testing.T, lines ...string) (answers map[string]map[string]any, order []string, closed bool) {
	t.Helper()
	adapter := Adapter{
		Hello: Hello{Protocol: protocoldata.Core().Protocol, Adapter: AdapterName{Name: "fake"}},
		Language: map[string]Handler{
			"quick": func(json.RawMessage) (any, error) { return "quick", nil },
		},
		Open: func(raw json.RawMessage) (Session, error) {
			if strings.Contains(string(raw), "refuse") {
				return nil, errors.New("refused")
			}
			return fakeSession{&closed}, nil
		},
	}
	var out bytes.Buffer
	Serve(strings.NewReader(strings.Join(lines, "\n")+"\n"), &out, adapter)
	answers = map[string]map[string]any{}
	for _, line := range strings.Split(strings.TrimSpace(out.String()), "\n") {
		var answer map[string]any
		if err := json.Unmarshal([]byte(line), &answer); err != nil {
			t.Fatalf("not JSON: %q", line)
		}
		id, _ := json.Marshal(answer["id"])
		answers[string(id)] = answer
		order = append(order, string(id))
	}
	return answers, order, closed
}

func kindOf(answer map[string]any) string {
	failure, _ := answer["error"].(map[string]any)
	kind, _ := failure["kind"].(string)
	return kind
}

const hello = `{"id":1,"method":"hello","params":{"protocol":1}}`

func TestOrderAndTheEndOfInput(t *testing.T) {
	answers, order, closed := talk(t, hello,
		`{"id":2,"method":"open","params":{}}`,
		`{"id":3,"method":"slow"}`,
		`{"id":4,"method":"quick"}`,
		`{"id":5,"method":"close"}`,
		`{"id":6,"method":"slow"}`,
	)
	if got := strings.Join(order, " "); got != "1 2 4 3 5 6" {
		t.Fatalf("answers came in the order %s", got)
	}
	if answers["3"]["result"] != "slow" || kindOf(answers["6"]) != "internal" || !closed {
		t.Fatalf("got %v, closed %v", answers, closed)
	}
	_, _, closed = talk(t, hello, `{"id":2,"method":"open"}`, `{"id":3,"method":"slow"}`)
	if !closed {
		t.Fatal("the session is closed at the end of input")
	}
}

func TestMistakesAreAnsweredAndLeaveTheServerServing(t *testing.T) {
	answers, _, _ := talk(t,
		`{"id":1,"method":"quick"}`,
		`{"id":2,"method":"hello","params":{"protocol":7}}`,
		`{"id":3,"method":"hello","params":{"protocol":1}}`,
		`{"id":4,"method":"slow"}`,
		`{"id":5,"method":"open","params":{"refuse":true}}`,
		`{"id":6,"method":"open"}`,
		`{"id":7,"method":"open"}`,
		`{"id":8,"method":"no-such"}`,
		`{"id":9,"method":"broken"}`,
		`{"id":10,"method":"wrong"}`,
		`{"id":11,"method":"none"}`,
		`not json`,
		`{"id":-1,"method":"quick"}`,
		``,
	)
	want := map[string]string{"1": "internal", "2": "user", "4": "internal", "5": "internal", "7": "internal",
		"8": "internal", "9": "internal", "10": "user", "null": "internal"}
	for id, kind := range want {
		if kindOf(answers[id]) != kind {
			t.Errorf("request %s: got %v", id, answers[id])
		}
	}
	if _, has := answers["11"]["result"]; !has || answers["11"]["result"] != nil {
		t.Errorf("a nil result is written as null: %v", answers["11"])
	}
	if len(answers) != 12 {
		t.Errorf("got %d distinct answers", len(answers))
	}
}
