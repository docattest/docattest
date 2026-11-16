package program

import (
	"errors"
	"reflect"
	"strings"
	"testing"
)

type code string

type named struct {
	Name  string
	Sizes map[string]int
	note  string
}

func TestObserve(t *testing.T) {
	cases := []struct {
		value       any
		text        string
		items       any
		kind        any
		description string
	}{
		{3, "3", nil, nil, "int 3"},
		{"a b", "a b", nil, "text", `string "a b"`},
		{code("x"), "x", nil, "text", `program.code "x"`},
		{true, "true", nil, nil, "bool true"},
		{1.5, "1.5", nil, nil, "float64 1.5"},
		{nil, "<nil>", nil, nil, "nil"},
		{[]string{"b", "a"}, "[b a]", []string{"b", "a"}, nil, "a []string"},
		{[]int{}, "[]", []string{}, nil, "an []int"},
		{map[string]int{"b": 1, "a": 2}, "map[a:2 b:1]", []string{"a", "b"}, nil, "a map[string]int"},
		{map[int]bool{10: true, 9: true}, "map[9:true 10:true]", []string{"9", "10"}, nil, "a map[int]bool"},
		{errors.New("gone"), "gone", nil, nil, "an *errors.errorString"},
	}
	for _, c := range cases {
		got := docattestObserve(c.value)
		want := docattestObservation{"text": c.text, "description": c.description}
		if c.items != nil {
			want["items"] = c.items
		}
		if c.kind != nil {
			want["kind"] = c.kind
		}
		if !reflect.DeepEqual(got, want) {
			t.Errorf("observe(%#v)\n got  %v\n want %v", c.value, got, want)
		}
	}
}

func TestWalkFollowsFieldsAndStringKeys(t *testing.T) {
	found := []docattestObservable{}
	root := &named{Name: "n", Sizes: map[string]int{"small": 1}, note: "hidden"}
	docattestWalk(&found, reflect.ValueOf(root), "Root", []string{"Root"})
	got := []string{}
	for _, o := range found {
		got = append(got, o.Expression+"="+o.Value["text"].(string))
	}
	want := []string{"Root={n map[small:1] hidden}", "Root.Name=n", "Root.Sizes=map[small:1]", `Root.Sizes["small"]=1`}
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("got %v", got)
	}
	found = found[:0]
	docattestWalk(&found, reflect.ValueOf(errors.New("gone")), "Err", []string{"Err"})
	docattestWalk(&found, reflect.ValueOf(func() {}), "F", []string{"F"})
	if len(found) != 1 || found[0].Value["text"] != "gone" {
		t.Fatalf("an error is shown as it prints, and a function is not shown: %v", found)
	}
}

// holder holds what fmt prints as an address, and something that is printed in full.
type holder struct {
	Files *[]string
	Shown []string
}

func TestWalkLeavesOutWhatPrintsAnAddress(t *testing.T) {
	found := []docattestObservable{}
	files := []string{"a"}
	docattestWalk(&found, reflect.ValueOf(holder{Files: &files, Shown: []string{"x"}}), "H", []string{"H"})
	docattestWalk(&found, reflect.ValueOf(holder{Shown: []string{"y"}}), "Nil", []string{"Nil"})
	docattestWalk(&found, reflect.ValueOf(&named{Name: "p"}), "Ptr", []string{"Ptr"})
	docattestWalk(&found, reflect.ValueOf([]any{func() {}}), "Funcs", []string{"Funcs"})
	got := []string{}
	for _, o := range found {
		got = append(got, o.Expression)
	}
	// H prints its field's address; what the field points to prints in full, and so does a
	// nil pointer, a pointer at the top to a struct, and nothing holding a function.
	want := []string{"H.Files", "H.Shown", "Nil", "Nil.Shown", "Ptr", "Ptr.Name", "Ptr.Sizes"}
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("got %v", got)
	}
}

func TestRefusal(t *testing.T) {
	cases := map[string]docattestRunner{"adds": nil, "subtracts": nil}
	table := docattestTable{Runner: "nope", Setup: "docs/setup.go", Line: 7, Header: []string{"a"}}
	got := docattestRefusal(table, cases, false, nil)
	if got != `cases runner "nope" at L7 is not in Cases of docs/setup.go (known: adds, subtracts)` {
		t.Fatal(got)
	}
	table.Runner = "adds"
	if got := docattestRefusal(table, cases, true, []string{"a"}); got != "" {
		t.Fatal(got)
	}
	got = docattestRefusal(table, cases, true, []string{"a", "b", "sum"})
	if !strings.HasSuffix(got, `the table is missing columns "b", "sum" declared by the runner`) {
		t.Fatal(got)
	}
}

func TestGuardedReportsAPanic(t *testing.T) {
	line := docattestGuarded(4, func(int) any { panic("boom") })
	if line.Item != 4 || line.Panic != "boom" || line.Value != nil {
		t.Fatalf("got %+v", line)
	}
	if line := docattestGuarded(1, func(k int) any { return k * 2 }); line.Value != 2 || line.Panic != "" {
		t.Fatalf("got %+v", line)
	}
}
