package analysis

import (
	"fmt"
	"os"
	"path/filepath"

	"github.com/docattest/docattest/adapters/go/internal/protocoldata"
	"golang.org/x/mod/modfile"
	"golang.org/x/mod/module"
)

// The module of the setup package requires this adapter's module when a runner reads its
// cells with `cells`. The command that runs is pinned elsewhere (a CI step, an install
// line), and the two are upgraded together, so a go.mod that requires another release
// than the one running is a warning.

// adapterModule is the module path of this adapter, as its go.mod declares it.
const adapterModule = "github.com/docattest/docattest/adapters/go"

// versionWarnings warn of each go.mod of the project's code, its module's and its setup
// module's, that requires this adapter's module at another release than the adapter's.
func (p *Project) versionWarnings() []string {
	running := protocoldata.Core().Version
	warnings := []string{}
	for _, dir := range p.moduleDirs() {
		path := filepath.Join(dir, "go.mod")
		text, err := os.ReadFile(path)
		if err != nil {
			warnings = append(warnings, fmt.Sprintf("cannot read %s to compare its %s with this docattest: %v",
				p.rel(path), adapterModule, err))
			continue
		}
		if required := otherRelease(text, "v"+running); required != "" {
			warnings = append(warnings, fmt.Sprintf(
				"%s requires %s %s, but this docattest is %s: upgrade the two together",
				p.rel(path), adapterModule, required, running))
		}
	}
	return warnings
}

// moduleDirs are the directories of the go.mod files of the project's code, when it has any.
func (p *Project) moduleDirs() []string {
	switch {
	case p.Module.Path == "":
		return nil
	case p.SetupModule.Dir != p.Module.Dir:
		return []string{p.Module.Dir, p.SetupModule.Dir}
	}
	return []string{p.Module.Dir}
}

// otherRelease is the release of this adapter's module a go.mod requires when it is not
// `release`; "" when it requires that one or none. A pseudo-version names a commit, not a
// release, and a replaced module is not the release its requirement names, so neither is
// compared. A go.mod that does not parse is the go command's to report, where it lists
// the packages.
func otherRelease(goMod []byte, release string) string {
	parsed, err := modfile.Parse("go.mod", goMod, nil)
	if err != nil {
		return ""
	}
	for _, replaced := range parsed.Replace {
		if replaced.Old.Path == adapterModule {
			return ""
		}
	}
	for _, required := range parsed.Require {
		version := required.Mod.Version
		if required.Mod.Path == adapterModule && version != release && !module.IsPseudoVersion(version) {
			return version
		}
	}
	return ""
}
