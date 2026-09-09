/**
 * Minimal V1 settings (PRD §15). Rendered inside the main window and toggled
 * from the tray menu ("Settings…") or the `s` key in the browser preview.
 */
import type { Backend, HooksStatus } from '../../app/ipc';
import { DEFAULT_SETTINGS, type Settings } from '../../state/State';
import './Settings.css';

export interface SettingsPanelOptions {
  onChange: (settings: Settings) => void;
}

export class SettingsPanel {
  readonly element: HTMLElement;
  private readonly form: HTMLFormElement;
  private readonly hooksLine: HTMLElement;
  private readonly connectButton: HTMLButtonElement;
  private values: Settings = { ...DEFAULT_SETTINGS };

  constructor(
    private readonly backend: Backend,
    private readonly opts: SettingsPanelOptions,
  ) {
    this.element = document.createElement('section');
    this.element.className = 'settings surface';
    this.element.hidden = true;

    this.form = document.createElement('form');
    this.form.className = 'settings__form';
    this.form.innerHTML = `
      <header class="settings__header">
        <span class="settings__title">Claude Notch</span>
        <button type="button" class="settings__close" aria-label="Close settings">×</button>
      </header>

      <fieldset>
        <legend>Appearance</legend>
        <label><input type="radio" name="appearance" value="always" /> Always visible</label>
        <label><input type="radio" name="appearance" value="edge" disabled /> Edge trigger <small>(later)</small></label>
      </fieldset>

      <fieldset>
        <legend>Notifications</legend>
        <label><input type="checkbox" name="notifyDone" /> Task completed</label>
        <label><input type="checkbox" name="notifyWaiting" /> Needs input</label>
        <label><input type="checkbox" name="notifyError" /> Errors</label>
      </fieldset>

      <fieldset>
        <legend>Motion</legend>
        <label>Reduced motion
          <select name="reducedMotion">
            <option value="system">Follow Windows</option>
            <option value="on">Always</option>
            <option value="off">Never</option>
          </select>
        </label>
      </fieldset>

      <fieldset>
        <legend>Startup</legend>
        <label><input type="checkbox" name="launchOnSignIn" /> Launch on sign-in</label>
      </fieldset>

      <fieldset>
        <legend>Position</legend>
        <label><input type="radio" name="position" value="top-right" checked /> Top-right</label>
      </fieldset>

      <fieldset>
        <legend>Claude Code</legend>
        <div class="settings__hooks" data-role="hooks-line">Checking…</div>
        <button type="button" class="settings__connect" data-role="connect">Connect Claude Code</button>
      </fieldset>
    `;

    this.hooksLine = this.form.querySelector<HTMLElement>('[data-role="hooks-line"]')!;
    this.connectButton = this.form.querySelector<HTMLButtonElement>('[data-role="connect"]')!;
    this.element.append(this.form);

    this.form.addEventListener('change', () => void this.commit());
    this.form.querySelector('.settings__close')!.addEventListener('click', () => this.toggle(false));
    this.connectButton.addEventListener('click', () => void this.connect());
  }

  setValues(settings: Settings): void {
    this.values = { ...DEFAULT_SETTINGS, ...settings };
    const f = this.form.elements;
    (f.namedItem('appearance') as RadioNodeList).value = this.values.appearance;
    (f.namedItem('position') as RadioNodeList).value = this.values.position;
    (f.namedItem('notifyDone') as HTMLInputElement).checked = this.values.notifyDone;
    (f.namedItem('notifyWaiting') as HTMLInputElement).checked = this.values.notifyWaiting;
    (f.namedItem('notifyError') as HTMLInputElement).checked = this.values.notifyError;
    (f.namedItem('launchOnSignIn') as HTMLInputElement).checked = this.values.launchOnSignIn;
    (f.namedItem('reducedMotion') as HTMLSelectElement).value = this.values.reducedMotion;
  }

  toggle(force?: boolean): void {
    const open = force ?? this.element.hidden;
    this.element.hidden = !open;
    if (open) void this.refreshHooks();
  }

  private read(): Settings {
    const data = new FormData(this.form);
    return {
      ...this.values,
      appearance: (data.get('appearance') as Settings['appearance'] | null) ?? this.values.appearance,
      position: 'top-right',
      notifyDone: data.get('notifyDone') === 'on',
      notifyWaiting: data.get('notifyWaiting') === 'on',
      notifyError: data.get('notifyError') === 'on',
      launchOnSignIn: data.get('launchOnSignIn') === 'on',
      reducedMotion:
        (data.get('reducedMotion') as Settings['reducedMotion'] | null) ?? this.values.reducedMotion,
    };
  }

  private async commit(): Promise<void> {
    const next = this.read();
    try {
      this.values = await this.backend.setSettings(next);
    } catch (err) {
      console.error('failed to save settings', err);
      this.values = next;
    }
    this.opts.onChange(this.values);
  }

  private async refreshHooks(): Promise<void> {
    try {
      this.renderHooks(await this.backend.hooksStatus());
    } catch (err) {
      this.hooksLine.textContent = `Status unavailable: ${String(err)}`;
    }
  }

  private async connect(): Promise<void> {
    this.connectButton.disabled = true;
    try {
      this.renderHooks(await this.backend.installHooks());
    } catch (err) {
      this.hooksLine.textContent = `Could not install hooks: ${String(err)}`;
    } finally {
      this.connectButton.disabled = false;
    }
  }

  private renderHooks(status: HooksStatus): void {
    const state = status.installed
      ? `Hooks installed · port ${status.port}${status.listening ? '' : ' · listener not running'}`
      : `Not connected · port ${status.port}`;
    this.hooksLine.textContent = status.message ? `${state} — ${status.message}` : state;
    this.connectButton.textContent = status.installed ? 'Reinstall hooks' : 'Connect Claude Code';
  }
}
