import { StrictMode, useLayoutEffect, useMemo, useRef, useState } from 'react';
import { createRoot } from 'react-dom/client';
import { IntlProvider } from 'react-intl';
import ThreadNavigator, {
  THREAD_TURN_ATTRIBUTE,
} from './components/conversation/ThreadNavigator';
import type { Message } from './types/message';
import './styles/main.css';

declare global {
  interface Window {
    navigationPlaytest: {
      lastBehavior: ScrollBehavior | null;
      latestClicks: number;
      startClicks: number;
    };
  }
}

window.navigationPlaytest = {
  lastBehavior: null,
  latestClicks: 0,
  startClicks: 0,
};

HTMLElement.prototype.scrollIntoView = function scrollIntoView(options) {
  const behavior = typeof options === 'object' ? options.behavior : undefined;
  window.navigationPlaytest.lastBehavior = behavior ?? null;
  const viewport = this.closest<HTMLElement>('[data-playtest-viewport]');
  if (viewport) {
    viewport.scrollTo({
      top: Math.max(0, this.offsetTop - viewport.clientHeight / 2),
      behavior,
    });
  }
};

function message(role: Message['role'], text: string, id: string): Message {
  return {
    id,
    role,
    content: [{ type: 'text', text }],
    created: 1,
    metadata: { agentVisible: true, userVisible: true },
  };
}

function NavigationPlaytest() {
  const params = new URLSearchParams(window.location.search);
  const turnCount = Math.max(1, Number.parseInt(params.get('turns') ?? '5', 10));
  const historyHasMore = params.get('history') === '1';
  const longAnswer = params.get('long') === '1';
  const viewportRef = useRef<HTMLDivElement>(null);
  const [viewport, setViewport] = useState<HTMLDivElement | null>(null);
  const messages = useMemo(
    () =>
      Array.from({ length: turnCount }, (_, turnIndex) => [
        message(
          'user',
          `Prompt ${turnIndex + 1} — ${turnIndex % 3 === 0 ? '日本語 🪿 ' : ''}navigation marker ${turnIndex + 1}`,
          `user-${turnIndex}`
        ),
        message(
          'assistant',
          `Answer ${turnIndex + 1}. ${'This is realistic conversation content. '.repeat(longAnswer ? 120 : 12)}`,
          `assistant-${turnIndex}`
        ),
      ]).flat(),
    [longAnswer, turnCount]
  );

  useLayoutEffect(() => {
    setViewport(viewportRef.current);
  }, []);

  return (
    <main className="h-screen bg-background-primary text-text-primary">
      <section className="relative mx-auto h-full max-w-5xl border-x border-border-primary">
        <header className="absolute inset-x-0 top-0 z-10 flex h-16 items-center border-b border-border-primary bg-background-primary px-6">
          <h1 className="text-lg font-semibold">Thread navigation playtest</h1>
        </header>
        <div
          ref={viewportRef}
          data-playtest-viewport
          className="absolute inset-x-0 top-16 bottom-0 overflow-y-auto px-8 pr-12"
        >
          <div className="mx-auto max-w-3xl py-8">
            {messages.map((currentMessage, messageIndex) => (
              <article
                key={currentMessage.id}
                {...(currentMessage.role === 'user'
                  ? { [THREAD_TURN_ATTRIBUTE]: messageIndex }
                  : {})}
                className={
                  currentMessage.role === 'user'
                    ? 'mb-5 ml-auto min-h-20 max-w-2xl rounded-2xl bg-background-secondary px-5 py-4'
                    : 'mb-16 min-h-64 px-2 py-4 leading-7'
                }
              >
                {currentMessage.content[0]?.type === 'text'
                  ? currentMessage.content[0].text
                  : null}
              </article>
            ))}
          </div>
        </div>
        <ThreadNavigator
          messages={messages}
          viewport={viewport}
          historyHasMore={historyHasMore}
          onJumpToStart={() => {
            window.navigationPlaytest.startClicks += 1;
            viewport?.scrollTo({ top: 0, behavior: 'smooth' });
          }}
          onJumpToLatest={() => {
            window.navigationPlaytest.latestClicks += 1;
            viewport?.scrollTo({ top: viewport.scrollHeight, behavior: 'smooth' });
          }}
        />
      </section>
    </main>
  );
}

document.documentElement.classList.add('dark');
document.documentElement.style.cssText = [
  '--color-background-primary:#17191d',
  '--color-background-secondary:#24272d',
  '--color-background-inverse:#f4f6f7',
  '--color-text-primary:#f4f6f7',
  '--color-text-secondary:#a7b0b9',
  '--color-text-inverse:#22252a',
  '--color-border-primary:#474e57',
  '--color-border-tertiary:#606c7a',
  '--color-border-active:#7cacff',
].join(';');

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <IntlProvider locale="en" defaultLocale="en" messages={{}}>
      <NavigationPlaytest />
    </IntlProvider>
  </StrictMode>
);
