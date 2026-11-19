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
Citing a benchmark is an error that says so. The file is read, not run, unless the check
runs the cited tests ([running cited tests](#running-cited-tests)). What a cited test uses
is read as for any attestation ([what an attestation uses](../README.md#configure)), also
in a file behind a build constraint (`//go:build integration`), which is read as if it had
none. <!-- test: internal/analysis/symbols_test.go > "TestSymbolsAreResolvedByType" -->

## Running cited tests

`docattest check --run-cited` also runs each cited test. The cited tests of one package,
the directory of their test file with its external test package (`package p_test`)
included, run in one invocation of `go test -json -count=1 -run '<pattern>' .` in that
directory, with the go command and the workspace that run examples; as many packages run at
once as the check's workers. A test passes when `go test` reports that it passed; one that
fails fails its citation with the lines it wrote, and the lines of its subtests that
failed, without the `=== RUN` and `--- FAIL` lines around them; one that calls `t.Skip`
is skipped, with the reason it gives; one `go test` reports nothing of, such as a test the
file declares that the package no longer has, did not run. <!-- test: internal/citedtests/citedtests_test.go > "TestCitedTestsRunInTheirPackages" -->

The pattern selects the cited tests and no other: one alternative for each test, each name
in its path anchored and quoted (`^TestRounding$/^rounds_up$`), and each subtest's name
rewritten as `go test` writes it: a space becomes `_`, a rune that does not print is
escaped (`\x00`), and an empty name is `#00`. <!-- test: internal/citedtests/selection_test.go > "TestReportedNamesAreRewrittenAsTestingRewritesThem" -->
A subtest whose name merely begins with the cited one's, or that an unquoted name would
match, does not run, and neither do the other subtests of its test. <!-- test: internal/citedtests/citedtests_test.go > "TestOnlyTheCitedTestRuns" -->
A test with subtests passes when it passes, which `go test` reports only when every
subtest it ran passed. <!-- test: internal/citedtests/judge_test.go > "TestEachTestIsJudgedByItsOwnEvents" -->

Every cited test of a package that does not build fails with the build's errors, which go
1.21 to 1.23 write to the standard error and go 1.24 and later report as events of their
own. <!-- test: internal/citedtests/judge_test.go > "TestEveryTestOfAPackageThatDoesNotBuildFailsWithTheBuildErrors" -->
An invocation that does not end in time is stopped, with every program it started, and
each of its tests fails, saying so. <!-- test: internal/citedtests/citedtests_test.go > "TestARunThatDoesNotEndInTimeFailsItsTests" -->
Without a go command no test runs, and each says why. <!-- test: internal/citedtests/judge_test.go > "TestARunThatEndsBeforeItsTestsFailsOrDoesNotRunThemAll" -->
In a project given in memory nothing runs at all. <!-- test: internal/session/session_test.go > "TestRunTestsRunsNothingInMemory" -->
