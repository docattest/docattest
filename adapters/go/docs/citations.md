# Citations in Go

What a citation of a test names in a Go test file. The [guide](../README.md) has the rest.

`<!-- test: shop_test.go > "TestTotals" > "rejects big discounts" -->` ties a sentence to
a test. A test is a `Test`, `Fuzz` or `Example` function, titled by its name, or a subtest
started with `t.Run("a literal name", …)`, titled by that name, with the functions and
subtests that enclose it named first, outermost first.

A table-driven test's subtests can be cited too, by the names its table writes. The name
given to `t.Run` is the key `k`, or a field `v.name`, of an enclosing
`for k, v := range table`, where the table is a composite literal written in the loop or
assigned once to a variable of the test function (`tests := []struct{…}{…}`,
`cases := map[string]T{…}`). Each element then names a subtest: its map key, or its field,
written with the field's name or in its position in the struct type of the literal, when
that is a string literal. Any other computed name cannot be cited: a table a function
returns, a variable assigned again (`tests = append(tests, …)`), an element whose name is
not a literal. <!-- test: internal/analysis/analysis_test.go > "TestSubtestsOfTableDrivenTests" -->
Citing a benchmark is an error that says so. The file is read, never run. What a cited test
uses is read as for any attestation ([what an attestation uses](../README.md#configure)),
also in a file behind a build constraint (`//go:build integration`), which is read as if it
had none. <!-- test: internal/analysis/symbols_test.go > "TestSymbolsAreResolvedByType" -->
