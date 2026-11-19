// Package docs is the setup of the pages in this directory, in a module of its own that
// requires the product's.
package docs

import (
	"fmt"
	"strings"

	"example.com/greet"
)

// Cases are the runners of cases tables, by name.
var Cases = map[string]func(row map[string]string) error{
	"hello": func(row map[string]string) error {
		if got := greet.Hello(row["name"]); got != row["greeting"] {
			return fmt.Errorf("expected %q, got %q", row["greeting"], got)
		}
		return nil
	},
}

// Shouted is the greeting in capitals, for a fact.
func Shouted() string { return strings.ToUpper(greet.Greeting) }
