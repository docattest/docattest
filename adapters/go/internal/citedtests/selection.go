package citedtests

import (
	"regexp"
	"slices"
	"strconv"
	"strings"

	"github.com/docattest/docattest/adapters/go/internal/protocol"
)

// reportedName is a test's name as the testing package reports it and -run matches it:
// the function's name, then each subtest's name as testing rewrites it, joined by `/`.
func reportedName(test protocol.CitedTest) string {
	names := append(slices.Clone(test.Describes), test.Title)
	for k := 1; k < len(names); k++ {
		names[k] = subtestName(names[k])
	}
	return strings.Join(names, "/")
}

// subtestName is the name testing gives the first subtest that `t.Run` starts with a
// name: every space an underscore and every rune that does not print escaped
// (testing.rewrite), and an empty name `#00`.
func subtestName(name string) string {
	if name == "" {
		return "#00"
	}
	var out strings.Builder
	for _, r := range name {
		switch {
		case isSpace(r):
			out.WriteByte('_')
		case !strconv.IsPrint(r):
			quoted := strconv.QuoteRune(r)
			out.WriteString(quoted[1 : len(quoted)-1])
		default:
			out.WriteRune(r)
		}
	}
	return out.String()
}

// isSpace is testing's own list of white space, which is not Unicode's Z class.
func isSpace(r rune) bool {
	if r < 0x2000 {
		switch r {
		case '\t', '\n', '\v', '\f', '\r', ' ', 0x85, 0xA0, 0x1680:
			return true
		}
		return false
	}
	switch r {
	case 0x2028, 0x2029, 0x202f, 0x205f, 0x3000:
		return true
	}
	return r <= 0x200a
}

// runPattern is the -run pattern that selects exactly the tests named, and no other: one
// alternative for each, each element of its name anchored and quoted. testing splits a
// pattern at every `|` and then every `/` outside brackets and parentheses, and matches
// each element against the element of a name at the same depth; QuoteMeta escapes every
// bracket, parenthesis and bar a name holds, so the splits are exactly the ones written
// here. testing also rewrites each element of the pattern as it rewrites a subtest's
// name, which leaves a quoted reported name as it is.
func runPattern(names []string) string {
	alternatives := []string{}
	for _, name := range names {
		elements := []string{}
		for _, element := range strings.Split(name, "/") {
			elements = append(elements, "^"+regexp.QuoteMeta(element)+"$")
		}
		alternatives = append(alternatives, strings.Join(elements, "/"))
	}
	return strings.Join(alternatives, "|")
}
