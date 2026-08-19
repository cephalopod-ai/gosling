import { StrictMode } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { resolveShellApi } from './api';
import { ShellApp } from './ShellApp';
import { createShellStore, type ShellStore } from './state/store';

export interface MountedDefaultShell {
  store: ShellStore;
  unmount(): void;
}

export interface MountDefaultShellOptions {
  container?: HTMLElement | null;
  productName?: string;
}

/**
 * The composition seam a consumer renderer calls. The host owns the reusable application; the
 * consumer owns the decision to use it and may replace it entirely.
 */
export function mountDefaultShell(options: MountDefaultShellOptions = {}): MountedDefaultShell {
  const container = options.container ?? document.querySelector<HTMLElement>('#root');
  if (!container) throw new Error('the Gosling shell renderer requires a #root container');
  container.textContent = '';

  const store = createShellStore(resolveShellApi());
  const root: Root = createRoot(container);
  root.render(
    <StrictMode>
      <ShellApp store={store} productName={options.productName ?? document.title} />
    </StrictMode>
  );
  void store.start();

  return {
    store,
    unmount() {
      store.dispose();
      root.unmount();
    },
  };
}
