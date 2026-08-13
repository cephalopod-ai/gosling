import type { ShellLifecycleState } from './lifecycle';
import './preloadApi';

function render(state: ShellLifecycleState): void {
  const root = document.getElementById('root');
  if (!root) {
    return;
  }
  root.textContent = state.reasonCode ? `${state.name}: ${state.reasonCode}` : state.name;
}

void window.goslingShell.runtime.read().then(render);
window.goslingShell.runtime.onChanged(render);
