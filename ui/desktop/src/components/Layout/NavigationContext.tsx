import React, {
  createContext,
  ReactNode,
  useCallback,
  useContext,
  useEffect,
  useRef,
  useState,
} from 'react';

/**
 * When the window is narrower than this many CSS pixels, we auto-collapse
 * the sidebar. The user can re-expand it via the menu button; it will only
 * auto-collapse again if they go below the threshold from above. An automatic
 * collapse is never saved as the user's preference, and it is undone when the
 * window widens past the threshold again.
 */
const NARROW_WINDOW_THRESHOLD = 700;

interface NavigationContextValue {
  isNavExpanded: boolean;
  setIsNavExpanded: (expanded: boolean) => void;
}

const NavigationContext = createContext<NavigationContextValue | null>(null);

export const useNavigationContext = () => {
  const context = useContext(NavigationContext);
  if (!context) {
    throw new Error('useNavigationContext must be used within NavigationProvider');
  }
  return context;
};

export const useNavigationContextSafe = () => {
  return useContext(NavigationContext);
};

interface NavigationProviderProps {
  children: ReactNode;
}

export const NavigationProvider: React.FC<NavigationProviderProps> = ({ children }) => {
  const [isNavExpanded, setIsNavExpandedState] = useState<boolean>(() => {
    const stored = localStorage.getItem('navigation_expanded');
    return stored !== 'false';
  });

  const isAutoCollapsedRef = useRef(false);

  const setIsNavExpanded = useCallback((expanded: boolean) => {
    isAutoCollapsedRef.current = false;
    setIsNavExpandedState(expanded);
    localStorage.setItem('navigation_expanded', String(expanded));
  }, []);

  const autoCollapse = useCallback(() => {
    isAutoCollapsedRef.current = true;
    setIsNavExpandedState(false);
  }, []);

  const undoAutoCollapse = useCallback(() => {
    isAutoCollapsedRef.current = false;
    setIsNavExpandedState(true);
  }, []);

  const isNavExpandedRef = useRef(isNavExpanded);
  useEffect(() => {
    isNavExpandedRef.current = isNavExpanded;
  }, [isNavExpanded]);

  useEffect(() => {
    const handleToggleNavigation = () => {
      setIsNavExpanded(!isNavExpandedRef.current);
    };
    return window.electron.on('toggle-navigation', handleToggleNavigation);
  }, [setIsNavExpanded]);

  // Auto-collapse the sidebar when the window becomes narrow. Track the
  // previous width so we only fire on the downward crossing — the user can
  // re-expand it manually without us fighting them on the next resize.
  useEffect(() => {
    let lastWidth = window.innerWidth;
    if (lastWidth < NARROW_WINDOW_THRESHOLD && isNavExpandedRef.current) {
      autoCollapse();
    }
    const onResize = () => {
      const width = window.innerWidth;
      const isNarrow = width < NARROW_WINDOW_THRESHOLD;
      const wasNarrow = lastWidth < NARROW_WINDOW_THRESHOLD;
      if (isNarrow && !wasNarrow && isNavExpandedRef.current) {
        autoCollapse();
      } else if (!isNarrow && wasNarrow && isAutoCollapsedRef.current) {
        undoAutoCollapse();
      }
      lastWidth = width;
    };
    window.addEventListener('resize', onResize);
    return () => window.removeEventListener('resize', onResize);
  }, [autoCollapse, undoAutoCollapse]);

  const value: NavigationContextValue = {
    isNavExpanded,
    setIsNavExpanded,
  };

  return <NavigationContext.Provider value={value}>{children}</NavigationContext.Provider>;
};
