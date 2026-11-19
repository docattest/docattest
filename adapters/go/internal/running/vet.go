package running

import (
	"path/filepath"
	"slices"

	"github.com/docattest/docattest/adapters/go/internal/analysis"
	"github.com/docattest/docattest/adapters/go/internal/protocol"
	"github.com/docattest/docattest/adapters/go/internal/toolchain"
)

// The go command builds a docs helper file only with the docs tag, so the project's own
// `go vet ./...` and linters never see one unless told the tag. A check vets the surface
// packages that have docs helper files with the tag, in one go command, whose cache keeps
// the verdict on code that has not changed. Only the findings in docs helper files are
// reported: the package's other files are the project's own vet's, and a file that does
// not compile fails the facts that build it. A finding belongs to no attestation, so it
// is a problem of the project, which fails the check at its file and line.

// vetPrefix begins each finding, so that it says where it comes from.
const vetPrefix = "go vet -tags " + toolchain.DocsTag + ": "

// VetDocsHelpers vets the docs helper files of the surfaces and returns what go vet
// finds in them, each at its file and line as `go vet -tags docattest: <message>`.
func (r *Runner) VetDocsHelpers() []protocol.Problem {
	files := r.project.DocsHelperFiles()
	if len(files) == 0 {
		return []protocol.Problem{}
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
// `files`, each at the file's path from the project root.
func findingsIn(
	problems []toolchain.Problem, moduleDir string, files []analysis.DocsHelperFile,
) []protocol.Problem {
	out := []protocol.Problem{}
	for _, problem := range problems {
		path := problem.File
		if !filepath.IsAbs(path) {
			path = filepath.Join(moduleDir, path)
		}
		at := slices.IndexFunc(files, func(file analysis.DocsHelperFile) bool { return file.Path == path })
		if at >= 0 {
			out = append(out, protocol.Problem{Path: files[at].Page, Line: problem.Line, Message: vetPrefix + problem.Message})
		}
	}
	return out
}
