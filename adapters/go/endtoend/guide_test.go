package endtoend

import "testing"

// The guide, ../README.md and the pages of ../docs/, and ../CONTRIBUTING.md are checked by
// docattest with this adapter (../docattest.config.json): their facts, cases tables,
// citations and commands, and the sample page the guide shows under "A page", which an
// `md check` block checks in a copy of ../testdata/fixture.
func TestTheGuideHolds(t *testing.T) {
	if out, code := docattest(t, "..", "check"); code != 0 {
		t.Fatalf("exit %d\n%s", code, out)
	}
}
