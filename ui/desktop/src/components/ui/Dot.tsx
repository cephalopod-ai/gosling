export type LoadingStatus = 'loading' | 'success' | 'error' | 'unknown';
export default function Dot({
  size,
  loadingStatus,
}: {
  size: number;
  loadingStatus: LoadingStatus;
}) {
  const backgroundColorClasses = {
    loading: 'bg-blue-500',
    success: 'bg-green-600',
    error: 'bg-red-600',
    // No confirmed backend response was ever received for this call — do
    // not render it as a green success dot, which would misreport an
    // unconfirmed result as a completed one.
    unknown: 'bg-amber-500',
  };

  return (
    <div className={`${loadingStatus === 'loading' ? '' : ''} flex items-center justify-center`}>
      <div
        className={`rounded-full ${backgroundColorClasses[loadingStatus] || 'bg-icon-extra-subtle'}`}
        style={{
          width: `${size * 2}px`,
          height: `${size * 2}px`,
        }}
      />
    </div>
  );
}
