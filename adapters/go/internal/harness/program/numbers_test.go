package program

import (
	"bytes"
	"encoding/json"
	"reflect"
	"strings"
	"testing"
)

// decoded is JSON as encoding/json reads it with UseNumber.
func decoded(t *testing.T, text string) any {
	t.Helper()
	decoder := json.NewDecoder(bytes.NewReader([]byte(text)))
	decoder.UseNumber()
	var read any
	if err := decoder.Decode(&read); err != nil {
		t.Fatal(err)
	}
	return read
}

func TestNumbersAreAsGoHoldsThem(t *testing.T) {
	got, err := DocattestNumbers(decoded(t, `{"sizes": [1048576, 1.0, 1.5, 1e21, -3], "nested": {"port": 8080}, "name": "a"}`))
	want := map[string]any{
		"sizes":  []any{int64(1048576), float64(1), 1.5, 1e21, int64(-3)},
		"nested": map[string]any{"port": int64(8080)},
		"name":   "a",
	}
	if err != nil || !reflect.DeepEqual(got, want) {
		t.Fatalf("got %#v, %v", got, err)
	}
}

func TestANumberNoFloatHoldsIsAnError(t *testing.T) {
	if _, err := DocattestNumbers(decoded(t, `{"far": [1e400]}`)); err == nil || !strings.Contains(err.Error(), "1e400") {
		t.Fatalf("got %v", err)
	}
}
