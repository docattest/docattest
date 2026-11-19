package running

import (
	"fmt"
	"go/types"
	"strings"
)

// typeWriter writes types for a generated program, copying those it cannot name
// (variables.go says which).
type typeWriter struct {
	// program is the import path of the generated program.
	program string
	// copied are the named types copied so far, by name.
	copied map[string]*types.Named
	// declarations are the declarations of the copied types, each before those that use it.
	declarations []markedSource
}

// newTypeWriter is a type writer for a program with an import path that has copied
// nothing yet.
func newTypeWriter(program string) *typeWriter {
	return &typeWriter{program: program, copied: map[string]*types.Named{}}
}

// write writes a type.
func (w *typeWriter) write(of types.Type) (string, error) {
	switch t := types.Unalias(of).(type) {
	case *types.Basic:
		if t.Kind() != types.UnsafePointer {
			return t.Name(), nil
		}
	case *types.Named:
		return w.named(t)
	case *types.Slice:
		item, err := w.write(t.Elem())
		return "[]" + item, err
	case *types.Array:
		item, err := w.write(t.Elem())
		return fmt.Sprintf("[%d]%s", t.Len(), item), err
	case *types.Map:
		return w.mapType(t)
	case *types.Struct:
		return w.structType(t)
	case *types.Interface:
		if t.Empty() {
			return "any", nil
		}
	}
	return "", fmt.Errorf("its type holds %s, which a fact cannot copy", of)
}

// named writes a named type: by its package's name when the program can name it and set
// its fields, else as a copy.
func (w *typeWriter) named(t *types.Named) (string, error) {
	object := t.Obj()
	switch {
	case object.Pkg() == nil:
		return object.Name(), nil
	case t.TypeArgs().Len() > 0:
		return "", fmt.Errorf("its type holds %s, an instance of a generic type, which a fact cannot copy", t)
	case nameable(t, w.program) && fieldsExported(t):
		return packageMark + object.Pkg().Path() + packageMark + "." + object.Name(), nil
	}
	name := object.Name()
	if earlier, taken := w.copied[name]; taken {
		if earlier != t {
			return "", fmt.Errorf("its value has two types named %s, which a fact cannot copy side by side", name)
		}
		return name, nil
	}
	w.copied[name] = t
	underlying, err := w.write(t.Underlying())
	if err == nil {
		w.declarations = append(w.declarations, markedSource("type "+name+" "+underlying))
	}
	return name, err
}

// fieldsExported says whether a type is no struct, or a struct whose fields are all
// exported: one a program of another package can write a literal of.
func fieldsExported(t *types.Named) bool {
	fields, isStruct := t.Underlying().(*types.Struct)
	if !isStruct {
		return true
	}
	for k := range fields.NumFields() {
		if !fields.Field(k).Exported() {
			return false
		}
	}
	return true
}

func (w *typeWriter) mapType(t *types.Map) (string, error) {
	key, err := w.write(t.Key())
	if err != nil {
		return "", err
	}
	item, err := w.write(t.Elem())
	return "map[" + key + "]" + item, err
}

func (w *typeWriter) structType(t *types.Struct) (string, error) {
	fields := make([]string, t.NumFields())
	for k := range t.NumFields() {
		field := t.Field(k)
		typeName, err := w.write(field.Type())
		if err != nil {
			return "", err
		}
		fields[k] = field.Name() + " " + typeName
		if field.Embedded() {
			fields[k] = typeName
		}
	}
	return "struct{" + strings.Join(fields, "; ") + "}", nil
}
