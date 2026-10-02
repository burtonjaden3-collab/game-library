import type { SVGProps } from "react";

type IconProps = SVGProps<SVGSVGElement>;

const base = (props: IconProps) => ({
  width: 18,
  height: 18,
  viewBox: "0 0 24 24",
  fill: "none",
  stroke: "currentColor",
  strokeWidth: 2,
  strokeLinecap: "round" as const,
  strokeLinejoin: "round" as const,
  "aria-hidden": true,
  ...props,
});

export const PlayIcon = (p: IconProps) => (
  <svg {...base(p)} fill="currentColor" stroke="none">
    <path d="M8 5.5v13a1 1 0 0 0 1.52.85l10.4-6.5a1 1 0 0 0 0-1.7L9.52 4.65A1 1 0 0 0 8 5.5Z" />
  </svg>
);

export const DownloadIcon = (p: IconProps) => (
  <svg {...base(p)}>
    <path d="M12 4v11m0 0-4.5-4.5M12 15l4.5-4.5M5 20h14" />
  </svg>
);

export const SearchIcon = (p: IconProps) => (
  <svg {...base(p)}>
    <circle cx="11" cy="11" r="6.5" />
    <path d="m20 20-4.2-4.2" />
  </svg>
);

export const GridIcon = (p: IconProps) => (
  <svg {...base(p)}>
    <rect x="4" y="4" width="6.5" height="6.5" rx="1.5" />
    <rect x="13.5" y="4" width="6.5" height="6.5" rx="1.5" />
    <rect x="4" y="13.5" width="6.5" height="6.5" rx="1.5" />
    <rect x="13.5" y="13.5" width="6.5" height="6.5" rx="1.5" />
  </svg>
);

export const CheckCircleIcon = (p: IconProps) => (
  <svg {...base(p)}>
    <circle cx="12" cy="12" r="8.5" />
    <path d="m8.5 12.2 2.4 2.4 4.6-4.9" />
  </svg>
);

export const CloudIcon = (p: IconProps) => (
  <svg {...base(p)}>
    <path d="M7 18.5a4.5 4.5 0 0 1-.6-8.96 6 6 0 0 1 11.4 1.46A3.75 3.75 0 0 1 17.25 18.5H7Z" />
  </svg>
);

export const RefreshIcon = (p: IconProps) => (
  <svg {...base(p)}>
    <path d="M20 11a8 8 0 0 0-14.6-4.5M4 4v3.5h3.5M4 13a8 8 0 0 0 14.6 4.5M20 20v-3.5h-3.5" />
  </svg>
);

export const ArrowLeftIcon = (p: IconProps) => (
  <svg {...base(p)}>
    <path d="M19 12H5m0 0 6-6m-6 6 6 6" />
  </svg>
);

export const CloseIcon = (p: IconProps) => (
  <svg {...base(p)}>
    <path d="M6 6l12 12M18 6 6 18" />
  </svg>
);

export const FolderIcon = (p: IconProps) => (
  <svg {...base(p)}>
    <path d="M3.5 7.5a2 2 0 0 1 2-2h4l2 2h7a2 2 0 0 1 2 2v7.5a2 2 0 0 1-2 2h-13a2 2 0 0 1-2-2V7.5Z" />
  </svg>
);

export const DriveIcon = (p: IconProps) => (
  <svg {...base(p)}>
    <rect x="3.5" y="13" width="17" height="6.5" rx="2" />
    <path d="M5.5 13 8 5.5h8l2.5 7.5M16.5 16.3h.01" />
  </svg>
);

export const ClockIcon = (p: IconProps) => (
  <svg {...base(p)}>
    <circle cx="12" cy="12" r="8.5" />
    <path d="M12 7.5V12l3 2" />
  </svg>
);
