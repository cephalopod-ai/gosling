import { Ban } from 'lucide-react';

export function TurnClosureNotice({ text }: { text: string }) {
  return (
    <div
      className="flex items-center gap-2 py-2 text-xs text-amber-700 dark:text-amber-400"
      data-testid="turn-closure-notice"
    >
      <Ban className="size-3.5 shrink-0" aria-hidden />
      <span>{text}</span>
    </div>
  );
}
