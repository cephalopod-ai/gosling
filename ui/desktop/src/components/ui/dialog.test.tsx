import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { useRef, useState } from 'react';
import { describe, expect, it } from 'vitest';
import { IntlTestWrapper } from '../../i18n/test-utils';
import { Dialog, DialogContent, DialogDescription, DialogTitle } from './dialog';
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from './dropdown-menu';

function StateDialog({
  open,
  onOpenChange,
  onCloseAutoFocus,
}: {
  open: boolean;
  onOpenChange(open: boolean): void;
  onCloseAutoFocus?: (event: Event) => void;
}) {
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent onCloseAutoFocus={onCloseAutoFocus}>
        <DialogTitle>Editor</DialogTitle>
        <DialogDescription>Edit things</DialogDescription>
        <button type="button">Inside</button>
      </DialogContent>
    </Dialog>
  );
}

function MenuOpenedDialog() {
  const [open, setOpen] = useState(false);
  return (
    <>
      <DropdownMenu>
        <DropdownMenuTrigger asChild>
          <button type="button">Workspace actions</button>
        </DropdownMenuTrigger>
        <DropdownMenuContent>
          <DropdownMenuItem onSelect={() => setOpen(true)}>Edit</DropdownMenuItem>
        </DropdownMenuContent>
      </DropdownMenu>
      <StateDialog open={open} onOpenChange={setOpen} />
    </>
  );
}

function ButtonOpenedDialog({ hideOpenerWhileOpen = false }: { hideOpenerWhileOpen?: boolean }) {
  const [open, setOpen] = useState(false);
  return (
    <>
      {!(hideOpenerWhileOpen && open) && (
        <button type="button" onClick={() => setOpen(true)}>
          Manage profiles
        </button>
      )}
      <StateDialog open={open} onOpenChange={setOpen} />
    </>
  );
}

const renderWithIntl = (ui: React.ReactElement) => render(ui, { wrapper: IntlTestWrapper });

const closeWithEscape = async (user: ReturnType<typeof userEvent.setup>) => {
  await user.keyboard('{Escape}');
  await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument());
};

describe('DialogContent return focus', () => {
  it('returns focus to the menu trigger when a menu item opened the dialog', async () => {
    const user = userEvent.setup();
    renderWithIntl(<MenuOpenedDialog />);
    const trigger = screen.getByRole('button', { name: 'Workspace actions' });

    trigger.focus();
    await user.keyboard('{Enter}');
    await screen.findByRole('menuitem', { name: 'Edit' });
    await user.keyboard('{Enter}');
    expect(await screen.findByRole('dialog', { name: 'Editor' })).toBeInTheDocument();

    await closeWithEscape(user);
    await waitFor(() => expect(trigger).toHaveFocus());
  });

  it('returns focus to the button that opened a state-driven dialog', async () => {
    const user = userEvent.setup();
    renderWithIntl(<ButtonOpenedDialog />);
    const opener = screen.getByRole('button', { name: 'Manage profiles' });

    opener.focus();
    await user.keyboard('{Enter}');
    expect(await screen.findByRole('dialog', { name: 'Editor' })).toBeInTheDocument();

    await closeWithEscape(user);
    await waitFor(() => expect(opener).toHaveFocus());
  });

  it('leaves focus alone when the opener is gone', async () => {
    const user = userEvent.setup();
    renderWithIntl(<ButtonOpenedDialog hideOpenerWhileOpen />);

    screen.getByRole('button', { name: 'Manage profiles' }).focus();
    await user.keyboard('{Enter}');
    await screen.findByRole('dialog', { name: 'Editor' });

    await closeWithEscape(user);
    const reopened = screen.getByRole('button', { name: 'Manage profiles' });
    await new Promise((resolve) => setTimeout(resolve, 10));
    expect(reopened).not.toHaveFocus();
    expect(document.body).toHaveFocus();
  });

  it("honours a caller's own onCloseAutoFocus", async () => {
    const user = userEvent.setup();
    function Custom() {
      const [open, setOpen] = useState(false);
      const targetRef = useRef<HTMLButtonElement>(null);
      return (
        <>
          <button type="button" onClick={() => setOpen(true)}>
            Open
          </button>
          <button type="button" ref={targetRef}>
            Elsewhere
          </button>
          <StateDialog
            open={open}
            onOpenChange={setOpen}
            onCloseAutoFocus={(event) => {
              event.preventDefault();
              targetRef.current?.focus();
            }}
          />
        </>
      );
    }
    renderWithIntl(<Custom />);

    screen.getByRole('button', { name: 'Open' }).focus();
    await user.keyboard('{Enter}');
    await screen.findByRole('dialog', { name: 'Editor' });

    await closeWithEscape(user);
    await waitFor(() => expect(screen.getByRole('button', { name: 'Elsewhere' })).toHaveFocus());
  });
});
