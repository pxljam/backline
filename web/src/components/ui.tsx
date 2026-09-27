import React from "react";
import { Link } from "react-router-dom";
import { Icon, type IconName } from "./icons";

/** Interface building blocks.
 *
 *  Still not competing with the collectives' own brands, which are the real
 *  visual subject — but the way not to compete is not to be colourless. The
 *  chrome is dark, cold and quiet so that the Studio artboard reads as the only
 *  piece of paper on screen, the way an editing suite frames a canvas. Colour
 *  here means state (accent = the thing to act on, danger/good/warn = status),
 *  never decoration. */

/** Joins class names. Five lines instead of a dependency. */
export const cx = (...parts: Array<string | false | null | undefined>) =>
  parts.filter(Boolean).join(" ");

export const Card: React.FC<{
  title?: React.ReactNode;
  action?: React.ReactNode;
  children: React.ReactNode;
  className?: string;
  /** `accent` lifts one card out of a grid — use it for at most one. */
  tone?: "default" | "accent";
}> = ({ title, action, children, className = "", tone = "default" }) => (
  <section
    className={cx(
      "rounded-panel border bg-panel shadow-[var(--shadow-panel)]",
      tone === "accent" ? "border-accent/40" : "border-line",
      className,
    )}
  >
    {(title || action) && (
      <header className="flex items-center justify-between gap-3 border-b border-line px-4 py-3">
        <h2 className="font-display text-xs font-semibold uppercase tracking-[0.12em] text-ink-soft">
          {title}
        </h2>
        {action}
      </header>
    )}
    <div className="p-4">{children}</div>
  </section>
);

type ButtonProps = React.ButtonHTMLAttributes<HTMLButtonElement> & {
  variant?: "primary" | "ghost" | "danger";
  size?: "sm" | "md" | "lg";
  icon?: IconName;
};

export const Button: React.FC<ButtonProps> = ({
  // Ghost stays the default: flipping it would silently restyle some sixty
  // buttons that never asked to become primary.
  variant = "ghost",
  size = "md",
  icon,
  className = "",
  children,
  ...props
}) => {
  const base =
    "inline-flex items-center justify-center gap-2 rounded-control font-medium transition " +
    "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent focus-visible:ring-offset-2 " +
    "focus-visible:ring-offset-stage disabled:cursor-not-allowed disabled:opacity-50";
  // 44px tall from `md` up: an admin works from a phone too (§14).
  // Shared with the form controls below, so a button and a field sitting on the
  // same row are the same height. Anything else reads as a misalignment.
  const sizes = {
    sm: `${CONTROL_H.sm} px-2.5 text-xs`,
    md: `${CONTROL_H.md} px-3.5 text-sm`,
    lg: "min-h-12 w-full px-4 py-2.5 text-sm",
  }[size];
  const variants = {
    primary: "bg-accent text-on-accent hover:shadow-[var(--shadow-glow)]",
    ghost: "border border-line-strong bg-panel text-ink hover:bg-raised",
    danger: "border border-danger/40 bg-danger-quiet text-danger hover:border-danger",
  }[variant];
  return (
    <button className={cx(base, sizes, variants, className)} {...props}>
      {icon && <Icon name={icon} />}
      {children}
    </button>
  );
};

export const Field: React.FC<{
  label: string;
  hint?: string;
  children: React.ReactNode;
}> = ({ label, hint, children }) => (
  <label className="block">
    <span className="mb-1 block font-display text-xs font-medium uppercase tracking-[0.1em] text-ink-soft">
      {label}
    </span>
    {children}
    {hint && <span className="mt-1 block text-xs text-ink-soft">{hint}</span>}
  </label>
);

/**
 * The one height scale in the interface.
 *
 * `md` is 44px — the tap target an admin needs on a phone (§14) — and `sm` is
 * the dense variant for a row inside a list. Buttons and fields both read from
 * here, because the alternative was a `className` override, and two competing
 * Tailwind classes of equal specificity are settled by stylesheet order rather
 * than by the order they are written in.
 */
const CONTROL_H = { sm: "min-h-8 py-1", md: "min-h-11 py-2" } as const;
type ControlSize = keyof typeof CONTROL_H;

/** One string for the three controls — it was copy-pasted three times, and the
 *  focus ring it was missing had to be added three times too. */
const control =
  "rounded-control border border-line-strong bg-panel px-3 text-ink " +
  "placeholder:text-ink-faint outline-none transition focus:border-accent " +
  "focus-visible:ring-2 focus-visible:ring-accent/60";

/**
 * Full width unless the caller asks for another one.
 *
 * Two Tailwind classes of equal specificity are settled by their order in the
 * stylesheet, not by the order they are written in a `className` — so a base
 * `w-full` quietly beat a `w-auto` override and stretched a dense select across
 * its row. The default is dropped instead of being competed with.
 */
const field = (size: ControlSize, className: string) =>
  cx(
    control,
    CONTROL_H[size],
    size === "sm" ? "text-xs" : "text-sm",
    /\bw-/.test(className) ? "" : "w-full",
  );

export const Input: React.FC<
  Omit<React.InputHTMLAttributes<HTMLInputElement>, "size"> & { size?: ControlSize }
> = ({ className = "", size = "md", ...props }) => (
  <input className={cx(field(size, className), className)} {...props} />
);

export const Textarea: React.FC<
  React.TextareaHTMLAttributes<HTMLTextAreaElement> & { size?: ControlSize }
> = ({ className = "", size = "md", ...props }) => (
  // A textarea sizes itself by `rows`; the shared height is only a floor.
  <textarea className={cx(field(size, className), className)} {...props} />
);

export const Select: React.FC<
  Omit<React.SelectHTMLAttributes<HTMLSelectElement>, "size"> & { size?: ControlSize }
> = ({ className = "", size = "md", ...props }) => (
  // The explicit background matters: a native select on a dark page inherits
  // badly on some engines and comes out white-on-white.
  <select className={cx(field(size, className), "bg-panel", className)} {...props} />
);

/**
 * A segmented control.
 *
 * Four screens had reimplemented this by hand, each with its own padding — and
 * next to a real form control the difference showed as a misalignment. One
 * height, taken from the same scale as buttons and fields.
 */
export function Tabs<T extends string>({
  value,
  onChange,
  options,
  className = "",
}: {
  value: T;
  onChange: (value: T) => void;
  options: readonly (readonly [T, string])[];
  className?: string;
}) {
  return (
    <nav className={cx("flex flex-wrap gap-1.5", className)}>
      {options.map(([key, label]) => (
        <button
          key={key}
          type="button"
          aria-current={value === key ? "page" : undefined}
          onClick={() => onChange(key)}
          className={cx(
            "rounded-control px-3.5 text-sm transition",
            CONTROL_H.md,
            "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent",
            value === key
              ? "bg-accent font-medium text-on-accent"
              : "border border-line-strong text-ink hover:bg-raised",
          )}
        >
          {label}
        </button>
      ))}
    </nav>
  );
}

export const Badge: React.FC<{
  tone?: "neutral" | "good" | "warn" | "bad" | "accent";
  children: React.ReactNode;
}> = ({ tone = "neutral", children }) => {
  const tones = {
    neutral: "bg-raised text-ink-soft",
    good: "bg-good-quiet text-good",
    warn: "bg-warn-quiet text-warn",
    bad: "bg-danger-quiet text-danger",
    accent: "bg-accent-quiet text-accent",
  }[tone];
  return (
    <span
      className={cx(
        "inline-block rounded-full px-2 py-0.5 text-xs font-medium tabular",
        tones,
      )}
    >
      {children}
    </span>
  );
};

/**
 * An empty state says what is not there, and — when there is something to do
 * about it — offers the one action that fills it.
 *
 * `icon`, `hint` and `action` are optional on purpose: every existing call site
 * passes children alone and keeps working untouched. Some emptiness is good
 * news ("rien ne bloque") and must stay a plain sentence with no button.
 */
export const Empty: React.FC<{
  children: React.ReactNode;
  icon?: IconName;
  hint?: string;
  action?: React.ReactNode;
}> = ({ children, icon, hint, action }) => (
  <div className="flex flex-col items-center gap-2 py-8 text-center">
    {icon && (
      <span className="mb-1 rounded-full border border-line bg-raised p-2 text-ink-faint">
        <Icon name={icon} className="h-5 w-5" />
      </span>
    )}
    <p className="text-sm text-ink-soft">{children}</p>
    {hint && <p className="max-w-sm text-xs text-ink-faint">{hint}</p>}
    {action && <div className="mt-1">{action}</div>}
  </div>
);

export const ErrorNote: React.FC<{ children: React.ReactNode }> = ({ children }) =>
  children ? (
    <p
      role="alert"
      className="flex items-start gap-2 rounded-control border border-danger/30 bg-danger-quiet px-3 py-2 text-sm text-danger"
    >
      <Icon name="alert" className="mt-0.5 h-4 w-4" />
      <span className="min-w-0">{children}</span>
    </p>
  ) : null;

/** The calm twin of ErrorNote: context, a confirmation, a welcome. */
export const Note: React.FC<{
  children: React.ReactNode;
  tone?: "info" | "good";
  icon?: IconName;
  className?: string;
}> = ({ children, tone = "info", icon, className = "" }) => (
  <div
    className={cx(
      "flex items-start gap-2 rounded-control border px-3 py-2 text-sm",
      tone === "good"
        ? "border-good/30 bg-good-quiet text-good"
        : "border-accent/30 bg-accent-quiet text-ink",
      className,
    )}
  >
    {icon && <Icon name={icon} className="mt-0.5 h-4 w-4" />}
    <div className="min-w-0">{children}</div>
  </div>
);

export const Skeleton: React.FC<{ className?: string }> = ({ className = "h-4 w-full" }) => (
  <span className={cx("block animate-pulse rounded bg-raised", className)} />
);

/** A pulsing shape rather than the word "Chargement…": it holds the layout
 *  instead of letting it jump when the data lands. */
export const Loading: React.FC<{ label?: string }> = ({ label }) => (
  <div className="space-y-2 py-6" aria-busy="true" aria-live="polite">
    <span className="sr-only">{label ?? "Chargement…"}</span>
    <Skeleton className="h-4 w-1/3" />
    <Skeleton className="h-4 w-2/3" />
    <Skeleton className="h-4 w-1/2" />
  </div>
);

export const PageTitle: React.FC<{
  title: string;
  subtitle?: React.ReactNode;
  action?: React.ReactNode;
}> = ({ title, subtitle, action }) => (
  <header className="mb-5 flex flex-wrap items-end justify-between gap-3">
    <div>
      <h1 className="font-display text-2xl font-semibold tracking-tight">{title}</h1>
      {subtitle && <p className="mt-1 text-sm text-ink-soft">{subtitle}</p>}
    </div>
    {action}
  </header>
);

export const Row: React.FC<{ to?: string; children: React.ReactNode }> = ({ to, children }) => {
  const content = (
    <div className="flex items-center justify-between gap-4 border-b border-line py-3 last:border-0">
      {children}
    </div>
  );
  return to ? (
    <Link
      to={to}
      className="-mx-2 block rounded-control px-2 transition hover:bg-raised focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent"
    >
      {content}
    </Link>
  ) : (
    content
  );
};

/**
 * The application's first overlay, on the native `<dialog>`.
 *
 * `showModal()` gives focus trapping, Escape-to-close, an inert background and
 * top-layer stacking for nothing — all the parts that a hand-rolled overlay
 * gets wrong, and no dependency.
 */
export const Modal: React.FC<{
  open: boolean;
  onClose: () => void;
  title?: string;
  children: React.ReactNode;
}> = ({ open, onClose, title, children }) => {
  const ref = React.useRef<HTMLDialogElement>(null);

  React.useEffect(() => {
    const dialog = ref.current;
    if (!dialog) return;
    if (open && !dialog.open) dialog.showModal();
    if (!open && dialog.open) dialog.close();
  }, [open]);

  return (
    <dialog
      ref={ref}
      onClose={onClose}
      onClick={(e) => {
        // Clicking the backdrop lands on the dialog élément itself.
        if (e.target === ref.current) onClose();
      }}
      className="m-auto w-[min(32rem,calc(100vw-2rem))] rounded-panel border border-line bg-panel p-0 text-ink shadow-[var(--shadow-panel)] backdrop:bg-black/60"
    >
      <div className="flex items-center justify-between gap-3 border-b border-line px-4 py-3">
        <h2 className="font-display text-sm font-semibold uppercase tracking-[0.12em] text-ink-soft">
          {title}
        </h2>
        <Button size="sm" onClick={onClose} aria-label="Fermer">
          <Icon name="close" />
        </Button>
      </div>
      <div className="p-4">{children}</div>
    </dialog>
  );
};
