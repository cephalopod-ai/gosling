import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it } from 'vitest';
import { IntlTestWrapper } from '../i18n/test-utils';
import ToolActivityGroup from './ToolActivityGroup';

function renderActivity(hasPendingApproval = false) {
  return render(
    <ToolActivityGroup count={9} hasPendingApproval={hasPendingApproval} status="success">
      <div>Tool details</div>
    </ToolActivityGroup>,
    { wrapper: IntlTestWrapper }
  );
}

describe('ToolActivityGroup', () => {
  it('is collapsed by default and remains expandable', async () => {
    renderActivity();
    const trigger = screen.getByRole('button', { name: 'Activity (9)' });

    expect(trigger).toHaveAttribute('aria-expanded', 'false');
    expect(screen.queryByText('Tool details')).not.toBeInTheDocument();

    await userEvent.click(trigger);

    expect(trigger).toHaveAttribute('aria-expanded', 'true');
    expect(screen.getByText('Tool details')).toBeInTheDocument();
  });

  it('stays open when it contains a pending approval', async () => {
    renderActivity(true);
    const trigger = screen.getByRole('button', { name: 'Activity (9)' });

    expect(trigger).toHaveAttribute('aria-expanded', 'true');
    expect(screen.getByText('Tool details')).toBeInTheDocument();

    await userEvent.click(trigger);

    expect(trigger).toHaveAttribute('aria-expanded', 'true');
  });

  it('marks streaming activity without forcing the details open', () => {
    render(
      <ToolActivityGroup count={3} hasPendingApproval={false} status="loading">
        <div>Tool details</div>
      </ToolActivityGroup>,
      { wrapper: IntlTestWrapper }
    );

    expect(screen.getByRole('button', { name: 'Activity (3)' })).toHaveAttribute(
      'aria-expanded',
      'false'
    );
    expect(screen.queryByText('Tool details')).not.toBeInTheDocument();
    expect(screen.getByLabelText('Tool status: loading')).toBeInTheDocument();
  });
});
