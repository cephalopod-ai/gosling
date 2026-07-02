import { createContext, useContext, useEffect, useState, useMemo } from 'react';
import { getAcpFeatureCapabilities } from '../acp/capabilities';

interface FeaturesContextValue {
  isLoading: boolean;
}

const FeaturesContext = createContext<FeaturesContextValue | null>(null);

export function FeaturesProvider({ children }: { children: React.ReactNode }) {
  const [isLoading, setIsLoading] = useState(true);

  useEffect(() => {
    (async () => {
      try {
        await getAcpFeatureCapabilities();
      } catch (error) {
        console.warn('[FeaturesContext] Failed to fetch features:', error);
      } finally {
        setIsLoading(false);
      }
    })();
  }, []);

  const value = useMemo<FeaturesContextValue>(() => ({ isLoading }), [isLoading]);

  return <FeaturesContext.Provider value={value}>{children}</FeaturesContext.Provider>;
}

export function useFeatures(): FeaturesContextValue {
  const context = useContext(FeaturesContext);
  if (!context) {
    throw new Error('useFeatures must be used within a FeaturesProvider');
  }
  return context;
}
