/**
 * Bottom surface: "Does Claude need me?" (PRD §6.2, §9).
 * Four vertically stacked lights; exactly one is lit for the current state.
 * Clicking reveals a small card with the reason and an "Open Claude Code" action.
 * Motion is applied by the Nudge component through data-motion / fx-* classes.
 */
import type { ActivitySnapshot } from '../../state/State';
import type { PresentationState } from '../../state/ClaudeStateMachine';
import { describeActivity } from './describe';
import './TrafficSignal.css';

/** Top to bottom: green, amber, white, red. */
export const LIGHT_ORDER = ['working', 'waiting', 'done', 'error'] as const;

export interface TrafficSignalOptions {
  onOpenClaude: () => void;
}

export class TrafficSignal {
  readonly element: HTMLElement;
  /** The four-light button. Nudge applies motion/effect classes here. */
  readonly lights: HTMLButtonElement;

  private readonly card: HTMLElement;
  private readonly cardState: HTMLElement;
  private readonly cardMeta: HTMLElement;
  private readonly cardAction: HTMLButtonElement;
  private readonly cardNote: HTMLElement;
  private noteTimer: ReturnType<typeof setTimeout> | null = null;

  constructor(private readonly opts: TrafficSignalOptions) {
    this.element = el('section', 'signal-wrap');

    this.lights = document.createElement('button');
    this.lights.type = 'button';
    this.lights.className = 'signal surface';
    this.lights.dataset.state = 'idle';
    this.lights.dataset.motion = 'none';
    this.lights.setAttribute('aria-label', 'Claude status: Idle');
    this.lights.setAttribute('aria-expanded', 'false');
    for (const light of LIGHT_ORDER) {
      const dot = el('span', `light light--${light}`);
      dot.dataset.light = light;
      this.lights.append(dot);
    }

    this.card = el('div', 'signal-card surface');
    this.card.hidden = true;
    const title = el('div', 'signal-card__title');
    title.textContent = 'Claude';
    this.cardState = el('div', 'signal-card__state');
    this.cardMeta = el('div', 'signal-card__meta');
    this.cardAction = document.createElement('button');
    this.cardAction.type = 'button';
    this.cardAction.className = 'signal-card__action';
    this.cardAction.textContent = 'Open Claude Code →';
    this.cardNote = el('div', 'signal-card__note');
    this.card.append(title, this.cardState, this.cardMeta, this.cardAction, this.cardNote);

    this.element.append(this.lights, this.card);

    this.lights.addEventListener('click', () => this.toggle());
    this.cardAction.addEventListener('click', () => this.opts.onOpenClaude());
    document.addEventListener('keydown', (e) => {
      if (e.key === 'Escape') this.toggle(false);
    });
  }

  update(p: PresentationState, activity: ActivitySnapshot): void {
    this.lights.dataset.state = p.state;
    this.lights.setAttribute('aria-label', `Claude status: ${p.label}`);
    this.updateDetails(p, activity);
  }

  updateDetails(p: PresentationState, activity: ActivitySnapshot): void {
    this.cardState.textContent = p.label;
    this.cardState.dataset.state = p.state;
    this.cardMeta.textContent = describeActivity(activity);
    this.cardAction.disabled = activity.focusSessionId === null;
  }

  toggle(force?: boolean): void {
    const open = force ?? this.card.hidden;
    this.card.hidden = !open;
    this.lights.setAttribute('aria-expanded', String(open));
    this.element.classList.toggle('is-open', open);
  }

  /** Transient message inside the card (e.g. focus failed). */
  flashMessage(text: string): void {
    this.toggle(true);
    this.cardNote.textContent = text;
    if (this.noteTimer !== null) clearTimeout(this.noteTimer);
    this.noteTimer = setTimeout(() => {
      this.cardNote.textContent = '';
    }, 4000);
  }
}

function el<K extends keyof HTMLElementTagNameMap>(tag: K, className: string): HTMLElementTagNameMap[K] {
  const node = document.createElement(tag);
  node.className = className;
  return node;
}
