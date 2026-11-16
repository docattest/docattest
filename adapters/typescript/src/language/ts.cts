// The TypeScript compiler, loaded with `require`. Imported from an ES module as itself,
// `typescript` (nine megabytes of CommonJS) is first scanned by Node for the names it
// exports, which takes longer than loading it; through this module it is not, and an
// adapter process starts about a tenth of a second sooner. Import `ts` from here.
// eslint-disable-next-line @typescript-eslint/no-require-imports -- the point of this module
import ts = require('typescript');
export = ts;
