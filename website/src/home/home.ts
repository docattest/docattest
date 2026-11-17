/**
 * The home page's small movements: the hero demo that flips between stale and true docs,
 * characters that answer back when clicked, commands copied on a click, cards that pop in
 * as they scroll into view, and the language tabs of the setup steps.
 */
import '../theme.css';
import './layout.css';
import './hero.css';
import './sections.css';

const TOAST_MS = 1400;
const JUMP_MS = 500;
const DEMO_LABELS = { stale: 'Fine, fix the docs', true: 'Break it again' } as const;
type DemoState = keyof typeof DEMO_LABELS;

startDemo(required('demo'), required('demo-toggle'));
startTalkers(document.querySelectorAll<HTMLElement>('.talker'));
startCopying(document.querySelectorAll<HTMLElement>('.command'), required('toast'));
startPops(document.querySelectorAll('.pop'));
startLanguageTabs(required('setup'));

function required(id: string): HTMLElement {
  const found = document.getElementById(id);
  if (found === null) throw new Error(`the home page has no #${id}`);
  return found;
}

function startDemo(demo: HTMLElement, toggle: HTMLElement) {
  const show = (state: DemoState) => {
    demo.dataset.state = state;
    toggle.textContent = DEMO_LABELS[state];
    for (const claim of demo.querySelectorAll<HTMLElement>('.claim[data-stale]')) {
      claim.textContent = claim.dataset[state] ?? '';
    }
    const terminal = demo.querySelector('.pane-term');
    if (terminal !== null) replay(terminal);
  };
  const current = (): DemoState => (demo.dataset.state === 'true' ? 'true' : 'stale');
  toggle.addEventListener('click', () => {
    show(current() === 'stale' ? 'true' : 'stale');
  });
}

/* Detaching and re-attaching a node is what restarts its CSS animations. */
function replay(node: Element) {
  const next = node.nextSibling;
  const parent = node.parentNode;
  node.remove();
  parent?.insertBefore(node, next);
}

function startTalkers(talkers: Iterable<HTMLElement>) {
  for (const talker of talkers) {
    const lines = (talker.dataset.lines ?? '').split('|');
    const bubble = talker.querySelector('.bubble');
    let spoken = 0;
    talker.addEventListener('click', () => {
      spoken = (spoken + 1) % lines.length;
      if (bubble !== null) bubble.textContent = lines[spoken] ?? '';
      talker.classList.add('jump');
      setTimeout(() => {
        talker.classList.remove('jump');
      }, JUMP_MS);
    });
  }
}

function startCopying(commands: Iterable<HTMLElement>, toast: HTMLElement) {
  let hiding: ReturnType<typeof setTimeout> | undefined;
  const announce = (message: string) => {
    toast.textContent = message;
    toast.hidden = false;
    clearTimeout(hiding);
    hiding = setTimeout(() => {
      toast.hidden = true;
    }, TOAST_MS);
  };
  for (const command of commands) {
    command.addEventListener('click', () => {
      navigator.clipboard.writeText(command.textContent).then(
        () => {
          announce('Copied');
        },
        () => {
          getSelection()?.selectAllChildren(command);
          announce('Selected. Copy it yourself, champ.');
        },
      );
    });
  }
}

function startPops(pops: Iterable<Element>) {
  const seen = new IntersectionObserver(
    (entries) => {
      for (const entry of entries.filter((candidate) => candidate.isIntersecting)) {
        entry.target.classList.add('seen');
        seen.unobserve(entry.target);
      }
    },
    { threshold: 0.4 },
  );
  for (const pop of pops) seen.observe(pop);
}

/* Each tab names the panel it shows (aria-controls); choosing one hides the others. */
function startLanguageTabs(setup: HTMLElement) {
  const tabs = [...setup.querySelectorAll<HTMLElement>('[role="tab"]')];
  for (const tab of tabs) {
    tab.addEventListener('click', () => {
      for (const other of tabs) {
        const chosen = other === tab;
        other.setAttribute('aria-selected', String(chosen));
        const panel = document.getElementById(other.getAttribute('aria-controls') ?? '');
        if (panel !== null) panel.hidden = !chosen;
      }
    });
  }
}
