import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { ChatHeaderBar } from './ChatHeaderBar';

describe('ChatHeaderBar', () => {
  it('lays the title and the actions out in one row so they cannot overlap', () => {
    render(
      <ChatHeaderBar title={<span>Title</span>} actions={<span>Actions</span>} reserveNavToggle />
    );

    const bar = screen.getByTestId('chat-header-bar');
    const title = screen.getByTestId('chat-header-title');
    const actions = screen.getByTestId('chat-header-actions');

    expect(bar).toHaveClass('flex', 'inset-x-0');
    expect(title.parentElement).toBe(bar);
    expect(actions.parentElement).toBe(bar);
    expect(title).toHaveClass('min-w-0', 'flex-1');
    expect(actions).toHaveClass('min-w-0', 'max-w-1/2');
    expect(title).toHaveTextContent('Title');
    expect(actions).toHaveTextContent('Actions');
  });

  it('starts after the floating navigation toggle only while the sidebar is collapsed', () => {
    const { rerender } = render(<ChatHeaderBar title="Title" actions="Actions" reserveNavToggle />);
    expect(screen.getByTestId('chat-header-bar')).toHaveClass('pl-36');

    rerender(<ChatHeaderBar title="Title" actions="Actions" reserveNavToggle={false} />);
    expect(screen.getByTestId('chat-header-bar')).toHaveClass('pl-4');
    expect(screen.getByTestId('chat-header-bar')).not.toHaveClass('pl-36');
  });
});
