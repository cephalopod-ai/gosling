import { EventEmitter } from 'node:events';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { render } from '@testing-library/react';
import { IntlTestWrapper } from '../../i18n/test-utils';
import { SearchView } from './SearchView';

const FIND_CHANNELS = ['find-command', 'find-next', 'find-previous', 'use-selection-find'];

type Listener = (...args: unknown[]) => void;

/**
 * window.electron as the renderer sees it through contextBridge: every call hands the preload a
 * new proxy of the renderer's function, so only the disposer returned by `on` can remove it.
 */
function bridgedElectronEvents(ipc: EventEmitter) {
  const acrossBridge =
    (callback: Listener): Listener =>
    (...args) =>
      callback(...args);
  return {
    on: (channel: string, callback: Listener) => {
      const registered = acrossBridge(callback);
      ipc.on(channel, registered);
      return () => {
        ipc.removeListener(channel, registered);
      };
    },
    off: (channel: string, callback: Listener) => {
      ipc.removeListener(channel, acrossBridge(callback));
    },
  };
}

describe('SearchView menu listeners', () => {
  const ipc = new EventEmitter();
  let originalElectron: typeof window.electron;

  beforeEach(() => {
    originalElectron = window.electron;
    window.electron = {
      ...window.electron,
      ...bridgedElectronEvents(ipc),
    } as unknown as typeof window.electron;
  });

  afterEach(() => {
    window.electron = originalElectron;
    ipc.removeAllListeners();
  });

  it('does not grow the IPC listener count as chats mount and unmount it', () => {
    for (let chat = 0; chat < 12; chat += 1) {
      const view = render(
        <SearchView>
          <div>chat {chat}</div>
        </SearchView>,
        { wrapper: IntlTestWrapper }
      );
      for (const channel of FIND_CHANNELS) {
        expect(ipc.listenerCount(channel)).toBe(1);
      }
      view.unmount();
    }

    for (const channel of FIND_CHANNELS) {
      expect(ipc.listenerCount(channel)).toBe(0);
    }
  });
});
