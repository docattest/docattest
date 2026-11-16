package program

import (
	"fmt"
	"reflect"
	"sort"
	"strconv"
	"strings"
)

// docattestObservation is a value as the core compares it (the protocol's Observation).
type docattestObservation map[string]any

// docattestObserve shows a value as Go displays it: fmt.Sprint of it, and of each of its
// items when it is a slice, an array, or a map (whose items are its keys, in the order
// fmt prints them).
func docattestObserve(value any) docattestObservation {
	observation := docattestObservation{
		"text":        fmt.Sprint(value),
		"description": docattestDescribe(value),
	}
	v := reflect.ValueOf(value)
	switch v.Kind() {
	case reflect.String:
		observation["kind"] = "text"
	case reflect.Slice, reflect.Array:
		items := make([]string, v.Len())
		for k := range items {
			items[k] = fmt.Sprint(v.Index(k).Interface())
		}
		observation["items"] = items
	case reflect.Map:
		observation["items"] = docattestKeys(v)
	}
	return observation
}

// docattestKeys are a map's keys as text, sorted as fmt sorts them.
func docattestKeys(v reflect.Value) []string {
	keys := v.MapKeys()
	sort.Slice(keys, func(a, b int) bool { return docattestLess(keys[a], keys[b]) })
	items := make([]string, len(keys))
	for k, key := range keys {
		items[k] = fmt.Sprint(key.Interface())
	}
	return items
}

func docattestLess(a, b reflect.Value) bool {
	switch {
	case a.CanInt() && b.CanInt():
		return a.Int() < b.Int()
	case a.CanUint() && b.CanUint():
		return a.Uint() < b.Uint()
	case a.CanFloat() && b.CanFloat():
		return a.Float() < b.Float()
	}
	return fmt.Sprint(a.Interface()) < fmt.Sprint(b.Interface())
}

// docattestDescribe is a few words about a value, for a message.
func docattestDescribe(value any) string {
	if value == nil {
		return "nil"
	}
	kind := reflect.TypeOf(value).Kind()
	name := fmt.Sprintf("%T", value)
	switch {
	case kind == reflect.String:
		return name + " " + strconv.Quote(fmt.Sprint(value))
	case kind == reflect.Bool || (kind >= reflect.Int && kind <= reflect.Complex128):
		return name + " " + fmt.Sprint(value)
	}
	article := "a "
	if strings.ContainsAny(strings.ToLower(strings.TrimLeft(name, "[]*"))[:1], "aeiou") {
		article = "an "
	}
	return article + name
}
