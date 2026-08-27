import { NavigateFunction } from 'react-router-dom';
import { UserInput } from '../types/message';
import type { SessionExperience } from '../types/sessionExperience';

export type View =
  | 'chat'
  | 'research'
  | 'pair'
  | 'settings'
  | 'extensions'
  | 'moreModels'
  | 'configureProviders'
  | 'configPage'
  | 'ConfigureProviders'
  | 'settingsV2'
  | 'sessions'
  | 'loading'
  | 'skills'
  | 'permission';

export type ViewOptions = {
  section?: string;
  showEnvVars?: boolean;
  deepLinkConfig?: unknown;
  error?: string;
  parentView?: View;
  parentViewOptions?: ViewOptions;
  disableAnimation?: boolean;
  initialMessage?: UserInput;
  resumeSessionId?: string;
  sessionExperience?: SessionExperience;
};

export const createNavigationHandler = (navigate: NavigateFunction) => {
  return (view: View, options?: ViewOptions) => {
    switch (view) {
      case 'chat':
        navigate('/', { state: options });
        break;
      case 'research':
        navigate('/research', { state: options });
        break;
      case 'pair': {
        // Put resumeSessionId in URL search params (not just state) so that:
        // 1. The sidebar can read it to highlight the active session
        // 2. Page refresh preserves which session is active
        // 3. Browser back/forward navigation works correctly
        const searchParams = new URLSearchParams();
        if (options?.resumeSessionId) {
          searchParams.set('resumeSessionId', options.resumeSessionId);
        }
        if (options?.sessionExperience === 'research') {
          searchParams.set('sessionExperience', options.sessionExperience);
        }
        const url = searchParams.toString() ? `/pair?${searchParams.toString()}` : '/pair';
        navigate(url, { state: options });
        break;
      }
      case 'settings':
        navigate('/settings', { state: options });
        break;
      case 'sessions':
        navigate('/sessions', { state: options });
        break;
      case 'skills':
        navigate('/settings?section=skills', { state: options });
        break;
      case 'permission':
        navigate('/permission', { state: options });
        break;
      case 'ConfigureProviders':
        navigate('/configure-providers', { state: options });
        break;
      case 'extensions':
        navigate('/settings?section=extensions', { state: options });
        break;
      default:
        navigate('/', { state: options });
    }
  };
};
