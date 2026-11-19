package toolchain

import (
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"strings"

	"golang.org/x/mod/modfile"
)

// Workspace is the go.work file every go command of a project runs with, when the
// project's code is in more than one module: a setup package can be a module of its own,
// which requires the product's, so that the product's go.mod names nothing the pages
// need. Empty when the code is in one module, and the go command finds it as it would for
// the user.
type Workspace string

// NewWorkspace writes the go.work of `modules`, given by their directories, the project's
// first, into `dir`, a directory of the adapter's own, so that nothing is written into the
// project. When the go command finds a go.work of the user's for the project, the
// workspace is a copy of it with the modules added, so that the product builds as the user
// builds it; GOWORK=off leaves the user's out. The go command adds the modules, and so
// gives the workspace a go line no module's is above.
func NewWorkspace(dir string, modules []string) (Workspace, error) {
	file := filepath.Join(dir, "go.work")
	users, err := usersWorkspace(modules[0])
	if err != nil {
		return "", err
	}
	verb := "init"
	if users != "" {
		if err := copyWorkspace(users, file); err != nil {
			return "", err
		}
		verb = "use"
	}
	// GOWORK names the file to write: else the go command would write to a go.work it
	// finds above `dir`.
	args := append([]string{"work", verb}, modules...)
	outcome := Run(Command{Path: GoCommand(), Args: args, Dir: dir, Env: []string{"GOWORK=" + file}})
	switch {
	case outcome.Err != nil:
		return "", fmt.Errorf("the go command could not be run: %w", outcome.Err)
	case outcome.Code != 0:
		return "", fmt.Errorf("cannot make a workspace of the modules in %s: %s",
			strings.Join(modules, " and "), strings.TrimSpace(outcome.Stderr))
	}
	return Workspace(file), nil
}

// Environment is what the go command's environment adds to this process's to run in the
// workspace: nothing when there is none.
func (w Workspace) Environment() []string {
	if w == "" {
		return nil
	}
	return []string{"GOWORK=" + string(w)}
}

// usersWorkspace is the go.work the go command runs with in `dir` for the user; "" when
// there is none, or GOWORK=off says to use none.
func usersWorkspace(dir string) (string, error) {
	outcome := Run(Command{Path: GoCommand(), Args: []string{"env", "GOWORK"}, Dir: dir})
	switch {
	case outcome.Err != nil:
		return "", fmt.Errorf("the go command could not be run: %w", outcome.Err)
	case outcome.Code != 0:
		return "", fmt.Errorf("the go command could not say which go.work it uses: %s", strings.TrimSpace(outcome.Stderr))
	}
	if found := strings.TrimSpace(outcome.Stdout); found != "off" {
		return found, nil
	}
	return "", nil
}

// copyWorkspace writes the user's go.work at `users` to `file`, elsewhere.
func copyWorkspace(users, file string) error {
	text, err := os.ReadFile(users)
	if err != nil {
		return fmt.Errorf("cannot read the go.work %s: %w", users, err)
	}
	relocated, err := relocatedWorkspace(text, users)
	if err != nil {
		return err
	}
	if err := os.WriteFile(file, relocated, 0o600); err != nil {
		return fmt.Errorf("cannot write a workspace: %w", err)
	}
	return nil
}

// relocatedWorkspace is a go.work, read from `path`, as it reads from anywhere: each
// directory it names, a module it uses or a replacement, made absolute.
func relocatedWorkspace(text []byte, path string) ([]byte, error) {
	parsed, err := modfile.ParseWork(path, text, nil)
	if err != nil {
		return nil, fmt.Errorf("cannot read the go.work %s: %w", path, err)
	}
	from := filepath.Dir(path)
	out := &modfile.WorkFile{Syntax: &modfile.FileSyntax{}}
	failures := []error{}
	if parsed.Go != nil {
		failures = append(failures, out.AddGoStmt(parsed.Go.Version))
	}
	if parsed.Toolchain != nil {
		failures = append(failures, out.AddToolchainStmt(parsed.Toolchain.Name))
	}
	for _, setting := range parsed.Godebug {
		failures = append(failures, out.AddGodebug(setting.Key, setting.Value))
	}
	for _, used := range parsed.Use {
		failures = append(failures, out.AddUse(absolute(from, used.Path), used.ModulePath))
	}
	for _, replaced := range parsed.Replace {
		target := replaced.New.Path
		// A replacement without a version is a directory.
		if replaced.New.Version == "" {
			target = absolute(from, target)
		}
		failures = append(failures, out.AddReplace(replaced.Old.Path, replaced.Old.Version, target, replaced.New.Version))
	}
	if err := errors.Join(failures...); err != nil {
		return nil, fmt.Errorf("cannot copy the go.work %s: %w", path, err)
	}
	return modfile.Format(out.Syntax), nil
}

// absolute is a path that a file in `from` names, as an absolute path.
func absolute(from, path string) string {
	if filepath.IsAbs(path) {
		return path
	}
	return filepath.Join(from, filepath.FromSlash(path))
}
