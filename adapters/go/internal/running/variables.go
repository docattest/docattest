package running

import (
	"fmt"
	"go/ast"
	"go/types"
	"strings"

	"github.com/docattest/docattest/adapters/go/internal/analysis"
)

// A literal variable (analysis.LiteralVariable) is declared in the fact's function, as a
// constant is, with its value written out from what the type checker gives each of its
// constants: nothing imports its package and nothing of it runs. Its type, and each type
// its value names, is named where the program can name it (an exported type, of an
// importable package, whose fields are all exported) and copied otherwise: declared in
// the fact's function with the same name and the same underlying type, without its
// methods. A type is copied as far as it is made of basic types, named types, slices,
// arrays, maps, structs and `any`; a pointer, a function, a channel, an interface with
// methods or an instance of a generic type is not, and the variable cannot be read.

// sourceVariable is a literal variable as a generated program declares it.
type sourceVariable struct {
	// copied are the declarations of the types it copies, each before those that use it.
	copied []markedSource
	// statements declare it, and use it once, so that a fact that shadows it compiles.
	statements markedSource
}

// markedSource is Go source in which a package that a type is named from is its import
// path between two NULs: it is written once, and given the program's name for that
// package whenever a program is.
type markedSource string

// packageMark is what stands on each side of an import path in marked source.
const packageMark = "\x00"

// in is marked source with the name a program's imports give each package it names.
func (w markedSource) in(imports *programImports) string {
	parts := strings.Split(string(w), packageMark)
	for k := 1; k < len(parts); k += 2 {
		parts[k] = imports.alias(parts[k])
	}
	return strings.Join(parts, "")
}

// readVariable writes the declaration of a literal variable for programs of the module;
// an error says why it cannot be written.
func (r *Runner) readVariable(read analysis.LiteralVariable) (sourceVariable, error) {
	w := &valueWriter{info: read.Info, types: newTypeWriter(r.programPath())}
	name := read.Object.Name()
	typeName, err := w.types.write(read.Object.Type())
	if err != nil {
		return sourceVariable{}, err
	}
	value, err := w.value(read.Value, read.Object.Type())
	if err != nil {
		return sourceVariable{}, err
	}
	statements := fmt.Sprintf("var %s %s = %s; _ = %s", name, typeName, value, name)
	return sourceVariable{copied: w.types.declarations, statements: markedSource(statements)}, nil
}

// valueWriter writes the value of a literal variable.
type valueWriter struct {
	info  *types.Info
	types *typeWriter
}

// value writes a part of a value where a value of type `into` goes: a constant as its
// literal, given its own type where that is an interface, or a composite literal.
func (w *valueWriter) value(part ast.Expr, into types.Type) (string, error) {
	part = ast.Unparen(part)
	typed := w.info.Types[part]
	if typed.Value == nil {
		return w.composite(part.(*ast.CompositeLit), typed.Type)
	}
	text := literal(typed.Value, typed.Type)
	if !types.IsInterface(into) {
		return text, nil
	}
	typeName, err := w.types.write(types.Default(typed.Type))
	return typeName + "(" + text + ")", err
}

// composite writes a composite literal of a type, with its type.
func (w *valueWriter) composite(literal *ast.CompositeLit, of types.Type) (string, error) {
	typeName, err := w.types.write(of)
	if err != nil {
		return "", err
	}
	items := make([]string, len(literal.Elts))
	for k, element := range literal.Elts {
		if items[k], err = w.element(element, k, of); err != nil {
			return "", err
		}
	}
	return typeName + "{" + strings.Join(items, ", ") + "}", nil
}

// element writes the k-th element of a composite literal of a type, with its key.
func (w *valueWriter) element(element ast.Expr, k int, of types.Type) (string, error) {
	pair, keyed := element.(*ast.KeyValueExpr)
	if fields, isStruct := of.Underlying().(*types.Struct); isStruct {
		if !keyed {
			return w.value(element, fields.Field(k).Type())
		}
		name := pair.Key.(*ast.Ident).Name
		return w.keyed(name, pair.Value, fieldNamed(fields, name).Type())
	}
	items := of.Underlying().(interface{ Elem() types.Type }).Elem()
	if !keyed {
		return w.value(element, items)
	}
	var keyType types.Type = types.Typ[types.Int]
	if keys, isMap := of.Underlying().(*types.Map); isMap {
		keyType = keys.Key()
	}
	key, err := w.value(pair.Key, keyType)
	if err != nil {
		return "", err
	}
	return w.keyed(key, pair.Value, items)
}

// keyed writes an element that has a key.
func (w *valueWriter) keyed(key string, value ast.Expr, into types.Type) (string, error) {
	text, err := w.value(value, into)
	return key + ": " + text, err
}

// fieldNamed is the field of a struct that has a name.
func fieldNamed(fields *types.Struct, name string) *types.Var {
	for k := range fields.NumFields() {
		if fields.Field(k).Name() == name {
			return fields.Field(k)
		}
	}
	panic("the type checker found no field " + name)
}
