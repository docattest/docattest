// The development set. Positive scenarios are drift (or a way of hiding it) the tool
// should surface; negative scenarios are ordinary states it must stay silent on.
// Builders may read this file and must not edit it.
//
// Each part is a file in scenarios/; they are joined here in the order the scenarios run
// and are recorded in (tests/parity/recordings/scenarios/).
import { scenarios as audit } from './scenarios/audit.ts';
import { scenarios as guard } from './scenarios/guard.ts';
import { scenarios as impact } from './scenarios/impact.ts';
import { scenarios as citations } from './scenarios/citations.ts';
import { scenarios as writtenShapes } from './scenarios/written-shapes.ts';
import type { Scenario } from './scenario.ts';

export const scenarios: Scenario[] = [
  ...audit,
  ...guard,
  ...impact,
  ...citations,
  ...writtenShapes,
];
