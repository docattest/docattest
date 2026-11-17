/**
 * What the playground says at each stop. `id` names a stop the recorder recorded
 * (../../recorder/stops.ts), which holds the stop's switches and what every command printed
 * in every setting; the tests hold the two lists to each other.
 */
import type { StampWords } from './report.ts';

export interface PageStop {
  readonly id: string;
  /** The heading the stop is listed under. */
  readonly group: string;
  readonly name: string;
  /** What the stop showcases, in a few words. */
  readonly shows: string;
  readonly who: Character;
  /** What to do here, in one or two sentences. */
  readonly task: string;
  /** Files to offer besides the ones with switches, the docs page and the code. */
  readonly files?: readonly string[];
  readonly commands: readonly PageCommand[];
}

export interface Character {
  /** An image in public/img, without its extension. */
  readonly image: 'proud' | 'stamp' | 'stale' | 'bouncer' | 'detective' | 'agent';
  readonly alt: string;
}

export interface PageCommand extends StampWords {
  /** The button's words, when not the command itself. */
  readonly label?: string;
  /** A file the command writes, opened once it has run. */
  readonly reveals?: string;
}

const PAIGE: Character = { image: 'proud', alt: 'Paige, a happy proven page' };
const ATTIE: Character = { image: 'stamp', alt: 'Attie, a smug pink rubber stamp' };
const DALE: Character = { image: 'stale', alt: 'Dale, a grumpy yellowed page' };
const GATE: Character = { image: 'bouncer', alt: 'Gate, an unimpressed blue robot bouncer' };
const SNOOP: Character = {
  image: 'detective',
  alt: 'Snoop, a magnifying glass in a detective hat',
};
const BOLT: Character = { image: 'agent', alt: 'Bolt, a round mint robot with a wrench' };

export const PAGE_STOPS: readonly PageStop[] = [
  {
    id: 'meet',
    group: 'Start',
    name: 'Start here',
    shows: 'A healthy project, checked',
    who: PAIGE,
    task: 'This project is healthy. Press check and watch every proof on the docs page pass.',
    commands: [{ id: 'check' }],
  },
  {
    id: 'example',
    group: 'Kinds of proof',
    name: 'Example',
    shows: 'A code block that really runs',
    who: ATTIE,
    task:
      'Line 5 of the docs is real code. ' +
      'Break retry in src/index.ts by clicking the yellow part, then press check.',
    commands: [{ id: 'check' }],
  },
  {
    id: 'fact',
    group: 'Kinds of proof',
    name: 'Fact',
    shows: 'A number in a sentence, read from the code',
    who: DALE,
    task:
      'Click the limit in the code to make it 5, then press check. ' +
      'To fix it, click the sentence in the docs and check again.',
    commands: [{ id: 'check' }],
  },
  {
    id: 'cases',
    group: 'Kinds of proof',
    name: 'Cases',
    shows: 'A table where every row is a test',
    who: GATE,
    task:
      'Click the formula in the code so the wait stops doubling, then press check. ' +
      'Fix the table row in the docs and check again.',
    files: ['docs/docattest.setup.ts'],
    commands: [{ id: 'check' }],
  },
  {
    id: 'citation',
    group: 'Kinds of proof',
    name: 'Citation',
    shows: 'A sentence backed by a named test',
    who: SNOOP,
    task:
      'Rename the test by clicking its name, then press check. ' +
      'Fix it by citing the new name in the docs.',
    commands: [{ id: 'check' }],
  },
  {
    id: 'cheat',
    group: 'Guard rails',
    name: 'Deleted proof',
    shows: 'check --base notices a proof that went missing',
    who: GATE,
    task:
      'Raise the limit in the code. Then cheat: delete the proof in the docs instead of ' +
      'fixing the sentence. check passes. Now press check --base.',
    commands: [{ id: 'check' }, { id: 'check-base' }],
  },
  {
    id: 'find',
    group: 'Questions',
    name: 'Lookups',
    shows: 'where, index, uncovered, inventory, audit',
    who: SNOOP,
    task:
      'Nothing to change here. Press each command to ask what the docs say about the code. ' +
      'Arrows mark the lines they point at.',
    commands: [
      { id: 'where' },
      { id: 'index' },
      { id: 'uncovered' },
      { id: 'inventory' },
      { id: 'audit' },
    ],
  },
  {
    id: 'impact',
    group: 'Questions',
    name: 'Impact',
    shows: 'From a code change to the sentences it touches',
    who: DALE,
    task:
      'Click the limit in the code to change it, ' +
      'then press impact to see which sentences need a second look.',
    commands: [{ id: 'impact' }, { id: 'impact-file' }],
  },
  {
    id: 'git-hook',
    group: 'Hooks',
    name: 'Git hook',
    shows: 'Stop a commit when the docs are wrong',
    who: GATE,
    task:
      'The code already says 5 and the docs say 3. ' +
      'Install the hook, then try to commit. Fix the sentence and commit again.',
    commands: [
      { id: 'install', reveals: '.git/hooks/pre-commit' },
      { id: 'commit', refusedStamp: 'Blocked', passedStamp: 'Committed' },
    ],
  },
  {
    id: 'agent',
    group: 'Hooks',
    name: 'Agent hooks',
    shows: 'Keep a coding agent honest',
    who: BOLT,
    task:
      'The code already says 5 and the docs say 3. ' +
      'Run the three installs, then play the agent: edit a file, then try to stop.',
    commands: [
      { id: 'agents-install', reveals: 'AGENTS.md' },
      { id: 'hooks-install', reveals: '.claude/settings.json' },
      { id: 'skill-install' },
      { id: 'impact-hook', label: 'Agent edits a file' },
      {
        id: 'check-hook',
        label: 'Agent tries to stop',
        refusedStamp: 'Sent back',
        passedStamp: 'Free to go',
      },
    ],
  },
];
