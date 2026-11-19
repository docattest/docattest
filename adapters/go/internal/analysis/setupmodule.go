package analysis

import "github.com/docattest/docattest/adapters/go/internal/toolchain"

// A setup package may be a module of its own, a go.mod in its directory (or above it,
// below the project's module) that requires the project's module, so that what only the
// pages need, such as the module of `cells`, is required there and not by the product.
// The project's go.mod does not require the setup module, so neither module alone builds
// a program that imports both. The go command then runs in a workspace of the two, which
// the adapter writes into a directory of its own, from the user's go.work when the project
// has one (toolchain.NewWorkspace): the project is listed in it, and every program it
// generates built in it, while the user's tree is left as it is.

// joinSetupModule finds the setup package's module and, when it is not the project's,
// makes the workspace of the two in `work`.
func (p *Project) joinSetupModule(work string, inMemory bool) error {
	p.SetupModule = p.Module
	if p.Setup == "" || p.Module.Path == "" {
		return nil
	}
	found, err := findModule(p.packageDir(p.Setup), p.Root, inMemory)
	if err != nil || found.Path == "" || found.Dir == p.Module.Dir {
		return err
	}
	p.SetupModule = found
	p.Workspace, err = toolchain.NewWorkspace(work, []string{p.Module.Dir, found.Dir})
	return err
}
