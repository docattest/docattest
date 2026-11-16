// `skill install`, in the project and `--global`, and its usage.
import type { ImpactCase } from './types.ts';

export const skillInstall: ImpactCase[] = [
  // ------------------------------------------------------------------------ skill install
  {
    name: 'skill install, then again',
    files: {},
    steps: [
      ['skill', 'install'],
      ['skill', 'install'],
      ['skill', 'install', '--force'],
    ],
  },
  {
    name: 'skill install over an edited copy',
    files: { '.claude/skills/docattest/SKILL.md': 'edited\n' },
    steps: [
      ['skill', 'install'],
      ['skill', 'install', '--force'],
      ['skill', 'install'],
    ],
  },
  {
    name: 'skill install over an empty copy',
    files: { '.claude/skills/docattest/SKILL.md': '' },
    steps: [
      ['skill', 'install'],
      ['skill', 'install', '--force'],
    ],
  },
  {
    name: 'skill install beside other skills',
    files: { '.claude/skills/other/SKILL.md': 'other\n', '.claude/settings.json': '{}' },
    steps: [['skill', 'install']],
  },
  {
    name: 'skill install --global',
    files: {},
    steps: [
      ['skill', 'install', '--global'],
      ['skill', 'install', '--global'],
      ['skill', 'install'],
    ],
  },
  {
    name: 'skill install --global over an edited copy',
    files: {},
    home: { '.claude/skills/docattest/SKILL.md': 'mine\n' },
    steps: [
      ['skill', 'install', '--global'],
      ['skill', 'install', '--global', '--force'],
    ],
  },
  {
    name: 'skill install --root',
    files: { 'sub/x': '' },
    steps: [
      ['skill', 'install', '--root', 'sub'],
      ['skill', 'install', '--root', '__ROOT__/sub', '--global'],
    ],
  },
  {
    name: 'skill install where the skill is a directory',
    files: { '.claude/skills/docattest/SKILL.md/x': '' },
    steps: [
      ['skill', 'install'],
      ['skill', 'install', '--force'],
    ],
  },
  {
    name: 'skill install where .claude is a file',
    files: { '.claude': 'x' },
    steps: [['skill', 'install']],
  },
  {
    name: 'skill usage',
    files: {},
    steps: [['skill'], ['skill', 'remove'], ['skill', 'install', 'x'], ['skill', '--global']],
  },
];
