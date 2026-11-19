package running

import (
	"fmt"
	"path/filepath"
	"slices"

	"github.com/docattest/docattest/adapters/go/internal/analysis"
	"github.com/docattest/docattest/adapters/go/internal/toolchain"
)

// The go command builds a docs helper file only with the docs tag, so the project's own
// `go vet ./...` and linters never see one unless told the tag. A check vets the surface
// packages that have docs helper files with the tag, in one go command, whose cache keeps
// the verdict on code that has not changed. Only the findings in docs helper files are
// reported: the package's other files are the project's own vet's, and a file that does
// not compile fails the facts that build it.

// vetPrefix begins each finding, so that it says where it comes from.
const vetPrefix = "go vet -tags " + toolchain.DocsTag

// VetDocsHelpers vets the docs helper files of the surfaces and returns what go vet
// finds in them, by surface name, each as `go vet -tags docattest: <page>:<line>: <message>`.
func (r *Runner) VetDocsHelpers() map[string][]string {
	files := r.project.DocsHelperFiles()
	if len(files) == 0 {
		return map[string][]string{}
	}
	return findingsIn(r.gotool.VetWithDocs(r.packagesOf(files)), r.project.Module.Dir, files)
}

// packagesOf are the packages of files, by their directories relative to the module.
func (r *Runner) packagesOf(files []analysis.DocsHelperFile) []string {
	packages := []string{}
	for _, file := range files {
		if pkg := r.project.InModule(filepath.Dir(file.Path)); !slices.Contains(packages, pkg) {
			packages = append(packages, pkg)
		}
	}
	return packages
}

// findingsIn are the problems go vet reported, run in `moduleDir`, that are in one of
// `files`, by the surface of the file.
func findingsIn(
	problems []toolchain.Problem, moduleDir string, files []analysis.DocsHelperFile,
) map[string][]string {
	out := map[string][]string{}
	for _, problem := range problems {
		path := problem.File
		if !filepath.IsAbs(path) {
			path = filepath.Join(moduleDir, path)
		}
		for _, file := range files {
			if file.Path == path {
				finding := fmt.Sprintf("%s: %s:%d: %s", vetPrefix, file.Page, problem.Line, problem.Message)
				out[file.Surface] = append(out[file.Surface], finding)
			}
		}
	}
	return out
}
