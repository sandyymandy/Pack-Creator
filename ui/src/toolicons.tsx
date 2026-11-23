import type { ComponentChildren } from 'preact';

/** Line icons for the buttons of the top bar (24 by 24, drawn with the text colour). */
const PATHS: Record<string, ComponentChildren> = {
  home: <path d="M3 11.5 12 4l9 7.5M5.5 10v10h5v-6h3v6h5V10" />,
  plus: <path d="M12 5v14M5 12h14" />,
  play: <><rect x="3.5" y="4" width="17" height="16" rx="3" /><path d="m10 9 5 3-5 3z" /></>,
  export: <path d="M12 15V3.5M7.5 8 12 3.5 16.5 8M5 14v6h14v-6" />,
  import: <path d="M12 3.5V15M7.5 10.5 12 15l4.5-4.5M5 17v3.5h14V17" />,
  log: <><path d="M6 3.5h8.5L19 8v12.5H6z" /><path d="M14 3.5V8h5M9 12.5h7M9 16h7M9 9h3" /></>,
  cube: <path d="m12 3 8 4.5v9L12 21l-8-4.5v-9zM4 7.5l8 4.5 8-4.5M12 12v9" />,
  search: <><circle cx="10.5" cy="10.5" r="6" /><path d="m15 15 5 5" /></>,
  docs: <><path d="M5 4.5h10.5a3 3 0 0 1 3 3V20H8a3 3 0 0 1-3-3z" /><path d="M5 17a3 3 0 0 1 3-3h10.5M9.5 8.5h5" /></>,
  settings: <><path d="M4 7h9M17 7h3M4 12h3M11 12h9M4 17h11M19 17h1" /><circle cx="15" cy="7" r="2" /><circle cx="9" cy="12" r="2" /><circle cx="17" cy="17" r="2" /></>,
  sideLeft: <><rect x="3" y="4.5" width="18" height="15" rx="2" /><path d="M9.5 4.5v15" /></>,
  sideRight: <><rect x="3" y="4.5" width="18" height="15" rx="2" /><path d="M14.5 4.5v15" /></>,
  find: <><path d="M4 6h9M4 11h5M4 16h7" /><circle cx="16.5" cy="13.5" r="3.5" /><path d="m19 16 2.5 2.5" /></>,
  help: <><circle cx="12" cy="12" r="8.5" /><path d="M9.6 9.6a2.4 2.4 0 1 1 3.4 2.2c-.7.4-1 .9-1 1.8M12 16.8v.1" /></>,
  layers: <path d="m12 3.5 9 4.5-9 4.5L3 8zM3 12.5l9 4.5 9-4.5M3 16.5l9 4.5 9-4.5" />,
  scope: <><circle cx="9" cy="8" r="3" /><path d="M3.5 19c.4-3.4 2.5-5.2 5.5-5.2s5.1 1.8 5.5 5.2" /><circle cx="17" cy="9" r="2.4" /><path d="M16.5 14.2c2.4.1 3.9 1.6 4.3 4" /></>,
  rig: <><circle cx="7" cy="6" r="2.2" /><circle cx="17.5" cy="9" r="2.2" /><circle cx="10" cy="18" r="2.2" /><path d="M9.2 6.5 15.3 8.5M16.4 11l-5 5.2M8.2 8 9.3 15.9" /></>,
  split: <path d="M3.5 12H9m0 0 5-6h6M9 12l5 6h6M17 3.5 20.5 6 17 8.5M17 15.5l3.5 2.5-3.5 2.5" />,
  wrench: <path d="M14.5 6.5a4 4 0 0 0-5 5L4 17l3 3 5.5-5.5a4 4 0 0 0 5-5l-2.5 2.5-2.5-.5-.5-2.5z" />,
  dots: <><circle cx="5.5" cy="12" r="1.3" /><circle cx="12" cy="12" r="1.3" /><circle cx="18.5" cy="12" r="1.3" /></>,
  collapse: <path d="M5 9.5h14M5 14.5h14M9 5l3 3 3-3M9 19l3-3 3 3" />
};

export type ToolIconName = keyof typeof PATHS;

export function ToolIcon({ name }: { name: ToolIconName }) {
  return (
    <svg viewBox="0 0 24 24" width="19" height="19" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
      {PATHS[name]}
    </svg>
  );
}

/** An icon button that tells what it is on hover (and says it to screen readers). */
export function IconButton({ icon, tip, onClick, primary, left, tour }: { icon: ToolIconName; tip: string; onClick: () => void; primary?: boolean; left?: boolean; tour?: string }) {
  return (
    <button class={'icon-btn' + (primary ? ' primary' : '') + (left ? ' tip-left' : '')} data-tip={tip} data-tour={tour} aria-label={tip} onClick={onClick}>
      <ToolIcon name={icon} />
    </button>
  );
}
