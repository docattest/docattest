package analysis

import "sort"

// sortedKeys are a map's keys in order.
func sortedKeys[V any](m map[string]V) []string {
	keys := make([]string, 0, len(m))
	for key := range m {
		keys = append(keys, key)
	}
	sort.Strings(keys)
	return keys
}

// union is the keys that are in either set.
func union[K comparable](a, b map[K]bool) map[K]bool {
	out := map[K]bool{}
	for key := range a {
		out[key] = true
	}
	for key := range b {
		out[key] = true
	}
	return out
}

// unionInOrder is a list followed by what a second list adds to it, each item once.
func unionInOrder(a, b []string) []string {
	seen := map[string]bool{}
	out := []string{}
	for _, item := range append(append([]string{}, a...), b...) {
		if !seen[item] {
			seen[item] = true
			out = append(out, item)
		}
	}
	return out
}
