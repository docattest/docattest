/**
 * The playground: a list of stops down the side; at each, the project's files (with yellow
 * spots to click and change) and a terminal whose commands replay what the built docattest
 * printed for exactly those files.
 */
import '../theme.css';
import './playground.css';
import './panes.css';
import type { RecordedState, RecordedStop, Recordings } from './recordingFormat.ts';
import { RECORDINGS } from './recordings.ts';
import {
  PAGE,
  pageMarks,
  stampFor,
  terminalLines,
  type PageMark,
  type Stamp,
  type TerminalLine,
} from './report.ts';
import { PAGE_STOPS, type PageCommand, type PageStop } from './stops.ts';

/** Where the visitor is and what they have done there: the playground's one piece of state. */
interface View {
  readonly stop: PageStop;
  readonly recorded: RecordedStop;
  /** The chosen option of each of the stop's switches. */
  readonly chosen: readonly number[];
  readonly file: string;
  /** Files a command has written, offered from then on. */
  readonly revealed: readonly string[];
  readonly marks: readonly PageMark[];
  /** The id of the command whose output the terminal shows. */
  readonly ran: string | null;
}

interface Parts {
  readonly stops: HTMLElement;
  readonly who: HTMLImageElement;
  readonly task: HTMLElement;
  readonly hint: HTMLElement;
  readonly tabs: HTMLElement;
  readonly code: HTMLElement;
  readonly commands: HTMLElement;
  readonly output: HTMLElement;
  readonly exit: HTMLElement;
}

const ALWAYS_SHOWN = ['docs/retries.md', 'src/index.ts'];
const LINE_DELAY_MS = 60;
const IDLE = {
  fresh: 'Nothing run yet. Press a command above.',
  changed: 'You changed a file. Press a command to see what docattest says now.',
};
const FILE_HINT = {
  some: 'Click a yellow part to change it',
  none: 'Nothing to change on this stop',
};
/* A switch's place in a file while its text is being laid out: \uE000 index \uE001. */
const SPOT = /(\uE000\d+\uE001)/;
const DIRECTIVE = /(<!--.*?-->)/;
/* How far above a marked line the file viewer stops, so the line has context. */
const SCROLL_LEAD_PX = 90;

startPlayground(document, PAGE_STOPS, RECORDINGS, location.hash.slice(1));

/** Opens at the stop the URL's fragment names (playground.html#fact), or the first. */
function startPlayground(
  root: ParentNode,
  stops: readonly PageStop[],
  recordings: Recordings,
  openingId: string,
) {
  const parts = findParts(root);
  const first = stops.find((stop) => stop.id === openingId) ?? stops[0];
  if (first === undefined) throw new Error('the playground has no stops');
  let view = arrivingAt(first, recordings);

  const draw = () => {
    drawStops(parts.stops, stops, view.stop, open);
    drawTask(parts, view);
    const showFile = (file: string) => {
      view = { ...view, file };
      draw();
    };
    drawTabs(parts.tabs, tabsOf(view), view, showFile);
    drawCode(parts.code, fileLines(recordings, view), view, flip);
    drawCommands(parts.commands, view, execute);
  };
  function open(stop: PageStop) {
    view = arrivingAt(stop, recordings);
    drawIdleTerminal(parts, IDLE.fresh);
    draw();
  }
  function flip(spot: number) {
    const count = view.recorded.switches[spot]?.options.length ?? 1;
    const chosen = view.chosen.map((option, index) =>
      index === spot ? (option + 1) % count : option,
    );
    view = { ...view, chosen, marks: [], ran: null };
    drawIdleTerminal(parts, IDLE.changed);
    draw();
  }
  function execute(command: PageCommand) {
    const output = outputOf(stateOf(view), command);
    const marks = pageMarks(output);
    const { reveals } = command;
    const revealed =
      reveals === undefined || view.revealed.includes(reveals)
        ? view.revealed
        : [...view.revealed, reveals];
    const file = reveals ?? (marks.length > 0 ? PAGE : view.file);
    view = { ...view, marks, revealed, file, ran: command.id };
    drawTerminal(parts, terminalLines(output), stampFor(command, output), output.exitCode);
    draw();
  }

  draw();
  const firstCommand = first.commands[0];
  if (firstCommand !== undefined) execute(firstCommand);
}

function findParts(root: ParentNode): Parts {
  return {
    stops: part(root, 'stops', HTMLElement),
    who: part(root, 'who', HTMLImageElement),
    task: part(root, 'task', HTMLElement),
    hint: part(root, 'file-hint', HTMLElement),
    tabs: part(root, 'tabs', HTMLElement),
    code: part(root, 'code-lines', HTMLElement),
    commands: part(root, 'commands', HTMLElement),
    output: part(root, 'output', HTMLElement),
    exit: part(root, 'exit', HTMLElement),
  };
}

function part<Kind extends HTMLElement>(root: ParentNode, id: string, kind: new () => Kind): Kind {
  const found = root.querySelector(`#${id}`);
  if (!(found instanceof kind)) throw new Error(`the playground page has no #${id}`);
  return found;
}

function arrivingAt(stop: PageStop, recordings: Recordings): View {
  const recorded = recordings.stops[stop.id];
  if (recorded === undefined) throw new Error(`no recordings for the stop ${stop.id}`);
  return {
    stop,
    recorded,
    chosen: recorded.switches.map(() => 0),
    file: recorded.switches[0]?.file ?? PAGE,
    revealed: [],
    marks: [],
    ran: null,
  };
}

function stateOf(view: View): RecordedState {
  const state = view.recorded.states[view.chosen.join('')];
  if (state === undefined) {
    throw new Error(`no recording of ${view.stop.id} as ${view.chosen.join('')}`);
  }
  return state;
}

function outputOf(state: RecordedState, command: PageCommand) {
  const output = state.outputs[command.id];
  if (output === undefined) throw new Error(`no recording of the command ${command.id}`);
  return output;
}

function tabsOf(view: View): string[] {
  const files = [
    ...view.recorded.switches.map((spot) => spot.file),
    ...ALWAYS_SHOWN,
    ...(view.stop.files ?? []),
    ...view.revealed,
  ];
  return [...new Set(files)];
}

function drawStops(
  list: HTMLElement,
  stops: readonly PageStop[],
  current: PageStop,
  open: (stop: PageStop) => void,
) {
  const items = stops.flatMap((stop, index) => {
    const button = element('button', stop === current ? 'stop current' : 'stop');
    button.type = 'button';
    button.append(element('b', undefined, stop.name), element('span', undefined, stop.shows));
    if (stop === current) button.setAttribute('aria-current', 'step');
    button.addEventListener('click', () => {
      open(stop);
    });
    const item = element('li');
    item.append(button);
    const startsGroup = stops[index - 1]?.group !== stop.group;
    return startsGroup ? [element('li', 'stop-group', stop.group), item] : [item];
  });
  list.replaceChildren(...items);
}

function drawTask(parts: Parts, view: View) {
  const image = `${import.meta.env.BASE_URL}img/${view.stop.who.image}.png`;
  parts.task.textContent = view.stop.task;
  if (parts.who.getAttribute('src') !== image) parts.who.src = image;
  parts.who.alt = view.stop.who.alt;
  parts.hint.textContent = view.recorded.switches.length > 0 ? FILE_HINT.some : FILE_HINT.none;
}

function drawTabs(
  tabs: HTMLElement,
  files: readonly string[],
  view: View,
  show: (file: string) => void,
) {
  const buttons = files.map((file) => {
    const button = element('button', file === view.file ? 'tab current' : 'tab', file);
    button.type = 'button';
    button.setAttribute('aria-pressed', String(file === view.file));
    if (view.recorded.switches.some((spot) => spot.file === file)) button.classList.add('has-spot');
    button.addEventListener('click', () => {
      show(file);
    });
    return button;
  });
  tabs.replaceChildren(...buttons);
  tabs.querySelector('.current')?.scrollIntoView({ block: 'nearest', inline: 'nearest' });
}

/** The open file's lines, with each switch's place held by a SPOT marker. */
function fileLines(recordings: Recordings, view: View): string[] {
  const original = recordings.project[view.file] ?? stateOf(view).written[view.file];
  if (original === undefined) throw new Error(`no file ${view.file} at the stop ${view.stop.id}`);
  const edited = view.recorded.given
    .filter((edit) => edit.file === view.file)
    .reduce((text, edit) => text.replace(edit.find, edit.replace), original);
  const spotted = view.recorded.switches.reduce((text, spot, index) => {
    const [first] = spot.options;
    return spot.file === view.file && first !== undefined
      ? text.replace(first.text, `\uE000${index}\uE001`)
      : text;
  }, edited);
  return spotted.trimEnd().split('\n');
}

function drawCode(
  code: HTMLElement,
  lines: readonly string[],
  view: View,
  flip: (spot: number) => void,
) {
  const cells = lines.flatMap((line, index) => {
    const mark =
      view.file === PAGE ? view.marks.find((candidate) => candidate.line === index + 1) : undefined;
    const gutter =
      mark === undefined
        ? element('span', 'gutter-cell', String(index + 1))
        : element('span', `gutter-cell mark mark-${mark.tone}`, mark.symbol);
    const text = element('span', 'code-cell');
    text.append(
      ...line
        .split(SPOT)
        .flatMap((piece) =>
          SPOT.test(piece) ? [spotButton(piece, view, flip)] : withDirectives(piece),
        ),
    );
    return [gutter, text];
  });
  code.replaceChildren(...cells);
  const target = code.querySelector<HTMLElement>('.mark-bad, .spot');
  const scroller = code.parentElement;
  if (scroller !== null) {
    scroller.scrollTop = target === null ? 0 : Math.max(0, target.offsetTop - SCROLL_LEAD_PX);
  }
}

function spotButton(marker: string, view: View, flip: (spot: number) => void): HTMLButtonElement {
  const index = Number(marker.slice(1, -1));
  const chosen = view.chosen[index] ?? 0;
  const option = view.recorded.switches[index]?.options[chosen];
  const words = option?.text === '' ? (option.shown ?? '') : (option?.text.trim() ?? '');
  const button = element('button', chosen === 0 ? 'spot' : 'spot flipped', words);
  button.type = 'button';
  button.title = 'Click to change this';
  button.addEventListener('click', () => {
    flip(index);
  });
  return button;
}

function withDirectives(text: string): (string | HTMLElement)[] {
  return text
    .split(DIRECTIVE)
    .map((piece) => (DIRECTIVE.test(piece) ? element('span', 'directive', piece) : piece));
}

function drawCommands(container: HTMLElement, view: View, execute: (command: PageCommand) => void) {
  const state = stateOf(view);
  const buttons = view.stop.commands.map((command) => {
    const label = command.label ?? outputOf(state, command).command.replace(/^npx /, '');
    const button = element('button', command.label === undefined ? 'run' : 'run run-scene', label);
    button.type = 'button';
    if (command.id === view.ran) button.classList.add('current');
    button.addEventListener('click', () => {
      execute(command);
    });
    return button;
  });
  container.replaceChildren(...buttons);
}

function drawTerminal(
  parts: Parts,
  lines: readonly TerminalLine[],
  stamp: Stamp | null,
  exitCode: number,
) {
  const rows = lines.map((line, index) => {
    const row = element('span', `line ${line.tone}`, line.text);
    row.style.animationDelay = `${index * LINE_DELAY_MS}ms`;
    return row;
  });
  parts.output.replaceChildren(...rows);
  if (stamp !== null) {
    const badge = element('span', stamp.tone === 'good' ? 'stamp stamp-good' : 'stamp', stamp.text);
    badge.style.animationDelay = `${rows.length * LINE_DELAY_MS}ms`;
    badge.setAttribute('aria-hidden', 'true');
    parts.output.append(badge);
  }
  parts.exit.textContent = `exit ${exitCode}`;
  parts.exit.hidden = false;
}

function drawIdleTerminal(parts: Parts, message: string) {
  parts.output.replaceChildren(element('span', 'line quiet', message));
  parts.exit.hidden = true;
}

function element<Tag extends keyof HTMLElementTagNameMap>(
  tag: Tag,
  className?: string,
  text?: string,
): HTMLElementTagNameMap[Tag] {
  const node = document.createElement(tag);
  if (className !== undefined) node.className = className;
  if (text !== undefined) node.textContent = text;
  return node;
}
