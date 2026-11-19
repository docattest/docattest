package citedtests

import (
	"testing"

	"github.com/docattest/docattest/adapters/go/internal/protocol"
)

func TestReportedNamesAreRewrittenAsTestingRewritesThem(t *testing.T) {
	cases := []struct {
		describes []string
		title     string
		want      string
	}{
		{nil, "TestAdds", "TestAdds"},
		{[]string{"TestRounding"}, "rounds up", "TestRounding/rounds_up"},
		{[]string{"TestA", "in a group"}, "and\ta tab em", "TestA/in_a_group/and_a_tab_em"},
		{[]string{"TestA"}, "nul\x00 and bell\a", `TestA/nul\x00_and_bell\a`},
		{[]string{"TestA"}, "", "TestA/#00"},
		{[]string{"TestA"}, "a/b", "TestA/a/b"},
		{[]string{"TestA"}, "é ünïcode", "TestA/é_ünïcode"},
	}
	for _, c := range cases {
		got := reportedName(protocol.CitedTest{File: "a_test.go", Describes: c.describes, Title: c.title})
		if got != c.want {
			t.Errorf("%q > %q: got %q, want %q", c.describes, c.title, got, c.want)
		}
	}
}

func TestThePatternAnchorsAndQuotesEachElementOfEachName(t *testing.T) {
	got := runPattern([]string{"TestAdds", "TestRounding/rounds_up", "TestOdd/a_(b)|c_[d]_e.f*/g"})
	want := `^TestAdds$|^TestRounding$/^rounds_up$|^TestOdd$/^a_\(b\)\|c_\[d\]_e\.f\*$/^g$`
	if got != want {
		t.Fatalf("got  %s\nwant %s", got, want)
	}
}
