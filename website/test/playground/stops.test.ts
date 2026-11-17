import assert from 'node:assert/strict';
import { describe, it } from 'node:test';
import { RECORDINGS } from '../../src/playground/recordings.ts';
import { PAGE_STOPS } from '../../src/playground/stops.ts';

/** The stops the page lists and the stops the recorder recorded are kept in two files: what
 *  the page says, and what was run. These hold them to each other. */
describe('the playground stops', () => {
  it('lists every recorded stop, once', () => {
    assert.deepEqual(
      PAGE_STOPS.map((stop) => stop.id),
      Object.keys(RECORDINGS.stops),
    );
  });

  it('has a recording of every command in every setting of the switches', () => {
    for (const stop of PAGE_STOPS) {
      const recorded = RECORDINGS.stops[stop.id];
      assert.ok(recorded, stop.id);
      const settings = recorded.switches.reduce((count, spot) => count * spot.options.length, 1);
      assert.equal(Object.keys(recorded.states).length, settings, stop.id);
      for (const [setting, state] of Object.entries(recorded.states)) {
        assert.deepEqual(
          Object.keys(state.outputs),
          stop.commands.map((command) => command.id),
          `${stop.id} ${setting}`,
        );
        for (const command of stop.commands.filter((each) => each.reveals !== undefined)) {
          assert.ok(command.reveals !== undefined && command.reveals in state.written, command.id);
        }
      }
    }
  });

  it('finds each switch where its file says it, exactly once', () => {
    for (const [id, recorded] of Object.entries(RECORDINGS.stops)) {
      for (const spot of recorded.switches) {
        const file = recorded.given
          .filter((edit) => edit.file === spot.file)
          .reduce(
            (text, edit) => text.replace(edit.find, edit.replace),
            RECORDINGS.project[spot.file] ?? '',
          );
        const original = spot.options[0]?.text ?? '';
        assert.equal(file.split(original).length, 2, `${id}: ${spot.file} ${original}`);
      }
    }
  });
});
