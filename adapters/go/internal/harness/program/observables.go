package program

import (
	"fmt"
	"reflect"
	"strconv"
)

// The bounds of the walk over what a surface offers.
const (
	docattestMaxDepth       = 3
	docattestMaxObservables = 2000
)

// docattestRoot is one exported constant or variable of a surface.
type docattestRoot struct {
	Name string
	Read func() any
}

// docattestObservable is a value a surface offers (the protocol's Observable, without
// the surface's name, which the adapter adds).
type docattestObservable struct {
	Expression string               `json:"expression"`
	Path       []string             `json:"path"`
	Value      docattestObservation `json:"value"`
}

// docattestObservables serves one item: every value the roots offer, each with the fact
// expression that reads it. Below a root it follows the fields of a struct and the
// entries of a map with string keys.
func docattestObservables(roots []docattestRoot) {
	docattestServe(docattestStart(), 1, func(int) any {
		found := []docattestObservable{}
		for _, root := range roots {
			value := reflect.ValueOf(root.Read())
			docattestWalk(&found, value, root.Name, []string{root.Name})
		}
		return found
	})
}

func docattestWalk(found *[]docattestObservable, v reflect.Value, expression string, path []string) {
	written := v
	for v.Kind() == reflect.Pointer || v.Kind() == reflect.Interface {
		v = v.Elem()
	}
	if !docattestShown(v) || len(*found) >= docattestMaxObservables {
		return
	}
	// A value shown differently in each run is not offered; what lies below it may be.
	if shown := docattestRead(written, v); docattestStable(reflect.ValueOf(shown), 0) {
		*found = append(*found, docattestObservable{expression, path, docattestObserve(shown)})
	}
	if len(path) >= docattestMaxDepth {
		return
	}
	for _, member := range docattestMembers(v) {
		docattestWalk(found, member.value, expression+member.access, docattestBelow(path, member.name))
	}
}

// docattestMember is something below a value: its name, and how an expression reaches it.
type docattestMember struct {
	name, access string
	value        reflect.Value
}

// docattestMembers are the exported fields of a struct and the entries of a map with
// string keys; nothing else has any.
func docattestMembers(v reflect.Value) []docattestMember {
	members := []docattestMember{}
	switch {
	case v.Kind() == reflect.Struct:
		for k := 0; k < v.NumField(); k++ {
			if field := v.Type().Field(k); field.IsExported() {
				members = append(members, docattestMember{field.Name, "." + field.Name, v.Field(k)})
			}
		}
	case v.Kind() == reflect.Map && v.Type().Key().Kind() == reflect.String:
		for _, key := range docattestKeys(v) {
			entry := v.MapIndex(reflect.ValueOf(key).Convert(v.Type().Key()))
			members = append(members, docattestMember{key, "[" + strconv.Quote(key) + "]", entry})
		}
	}
	return members
}

// docattestRead is the value to show for something that was written as `written` and
// points at `pointed`: what was written when it says how it is displayed (an error, a
// fmt.Stringer), as a fact that reads it would show it; else what it points at.
func docattestRead(written, pointed reflect.Value) any {
	if written.CanInterface() {
		switch written.Interface().(type) {
		case error, fmt.Stringer:
			return written.Interface()
		}
	}
	return pointed.Interface()
}

// docattestShown says whether a value is one a page could state: not a function, a
// channel or a bare pointer, and not something that cannot be read.
func docattestShown(v reflect.Value) bool {
	if !v.IsValid() || !v.CanInterface() {
		return false
	}
	switch v.Kind() {
	case reflect.Func, reflect.Chan, reflect.UnsafePointer:
		return false
	}
	return true
}

func docattestBelow(path []string, name string) []string {
	return append(append([]string{}, path...), name)
}

// docattestStable says whether fmt shows a value the same way in every run, which it does
// not when it prints an address: for a pointer, a function or a channel inside another
// value, and for a pointer at the top to anything but an array, a slice, a struct or a map.
// A value that says how it is displayed (an error, a fmt.Stringer) is taken at its word, as
// fmt takes it. `depth` is fmt's: 0 for the value itself.
func docattestStable(v reflect.Value, depth int) bool {
	if !v.IsValid() {
		return true
	}
	if v.CanInterface() {
		switch v.Interface().(type) {
		case error, fmt.Stringer:
			return true
		}
	}
	switch v.Kind() {
	case reflect.Pointer:
		return v.IsNil() || (depth == 0 && docattestShownWhole(v.Elem().Kind()) && docattestStable(v.Elem(), 1))
	case reflect.Func, reflect.Chan, reflect.UnsafePointer:
		return v.IsNil()
	case reflect.Interface:
		return docattestStable(v.Elem(), depth+1)
	}
	for _, part := range docattestParts(v) {
		if !docattestStable(part, depth+1) {
			return false
		}
	}
	return true
}

// docattestParts are what fmt prints of a value inside it: the fields of a struct, the keys
// and values of a map, the elements of an array or a slice; nothing else has any.
func docattestParts(v reflect.Value) []reflect.Value {
	parts := []reflect.Value{}
	switch v.Kind() {
	case reflect.Struct:
		for k := 0; k < v.NumField(); k++ {
			parts = append(parts, v.Field(k))
		}
	case reflect.Map:
		for entry := v.MapRange(); entry.Next(); {
			parts = append(parts, entry.Key(), entry.Value())
		}
	case reflect.Array, reflect.Slice:
		for k := 0; k < v.Len(); k++ {
			parts = append(parts, v.Index(k))
		}
	}
	return parts
}

// docattestShownWhole says whether fmt shows what a pointer at the top points to (`&{…}`),
// rather than the pointer's address.
func docattestShownWhole(kind reflect.Kind) bool {
	return kind == reflect.Array || kind == reflect.Slice || kind == reflect.Struct || kind == reflect.Map
}
