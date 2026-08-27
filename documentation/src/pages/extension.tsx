import React, { useEffect } from 'react';
import { useLocation } from '@docusaurus/router';

export default function ExtensionRedirect(): React.ReactElement {
  const location = useLocation();

  useEffect(() => {
    const params = new URLSearchParams(location.search);
    window.location.href = `gosling://extension${params.toString() ? '?' + params.toString() : ''}`;
  }, [location]);

  return (
    <div style={{ padding: '2rem', textAlign: 'center' }}>
      Redirecting to Gosling...
    </div>
  );
}
