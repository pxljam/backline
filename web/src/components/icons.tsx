import React from "react";

/**
 * The icon set, drawn inline.
 *
 * No icon library: the application shipped none, and eighteen paths cost less
 * than a dependency. Everything goes through `<Icon name=… />`, so swapping to
 * a library later is a change to this file and nowhere else.
 *
 * All 24×24, stroke-based, `currentColor` — an icon inherits the colour of the
 * text it sits beside, including in the light theme.
 */

const PATHS = {
  dashboard: "M4 13h6V4H4v9Zm0 7h6v-5H4v5Zm10 0h6v-9h-6v9Zm0-16v5h6V4h-6Z",
  opportunity: "M12 3v2m0 14v2M5 12H3m18 0h-2M7 7 5.5 5.5M17 7l1.5-1.5M7 17l-1.5 1.5M17 17l1.5 1.5M12 8a4 4 0 1 0 0 8 4 4 0 0 0 0-8Z",
  event: "M4 8h16M7 3v3m10-3v3M5 21h14a1 1 0 0 0 1-1V6a1 1 0 0 0-1-1H5a1 1 0 0 0-1 1v14a1 1 0 0 0 1 1Z",
  calendar: "M4 9h16M8 3v3m8-3v3M5 21h14a1 1 0 0 0 1-1V6a1 1 0 0 0-1-1H5a1 1 0 0 0-1 1v14a1 1 0 0 0 1 1Zm3-8h2v2H8v-2Z",
  group: "M9 11a3 3 0 1 0 0-6 3 3 0 0 0 0 6Zm7 0a3 3 0 1 0 0-6 3 3 0 0 0 0 6ZM3 19v-1a4 4 0 0 1 4-4h4a4 4 0 0 1 4 4v1m2-5h1a4 4 0 0 1 4 4v1",
  member: "M12 12a4 4 0 1 0 0-8 4 4 0 0 0 0 8Zm-8 8v-1a5 5 0 0 1 5-5h6a5 5 0 0 1 5 5v1",
  venue: "M12 21s7-5.5 7-11a7 7 0 1 0-14 0c0 5.5 7 11 7 11Zm0-8a3 3 0 1 0 0-6 3 3 0 0 0 0 6Z",
  studio: "M4 5h16v11H4V5Zm4 15h8m-4-4v4M8 9l2.5 2L8 13m4 0h4",
  render: "m9 8 7 4-7 4V8Z M5 4h14a1 1 0 0 1 1 1v14a1 1 0 0 1-1 1H5a1 1 0 0 1-1-1V5a1 1 0 0 1 1-1Z",
  brand: "M12 3 3 8l9 5 9-5-9-5ZM3 14l9 5 9-5M3 11l9 5 9-5",
  settings: "M12 15a3 3 0 1 0 0-6 3 3 0 0 0 0 6Zm8-3a8 8 0 0 0-.2-1.7l2-1.5-2-3.4-2.3 1a8 8 0 0 0-2.9-1.7L14.2 2H9.8l-.4 2.7a8 8 0 0 0-2.9 1.7l-2.3-1-2 3.4 2 1.5a8 8 0 0 0 0 3.4l-2 1.5 2 3.4 2.3-1a8 8 0 0 0 2.9 1.7l.4 2.7h4.4l.4-2.7a8 8 0 0 0 2.9-1.7l2.3 1 2-3.4-2-1.5c.1-.6.2-1.1.2-1.7Z",
  bell: "M18 16V11a6 6 0 1 0-12 0v5l-2 3h16l-2-3ZM10 22h4",
  instance: "M4 6h16M4 12h16M4 18h16M8 6v12m8-12v12",
  menu: "M4 7h16M4 12h16M4 17h16",
  check: "m5 13 4 4L19 7",
  plus: "M12 5v14M5 12h14",
  alert: "M12 8v5m0 3.5v.01M10.3 3.9 2.6 17.4A2 2 0 0 0 4.3 20.4h15.4a2 2 0 0 0 1.7-3L13.7 3.9a2 2 0 0 0-3.4 0Z",
  arrow: "M5 12h14m-6-6 6 6-6 6",
  copy: "M9 9h10v10H9V9Zm-4 6H4a1 1 0 0 1-1-1V4a1 1 0 0 1 1-1h10a1 1 0 0 1 1 1v1",
  spark: "M12 3v5m0 8v5m9-9h-5M8 12H3m13.5-4.5-3 3m-5 5-3 3m11 0-3-3m-5-5-3-3",
  close: "M6 6l12 12M18 6 6 18",
} as const;

export type IconName = keyof typeof PATHS;

export const Icon: React.FC<{
  name: IconName;
  className?: string;
  /** Icons sit beside their own label; only a lone icon needs a name read out. */
  title?: string;
}> = ({ name, className = "", title }) => (
  <svg
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    strokeWidth="1.6"
    strokeLinecap="round"
    strokeLinejoin="round"
    // The size lives in the base string, and is dropped only when the caller
    // states one. A default parameter would have been silently replaced by any
    // className — which is how one alert icon grew to fill its card.
    className={`shrink-0 ${/\b[hw]-/.test(className) ? "" : "h-4 w-4"} ${className}`}
    aria-hidden={title ? undefined : true}
    role={title ? "img" : undefined}
  >
    {title && <title>{title}</title>}
    <path d={PATHS[name]} />
  </svg>
);
