export function Logo({ size = 32 }: { size?: number }) {
  return (
    <div className="flex items-center gap-2 select-none">
      <svg width={size} height={size} viewBox="0 0 40 40" fill="none" aria-hidden="true">
        <defs>
          <linearGradient id="qoinGrad" x1="0" y1="0" x2="40" y2="40" gradientUnits="userSpaceOnUse">
            <stop stopColor="#3D3BD3" />
            <stop offset="1" stopColor="#2FD8E8" />
          </linearGradient>
        </defs>
        <rect width="40" height="40" rx="10" fill="url(#qoinGrad)" />
        <circle cx="20" cy="20" r="10" stroke="white" strokeWidth="3" fill="none" />
        <line x1="26" y1="26" x2="32" y2="32" stroke="white" strokeWidth="3" strokeLinecap="round" />
      </svg>
      <span className="font-heading text-lg font-semibold text-qoin-white tracking-tight">
        Qoin
      </span>
    </div>
  );
}
