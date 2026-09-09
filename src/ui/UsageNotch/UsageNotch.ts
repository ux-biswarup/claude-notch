/**
 * Top surface: "How much Claude do I have?" (PRD §6.1).
 */
import type { UsageSnapshot } from '../../state/State';
import { formatUsage } from './format';
import './UsageNotch.css';

const RADIUS = 15.5;
const CIRCUMFERENCE = 2 * Math.PI * RADIUS;
const SVG_NS = 'http://www.w3.org/2000/svg';

export class UsageNotch {
  readonly element: HTMLElement;
  private readonly ringValue: SVGCircleElement;
  private readonly primary: HTMLElement;
  private readonly secondary: HTMLElement;

  constructor() {
    this.element = document.createElement('section');
    this.element.className = 'usage surface';
    this.element.dataset.tone = 'off';
    this.element.setAttribute('aria-live', 'polite');

    const label = document.createElement('div');
    label.className = 'usage__label';
    label.textContent = 'Claude';

    const row = document.createElement('div');
    row.className = 'usage__row';

    const svg = document.createElementNS(SVG_NS, 'svg');
    svg.setAttribute('class', 'ring');
    svg.setAttribute('viewBox', '0 0 36 36');
    svg.setAttribute('aria-hidden', 'true');
    const track = document.createElementNS(SVG_NS, 'circle');
    track.setAttribute('class', 'ring__track');
    this.ringValue = document.createElementNS(SVG_NS, 'circle');
    this.ringValue.setAttribute('class', 'ring__value');
    for (const c of [track, this.ringValue]) {
      c.setAttribute('cx', '18');
      c.setAttribute('cy', '18');
      c.setAttribute('r', String(RADIUS));
    }
    this.ringValue.style.strokeDasharray = String(CIRCUMFERENCE);
    this.ringValue.style.strokeDashoffset = String(CIRCUMFERENCE);
    svg.append(track, this.ringValue);

    const text = document.createElement('div');
    text.className = 'usage__text';
    this.primary = document.createElement('div');
    this.primary.className = 'usage__primary';
    this.secondary = document.createElement('div');
    this.secondary.className = 'usage__secondary';
    text.append(this.primary, this.secondary);

    row.append(svg, text);
    this.element.append(label, row);
  }

  update(usage: UsageSnapshot, nowMs = Date.now()): void {
    const view = formatUsage(usage, nowMs);
    this.element.dataset.tone = view.tone;
    this.element.title = view.tooltip;
    this.primary.textContent = view.primary;
    this.secondary.textContent = view.secondary ?? '';
    this.secondary.hidden = view.secondary === null;
    this.ringValue.style.strokeDashoffset = String(CIRCUMFERENCE * (1 - view.ratio));
  }
}
