import '../styles/base.css';
import { App } from './App';
import { createBackend } from './ipc';

const root = document.getElementById('app');
if (!root) throw new Error('#app root element is missing');

const backend = createBackend();
document.documentElement.dataset.backend = backend.kind;

new App(root, backend).start().catch((err: unknown) => {
  console.error('Claude Notch failed to start', err);
});
