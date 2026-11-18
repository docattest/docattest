package running

import (
	"fmt"
	"os"
	"path/filepath"
	"sync"

	"github.com/docattest/docattest/adapters/go/internal/harness"
	"github.com/docattest/docattest/adapters/go/internal/language"
	"github.com/docattest/docattest/adapters/go/internal/protocol"
	"github.com/docattest/docattest/adapters/go/internal/toolchain"
)

// blocksDir is the directory of the module that generated packages are compiled in. It
// exists only in the build overlay.
const blocksDir = "docattest_blocks"

// startFile is the name of the file that writes a block's start line. The go command
// gives the compiler a package's files in the order of their names, and a package's
// variables are initialised in that order, before any init function: this name comes
// before "block.go", so the line is written before any of the block's code runs.
const startFile = "0_docattest_start.go"

// block is one example of a page as a program: a package of its own in the module.
type block struct {
	example protocol.Example
	// name is its package's directory inside blocksDir, and its program's file name.
	name string
	// label names the block in every position the compiler and a panic report.
	label string
	form  language.Form
	// problems are why it does not compile.
	problems []problem
	// binary is its program, when one was built.
	binary string
}

// problem is one complaint about a block, at a line of the block.
type problem struct {
	Line    int
	Message string
}

// blocks are the page examples of a session, each built once.
type blocks struct {
	// building lets one batch build at a time; a second waits and finds its blocks built.
	building sync.Mutex
	busy     sync.WaitGroup
	byID     map[string]*block
}

// start builds examples in the background.
func (b *blocks) start(r *Runner, examples []protocol.Example) {
	b.busy.Add(1)
	go func() {
		defer b.busy.Done()
		b.ensure(r, examples)
	}()
}

func (b *blocks) wait() { b.busy.Wait() }

// ensure builds, in one run of the go command, those of the examples that are blocks of
// a page and are not built yet, and returns every such example's block by id.
func (b *blocks) ensure(r *Runner, examples []protocol.Example) map[string]*block {
	b.building.Lock()
	defer b.building.Unlock()
	if b.byID == nil {
		b.byID = map[string]*block{}
	}
	fresh := []*block{}
	for _, example := range examples {
		if language.IsSource(example.Page) || example.Error != "" || b.byID[example.ID] != nil {
			continue
		}
		made := r.newBlock(example)
		b.byID[example.ID] = made
		if len(made.problems) == 0 {
			fresh = append(fresh, made)
		}
	}
	if len(fresh) > 0 {
		r.build(fresh)
	}
	return b.byID
}

// newBlock writes a block's package and places it in the module; a block that cannot be
// written has that for its problem.
func (r *Runner) newBlock(example protocol.Example) *block {
	name := fmt.Sprintf("e%d", r.serial.take())
	made := &block{example: example, name: name, label: "docattest_" + name + ".go"}
	made.form = language.Program(example.Code, made.label)
	pkg := "main"
	if made.form.File != nil {
		pkg = made.form.File.Name.Name
	}
	files := map[string]string{"block.go": made.form.Source, startFile: startSource(pkg)}
	if !example.Network {
		files["docattest_guard.go"] = guardSource(pkg)
	}
	if err := r.place(name, files); err != nil {
		made.problems = []problem{{1, err.Error()}}
	}
	return made
}

// startSource is a file that writes the start line as its package begins to be
// initialised.
func startSource(pkg string) string {
	return harness.Source(harness.Start, pkg) + "\nvar docattestStarted = docattestSayStarted()\n"
}

// guardSource is a file that refuses the network before anything of its package runs.
func guardSource(pkg string) string {
	return harness.Source(harness.Guard, pkg) + "\nfunc init() { docattestRefuseNetwork() }\n"
}

// place writes the files of a generated package and puts them in the module's overlay.
func (r *Runner) place(name string, files map[string]string) error {
	dir, err := r.work("generated", name)
	if err != nil {
		return err
	}
	for file, text := range files {
		real := filepath.Join(dir, file)
		if err := writeFile(real, text); err != nil {
			return fmt.Errorf("cannot write the generated %s: %w", file, err)
		}
		r.gotool.Place(filepath.Join(blocksDir, name, file), real)
	}
	return nil
}

// build builds blocks together and gives each its program or its problems.
func (r *Runner) build(fresh []*block) {
	out, err := r.work("bin")
	if err != nil {
		for _, made := range fresh {
			made.problems = []problem{{1, err.Error()}}
		}
		return
	}
	dirs := make([]string, len(fresh))
	for k, made := range fresh {
		dirs[k] = filepath.Join(blocksDir, made.name)
	}
	general := attribute(fresh, r.gotool.Build(out, dirs))
	for _, made := range fresh {
		binary := filepath.Join(out, toolchain.Executable(made.name))
		if _, err := os.Stat(binary); err == nil {
			made.binary = binary
		} else if len(made.problems) == 0 && general != "" {
			made.problems = []problem{{1, general}}
		}
	}
}

// attribute gives each complaint of a build to the block it is about: by the label in
// its position, else by its package, at line 1. It returns the first complaint that is
// about no block, which is then about all of them (a module that cannot be loaded).
func attribute(fresh []*block, problems []toolchain.Problem) string {
	general := ""
	for _, p := range problems {
		owner := ownerOf(fresh, p)
		switch {
		case owner != nil && filepath.Base(p.File) == owner.label:
			owner.problems = append(owner.problems, problem{p.Line, p.Message})
		case owner != nil:
			owner.problems = append(owner.problems, problem{1, p.Message})
		case general == "":
			general = p.Message
		}
	}
	return general
}

func ownerOf(fresh []*block, p toolchain.Problem) *block {
	for _, made := range fresh {
		if filepath.Base(p.File) == made.label || filepath.Base(p.Package) == made.name {
			return made
		}
	}
	return nil
}

// Compile answers `compile`: for each example, why it does not compile, if it does not.
func (r *Runner) Compile(examples []protocol.Example) []protocol.Diagnostic {
	built := r.blocks.ensure(r, examples)
	out := []protocol.Diagnostic{}
	for _, example := range examples {
		problems := []problem{}
		if language.IsSource(example.Page) {
			problems = r.docExampleProblems(example)
		} else if made := built[example.ID]; made != nil {
			problems = made.problems
		}
		for _, p := range problems {
			out = append(out, protocol.Diagnostic{ID: example.ID, Message: p.Message, Line: max(p.Line, 1)})
		}
	}
	return out
}
