// The environment the smoke test installs and runs in: nothing of the user's or of this
// checkout's, so that what passes would pass on a fresh machine.
import process from 'node:process';
import { mkdirSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { WINDOWS } from './machine.ts';

/**
 * This process's environment with nothing of docattest's, npm's or a virtualenv's in it, and
 * a home, caches and npm settings of its own under `work`: no user's cache (the launchers'
 * shared one included), no ~/.npmrc, no global prefix.
 */
export function cleanEnvironment(work: string): NodeJS.ProcessEnv {
  const dropped = (key: string): boolean =>
    key.startsWith('DOCATTEST_') ||
    /^npm_/i.test(key) ||
    ['VIRTUAL_ENV', 'PYTHONPATH', 'PYTHONHOME', 'NODE_OPTIONS', 'NODE_PATH'].includes(key);
  const env = Object.fromEntries(Object.entries(process.env).filter(([key]) => !dropped(key)));
  const home = join(work, 'home');
  mkdirSync(join(home, 'AppData', 'Local'), { recursive: true });
  writeFileSync(join(work, 'npmrc'), '');
  return {
    ...env,
    HOME: home,
    USERPROFILE: home,
    LOCALAPPDATA: join(home, 'AppData', 'Local'),
    XDG_CACHE_HOME: join(home, '.cache'),
    npm_config_cache: join(work, 'npm-cache'),
    npm_config_userconfig: join(work, 'npmrc'),
    npm_config_prefix: join(work, 'npm-prefix'),
    npm_config_audit: 'false',
    npm_config_fund: 'false',
    npm_config_update_notifier: 'false',
    PIP_CACHE_DIR: join(work, 'pip-cache'),
    PIP_CONFIG_FILE: WINDOWS ? 'NUL' : '/dev/null',
    PIP_DISABLE_PIP_VERSION_CHECK: '1',
  };
}
