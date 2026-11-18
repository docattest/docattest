# Examples in Go

What a block of a page and a testable example of a source file are to the Go adapter: how
each is built and run, and what proves it. The [guide](../README.md) has the rest.

## Blocks: `go check`, `go compile`

Blocks tagged `go` or `golang` are the adapter's. <!-- fact: Hello().BlockLanguages -->
A block is compiled as a program of its own, inside your module, so it imports your
packages by their import paths and uses your module's dependencies. Nothing is written into
your project: generated files reach the go command through a build overlay.

- **What a block is.** One of three things, told apart by what parses:
  - *a whole file*, when it begins with a package clause. To run, it must be
    `package main` with a `func main`;
  - *declarations*: imports, then functions, types, variables. It is given an empty
    `main` when it declares none;
  - *statements*: imports, then statements, which become the body of `main`. This is the
    usual shape of a snippet in prose.
- **`check`** builds the block and runs it. The proof is what proves a testable example in
  Go: a last comment `// Output:` (or `// Unordered output:`), which what the block prints
  must match, compared as `go test` compares it. A block also fails when it panics or
  exits with a failure, so `if got != want { panic(…) }` is an assertion too; the report
  gives the panic and the line of the block it came from. <!-- test: internal/running/running_test.go > "TestBlocksCompileAndRun" -->
  A block that states no output and calls nothing that ends the program runs, but is
  counted as *weak*.
- **`compile`** builds the block and never runs it. The diagnostics are the Go compiler's
  own, at the lines of the block: an undefined name, a wrong argument, an unused variable
  or import. `check` blocks pass through the same build before they run.
- **Isolation.** Every block is its own package and its own process, started in the
  project root. Blocks share nothing, so each imports what it uses.
- **Time.** A block that outlives `runtime.timeoutMs` is killed with everything it
  started. `runtime.workers` is the number of blocks that run at once. Building is not
  counted: the blocks of a run are built together, once, before any of them runs.
- **Output.** What a block prints is compared or dropped; it never reaches the report.
- **Network.** Host names do not resolve unless the block is tagged `check network`
  (which runs only with `DOCATTEST_NETWORK=1`), so reaching the internet by name fails
  with an error that says why. Names in your hosts file (`localhost`) still resolve, and
  an address written as numbers is dialled as before: the guard replaces
  `net.DefaultResolver`, and code that carries its own resolver is not stopped.

The calls that end a program, which make a block that states no output an assertion:

<!-- cases: weak -->
| block               | weak |
|---------------------|------|
| `fmt.Println(1)`    | yes  |
| `panic("no")`       | no   |
| `log.Fatalf("no")`  | no   |
| `log.Panicln("no")` | no   |
| `os.Exit(1)`        | no   |

## Testable examples in source files

Go already has examples that are checked: the `Example` functions of a `_test.go` file.
Every one in a `sources` file is an attestation, run as `go test` runs it, by its name, in
its package's test program. <!-- test: internal/running/running_test.go > "TestDocExamplesRunAsGoTestRunsThem" -->

```go
// A discount is taken off the price in whole cents.
func ExampleApplyDiscount() {
	price, _ := ApplyDiscount(EUR(999), 50)
	fmt.Println(price)
	// Output: 5.00 EUR
}
```

The claim it defends is its own doc comment. `ExampleApplyDiscount` attests the export
`ApplyDiscount`, and `ExampleMoney_String` attests `Money`. An example that states no
output is compiled and not run, which is what `go test` does with it. The fixture's
`shop_test.go` has the example above, and one that states no output:

```console check cwd=testdata/fixture
$ docattest check shop_test.go
shop_test.go
  ✓ L10 example: printed the output it states
  ✓ L17 example: compiles
...
```
