import { ArrowDownToLine, ArrowUpToLine } from 'lucide-react';
import { useCallback, useEffect, useMemo, useState } from 'react';
import { defineMessages, useIntl } from '../../i18n';
import { getTextAndImageContent, type Message } from '../../types/message';
import { cn } from '../../utils';
import { Tooltip, TooltipContent, TooltipTrigger } from '../ui/Tooltip';

const i18n = defineMessages({
  label: {
    id: 'threadNavigator.label',
    defaultMessage: 'Thread navigation',
  },
  start: {
    id: 'threadNavigator.start',
    defaultMessage: 'Jump to start',
  },
  startWithHistory: {
    id: 'threadNavigator.startWithHistory',
    defaultMessage: 'Jump to start · loads older history',
  },
  latest: {
    id: 'threadNavigator.latest',
    defaultMessage: 'Jump to latest',
  },
  turn: {
    id: 'threadNavigator.turn',
    defaultMessage: 'Turn {current} of {total}: {preview}',
  },
  image: {
    id: 'threadNavigator.image',
    defaultMessage: 'Image message',
  },
});

export const THREAD_TURN_ATTRIBUTE = 'data-thread-turn-index';

interface ThreadTurn {
  messageIndex: number;
  preview: string;
}

interface ThreadNavigatorProps {
  messages: Message[];
  viewport: HTMLDivElement | null;
  historyHasMore?: boolean;
  historyLoading?: boolean;
  onPrepareNavigation?: (messageIndex: number) => void;
  onJumpToStart: () => void | Promise<void>;
  onJumpToLatest: () => void;
}

function getThreadTurns(messages: Message[], imageFallback: string): ThreadTurn[] {
  return messages.flatMap((message, messageIndex) => {
    if (message.role !== 'user' || !message.metadata.userVisible) {
      return [];
    }

    const { imagePaths, textContent } = getTextAndImageContent(message);
    const normalizedText = textContent.replace(/\s+/g, ' ').trim();
    if (!normalizedText && imagePaths.length === 0) {
      return [];
    }

    return [
      {
        messageIndex,
        preview:
          normalizedText.length > 96
            ? `${normalizedText.slice(0, 95).trimEnd()}…`
            : normalizedText || imageFallback,
      },
    ];
  });
}

function findActiveTurn(viewport: HTMLDivElement, turns: ThreadTurn[]): number {
  const distanceFromBottom = viewport.scrollHeight - viewport.scrollTop - viewport.clientHeight;
  if (distanceFromBottom <= 2) {
    return turns.length - 1;
  }

  const viewportTop = viewport.getBoundingClientRect().top;
  const readingLine = viewportTop + viewport.clientHeight * 0.5;
  let activeTurn = 0;

  turns.forEach((turn, turnIndex) => {
    const element = viewport.querySelector<HTMLElement>(
      `[${THREAD_TURN_ATTRIBUTE}="${turn.messageIndex}"]`
    );
    if (element && element.getBoundingClientRect().top <= readingLine) {
      activeTurn = turnIndex;
    }
  });

  return activeTurn;
}

export default function ThreadNavigator({
  messages,
  viewport,
  historyHasMore = false,
  historyLoading = false,
  onPrepareNavigation,
  onJumpToStart,
  onJumpToLatest,
}: ThreadNavigatorProps) {
  const intl = useIntl();
  const [activeTurn, setActiveTurn] = useState(0);
  const turns = useMemo(
    () => getThreadTurns(messages, intl.formatMessage(i18n.image)),
    [intl, messages]
  );

  const updateActiveTurn = useCallback(() => {
    if (!viewport || turns.length === 0) {
      setActiveTurn(0);
      return;
    }
    setActiveTurn(findActiveTurn(viewport, turns));
  }, [turns, viewport]);

  useEffect(() => {
    if (!viewport) {
      return;
    }

    let frameId: number | null = null;
    const scheduleUpdate = () => {
      if (frameId !== null) {
        return;
      }
      frameId = window.requestAnimationFrame(() => {
        frameId = null;
        updateActiveTurn();
      });
    };

    updateActiveTurn();
    viewport.addEventListener('scroll', scheduleUpdate, { passive: true });
    window.addEventListener('resize', scheduleUpdate);

    const observer = new ResizeObserver(scheduleUpdate);
    observer.observe(viewport);
    const content = viewport.firstElementChild;
    if (content instanceof HTMLElement) {
      observer.observe(content);
    }

    return () => {
      viewport.removeEventListener('scroll', scheduleUpdate);
      window.removeEventListener('resize', scheduleUpdate);
      if (frameId !== null) {
        window.cancelAnimationFrame(frameId);
      }
      observer.disconnect();
    };
  }, [updateActiveTurn, viewport]);

  const jumpToTurn = useCallback(
    (turnIndex: number) => {
      if (!viewport) {
        return;
      }
      const turn = turns[turnIndex];
      setActiveTurn(turnIndex);
      const scrollToElement = () => {
        const element = viewport.querySelector<HTMLElement>(
          `[${THREAD_TURN_ATTRIBUTE}="${turn.messageIndex}"]`
        );
        element?.scrollIntoView({ behavior: 'smooth', block: 'center' });
      };

      if (viewport.querySelector(`[${THREAD_TURN_ATTRIBUTE}="${turn.messageIndex}"]`)) {
        scrollToElement();
      } else {
        onPrepareNavigation?.(turn.messageIndex);
      }
    },
    [onPrepareNavigation, turns, viewport]
  );

  if (turns.length < 2) {
    return null;
  }

  return (
    <nav
      aria-label={intl.formatMessage(i18n.label)}
      className="absolute right-3 top-20 bottom-14 z-20 hidden w-7 flex-col items-center md:flex"
    >
      <Tooltip>
        <TooltipTrigger asChild>
          <button
            type="button"
            aria-label={intl.formatMessage(i18n.start)}
            disabled={historyLoading}
            onClick={() => void onJumpToStart()}
            className="flex size-7 shrink-0 items-center justify-center rounded-full border border-border-primary bg-background-primary text-text-secondary shadow-sm transition-colors hover:bg-background-secondary hover:text-text-primary focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-border-active disabled:opacity-50"
          >
            <ArrowUpToLine className="size-3.5" />
          </button>
        </TooltipTrigger>
        <TooltipContent side="left">
          {intl.formatMessage(historyHasMore ? i18n.startWithHistory : i18n.start)}
        </TooltipContent>
      </Tooltip>

      <div className="my-2 flex min-h-0 flex-1 flex-col items-center justify-evenly py-1">
        {turns.map((turn, turnIndex) => {
          const isActive = turnIndex === activeTurn;
          const label = intl.formatMessage(i18n.turn, {
            current: turnIndex + 1,
            total: turns.length,
            preview: turn.preview,
          });

          return (
            <Tooltip key={`${turn.messageIndex}-${turnIndex}`}>
              <TooltipTrigger asChild>
                <button
                  type="button"
                  aria-label={label}
                  aria-current={isActive ? 'location' : undefined}
                  onClick={() => jumpToTurn(turnIndex)}
                  className="group flex h-4 w-7 shrink items-center justify-center rounded-sm focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-border-active"
                >
                  <span
                    className={cn(
                      'h-0.5 w-2 rounded-full bg-border-tertiary transition-all group-hover:w-3 group-hover:bg-text-secondary',
                      isActive && 'h-1 w-3 bg-text-primary'
                    )}
                  />
                </button>
              </TooltipTrigger>
              <TooltipContent side="left" className="max-w-72">
                {label}
              </TooltipContent>
            </Tooltip>
          );
        })}
      </div>

      <Tooltip>
        <TooltipTrigger asChild>
          <button
            type="button"
            aria-label={intl.formatMessage(i18n.latest)}
            onClick={onJumpToLatest}
            className="flex size-7 shrink-0 items-center justify-center rounded-full border border-border-primary bg-background-primary text-text-secondary shadow-sm transition-colors hover:bg-background-secondary hover:text-text-primary focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-border-active"
          >
            <ArrowDownToLine className="size-3.5" />
          </button>
        </TooltipTrigger>
        <TooltipContent side="left">{intl.formatMessage(i18n.latest)}</TooltipContent>
      </Tooltip>
    </nav>
  );
}
