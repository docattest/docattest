// `init --skill`, in the project and `--global`, and its usage.
import type { ImpactCase } from './types.ts';

export const skillInstall: ImpactCase[] = [
  // ------------------------------------------------------------------------ init --skill
  {
    name: 'skill install, then again',
    files: {},
    steps: [
      ['init', '--skill', '--no-audit'],
      ['init', '--skill'],
      ['init', '--skill', '--force', '--no-audit'],
    ],
  },
  {
    name: 'skill install over an edited copy',
    files: { '.claude/skills/docattest/SKILL.md': 'edited\n' },
    steps: [
      ['init', '--skill', '--no-audit'],
      ['init', '--skill', '--force', '--no-audit'],
      ['init', '--skill'],
    ],
  },
  {
    name: 'skill install over an empty copy',
    files: { '.claude/skills/docattest/SKILL.md': '' },
    steps: [
      ['init', '--skill', '--no-audit'],
      ['init', '--skill', '--force', '--no-audit'],
    ],
  },
  {
    name: 'skill install beside other skills',
    files: { '.claude/skills/other/SKILL.md': 'other\n', '.claude/settings.json': '{}' },
    steps: [['init', '--skill', '--no-audit']],
  },
  {
    name: 'skill install --global',
    files: {},
    steps: [
      ['init', '--skill', '--global', '--no-audit'],
      ['init', '--skill', '--global'],
      ['init', '--skill'],
    ],
  },
  {
    name: 'skill install --global over an edited copy',
    files: {},
    home: { '.claude/skills/docattest/SKILL.md': 'mine\n' },
    steps: [
      ['init', '--skill', '--global', '--no-audit'],
      ['init', '--skill', '--global', '--force', '--no-audit'],
    ],
  },
  {
    name: 'skill install --root',
    files: { 'sub/x': '' },
    steps: [
      ['init', '--skill', '--no-audit', '--root', 'sub'],
      ['init', '--skill', '--root', '__ROOT__/sub', '--global'],
    ],
  },
  {
    name: 'skill install where the skill is a directory',
    files: { '.claude/skills/docattest/SKILL.md/x': '' },
    steps: [
      ['init', '--skill', '--no-audit'],
      ['init', '--skill', '--force', '--no-audit'],
    ],
  },
  {
    name: 'skill install where .claude is a file',
    files: { '.claude': 'x' },
    steps: [['init', '--skill', '--no-audit']],
  },
  {
    name: 'skill usage',
    files: {},
    steps: [
      ['init', '--global'],
      ['init', '--local'],
      ['init', '--hooks', '--local', '--git-hook'],
    ],
  },
];
