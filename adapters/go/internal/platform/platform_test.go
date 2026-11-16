package platform

import (
	"testing"

	"github.com/docattest/docattest/adapters/go/internal/protocoldata"
)

// Every platform a core is built for is named the same way both ways, and its core's
// file is the one platforms.json gives.
func TestThePlatformsOfTheCore(t *testing.T) {
	platforms := protocoldata.Platforms()
	if len(platforms) == 0 {
		t.Fatal("no platforms")
	}
	for name, described := range platforms {
		goos, goarch := GoNames(name)
		if back := Name(goos, goarch); back != name {
			t.Errorf("%s: Go calls it %s/%s, which is named %s", name, goos, goarch, back)
		}
		if got := Executable(goos, "docattest"); got != described.Binary {
			t.Errorf("%s: the core is %s, not %s", name, described.Binary, got)
		}
	}
	if got := Name("linux", "amd64"); got != "linux-x64" {
		t.Errorf("got %s", got)
	}
}
