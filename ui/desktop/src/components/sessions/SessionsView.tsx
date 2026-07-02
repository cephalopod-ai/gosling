import React, { useCallback } from 'react';
import { useSearchParams } from 'react-router-dom';
import SessionListView from './SessionListView';
import { useNavigation } from '../../hooks/useNavigation';

const SessionsView: React.FC = () => {
  const setView = useNavigation();
  const [searchParams, setSearchParams] = useSearchParams();
  const initialTab = searchParams.get('tab') === 'archived' ? 'archived' : 'active';

  const handleSelectSession = useCallback(
    async (sessionId: string) => {
      setView('pair', {
        disableAnimation: true,
        resumeSessionId: sessionId,
      });
    },
    [setView]
  );

  return (
    <SessionListView
      initialTab={initialTab}
      onSelectSession={handleSelectSession}
      onTabChange={(tab) => {
        const nextSearchParams = new URLSearchParams(searchParams);
        if (tab === 'archived') {
          nextSearchParams.set('tab', 'archived');
        } else {
          nextSearchParams.delete('tab');
        }
        setSearchParams(nextSearchParams, { replace: true });
      }}
    />
  );
};

export default SessionsView;
