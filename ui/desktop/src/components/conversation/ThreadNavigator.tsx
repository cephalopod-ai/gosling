import { ArrowDownToLine, ArrowUpToLine } from 'lucide-react';
import { type KeyboardEvent, useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { defineMessages, useIntl } from '../../i18n';
import { getTextAndImageContent, type Message } from '../../types/message';
import { cn } from '../../utils';
import { getMotionAwareScrollBehavior } from '../../utils/motion';
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

function findActiveTurn(
  viewport: HTMLDivElement,
  turns: ThreadTurn[],
  turnIndexByMessageIndex: Map<number, number>
): number {
  const distanceFromBottom = viewport.scrollHeight - viewport.scrollTop - viewport.clientHeight;
  const finalTurn = turns[turns.length - 1];
  if (
    distanceFromBottom <= 2 &&
    finalTurn &&
    viewport.querySelector(`[${THREAD_TURN_ATTRIBUTE}="${finalTurn.messageIndex}"]`)
  ) {
    return turns.length - 1;
  }

  const viewportTop = viewport.getBoundingClientRect().top;
  const readingLine = viewportTop + viewport.clientHeight * 0.5;
  const mountedTurns = Array.from(
    viewport.querySelectorAll<HTMLElement>(`[${THREAD_TURN_ATTRIBUTE}]`)
  ).flatMap((element) => {
    const messageIndex = Number(element.getAttribute(THREAD_TURN_ATTRIBUTE));
    const turnIndex = turnIndexByMessageIndex.get(messageIndex);
    return turnIndex === undefined ? [] : [{ element, turnIndex }];
  });

  let activeTurn = mountedTurns[0]?.turnIndex ?? 0;
  let low = 0;
  let high = mountedTurns.length - 1;

  while (low <= high) {
    const middle = Math.floor((low + high) / 2);
    const candidate = mountedTurns[middle];
    if (candidate.element.getBoundingClientRect().top <= readingLine) {
      activeTurn = candidate.turnIndex;
      low = middle + 1;
    } else {
      high = middle - 1;
    }
  }

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
  const [selectedTurn, setSelectedTurn] = useState<number | null>(null);
  const [visibleTurn, setVisibleTurn] = useState(0);
  const [isScrollable, setIsScrollable] = useState(false);
  const markerTrackRef = useRef<HTMLDivElement>(null);
  const turnButtonRefs = useRef<Array<HTMLButtonElement | null>>([]);
  const turns = useMemo(
    () => getThreadTurns(messages, intl.formatMessage(i18n.image)),
    [intl, messages]
  );
  const turnIndexByMessageIndex = useMemo(
    () => new Map(turns.map((turn, turnIndex) => [turn.messageIndex, turnIndex])),
    [turns]
  );

  const updateActiveTurn = useCallback(() => {
    if (!viewport) {
      setIsScrollable(false);
      setVisibleTurn(0);
      return;
    }
    setIsScrollable(viewport.scrollHeight - viewport.clientHeight > 2);
    if (turns.length === 0) {
      setVisibleTurn(0);
      return;
    }
    const nextVisibleTurn = findActiveTurn(viewport, turns, turnIndexByMessageIndex);
    setVisibleTurn(nextVisibleTurn);
    setSelectedTurn((current) => (current === nextVisibleTurn ? null : current));
  }, [turnIndexByMessageIndex, turns, viewport]);

  useEffect(() => {
    if (!viewport) {
      return;
    }

    let frameId: number | null = null;
    let settleTimerId: number | null = null;
    const scheduleUpdate = () => {
      if (settleTimerId !== null) {
        window.clearTimeout(settleTimerId);
      }
      settleTimerId = window.setTimeout(() => {
        settleTimerId = null;
        setSelectedTurn(null);
        updateActiveTurn();
      }, 150);
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
      if (settleTimerId !== null) {
        window.clearTimeout(settleTimerId);
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
      setSelectedTurn(turnIndex);
      const scrollToElement = () => {
        const element = viewport.querySelector<HTMLElement>(
          `[${THREAD_TURN_ATTRIBUTE}="${turn.messageIndex}"]`
        );
        element?.scrollIntoView({
          behavior: getMotionAwareScrollBehavior(),
          block: 'center',
        });
      };

      if (viewport.querySelector(`[${THREAD_TURN_ATTRIBUTE}="${turn.messageIndex}"]`)) {
        scrollToElement();
      } else {
        onPrepareNavigation?.(turn.messageIndex);
      }
    },
    [onPrepareNavigation, turns, viewport]
  );

  const moveTurnFocus = useCallback(
    (event: KeyboardEvent<HTMLButtonElement>, turnIndex: number) => {
      let nextTurnIndex: number | null = null;
      if (event.key === 'ArrowDown') {
        nextTurnIndex = Math.min(turnIndex + 1, turns.length - 1);
      } else if (event.key === 'ArrowUp') {
        nextTurnIndex = Math.max(turnIndex - 1, 0);
      } else if (event.key === 'Home') {
        nextTurnIndex = 0;
      } else if (event.key === 'End') {
        nextTurnIndex = turns.length - 1;
      }

      if (nextTurnIndex === null) {
        return;
      }
      event.preventDefault();
      turnButtonRefs.current[nextTurnIndex]?.focus();
      jumpToTurn(nextTurnIndex);
    },
    [jumpToTurn, turns.length]
  );

  useEffect(() => {
    const track = markerTrackRef.current;
    const activeButton = turnButtonRefs.current[selectedTurn ?? visibleTurn];
    if (!track || !activeButton) {
      return;
    }

    const trackTop = track.scrollTop;
    const trackBottom = trackTop + track.clientHeight;
    const buttonTop = activeButton.offsetTop - track.offsetTop;
    const buttonBottom = buttonTop + activeButton.offsetHeight;
    if (buttonTop < trackTop) {
      track.scrollTo({ top: buttonTop, behavior: 'auto' });
    } else if (buttonBottom > trackBottom) {
      track.scrollTo({ top: buttonBottom - track.clientHeight, behavior: 'auto' });
    }
  }, [selectedTurn, visibleTurn]);

  if (turns.length < 2 && !historyHasMore && !isScrollable) {
    return null;
  }

  return (
    <nav
      aria-label={intl.formatMessage(i18n.label)}
      className="absolute right-2 top-20 bottom-14 z-20 flex w-7 flex-col items-center md:right-3"
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

      <div
        ref={markerTrackRef}
        className="my-2 flex min-h-0 flex-1 flex-col items-center overflow-y-auto py-1 [scrollbar-width:none] [&::-webkit-scrollbar]:hidden"
      >
        {turns.map((turn, turnIndex) => {
          const activeTurn = selectedTurn ?? visibleTurn;
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
                  ref={(button) => {
                    turnButtonRefs.current[turnIndex] = button;
                  }}
                  type="button"
                  data-thread-turn-control={turnIndex}
                  aria-label={label}
                  aria-current={isActive ? 'location' : undefined}
                  tabIndex={isActive ? 0 : -1}
                  onClick={() => jumpToTurn(turnIndex)}
                  onKeyDown={(event) => moveTurnFocus(event, turnIndex)}
                  className="group flex h-6 w-7 shrink-0 items-center justify-center rounded-sm focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-border-active"
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
