package running

import (
	"fmt"
	"sort"
	"strings"
)

// programImports are the imports of a generated main. A path may be imported under more
// than one name: a surface with a dot for the names a fact uses, and under a name of
// its own for the type of a constant read from source.
type programImports struct {
	named map[importSpec]bool
	// aliases are the names the packages of constants' types are imported under, by path.
	aliases map[string]string
}

// importSpec is one import: a path under a name, "." for a dot import.
type importSpec struct{ name, path string }

func newImports() *programImports {
	return &programImports{named: map[importSpec]bool{}, aliases: map[string]string{}}
}

// add imports a path under a name.
func (i *programImports) add(name, path string) {
	i.named[importSpec{name, path}] = true
}

// alias imports the package of a constant's type under a name nothing else in the
// program uses, and is that name.
func (i *programImports) alias(path string) string {
	if _, done := i.aliases[path]; !done {
		i.aliases[path] = fmt.Sprintf("docattest_t%d", len(i.aliases))
		i.add(i.aliases[path], path)
	}
	return i.aliases[path]
}

// block writes the imports in a fixed order.
func (i *programImports) block() string {
	if len(i.named) == 0 {
		return ""
	}
	specs := make([]importSpec, 0, len(i.named))
	for spec := range i.named {
		specs = append(specs, spec)
	}
	sort.Slice(specs, func(a, b int) bool {
		if specs[a].path != specs[b].path {
			return specs[a].path < specs[b].path
		}
		return specs[a].name < specs[b].name
	})
	var block strings.Builder
	block.WriteString("import (\n")
	for _, spec := range specs {
		fmt.Fprintf(&block, "\t%s %q\n", spec.name, spec.path)
	}
	block.WriteString(")\n")
	return block.String()
}
