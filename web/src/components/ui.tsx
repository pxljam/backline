import React from "react";
import { Link } from "react-router-dom";

/** Briques d'interface. Sobres : l'outil ne doit pas concurrencer la charte
 *  des collectifs, qui est le vrai sujet visuel. */

export const Card: React.FC<{
  title?: React.ReactNode;
  action?: React.ReactNode;
  children: React.ReactNode;
  className?: string;
}> = ({ title, action, children, className = "" }) => (
  <section className={`rounded-xl border border-line bg-panel ${className}`}>
    {(title || action) && (
      <header className="flex items-center justify-between gap-3 border-b border-line px-4 py-3">
        <h2 className="text-sm font-semibold tracking-wide uppercase text-ink-soft">{title}</h2>
        {action}
      </header>
    )}
    <div className="p-4">{children}</div>
  </section>
);

type ButtonProps = React.ButtonHTMLAttributes<HTMLButtonElement> & {
  variant?: "primary" | "ghost" | "danger";
  size?: "sm" | "md";
};

export const Button: React.FC<ButtonProps> = ({
  variant = "ghost",
  size = "md",
  className = "",
  ...props
}) => {
  const base =
    "inline-flex items-center justify-center gap-2 rounded-lg font-medium transition disabled:cursor-not-allowed disabled:opacity-50";
  const sizes = size === "sm" ? "px-2.5 py-1 text-xs" : "px-3.5 py-2 text-sm";
  const variants = {
    primary: "bg-ink text-paper hover:bg-ink-soft",
    ghost: "border border-line bg-panel text-ink hover:border-ink",
    danger: "border border-accent/40 bg-accent-soft text-accent hover:border-accent",
  }[variant];
  return <button className={`${base} ${sizes} ${variants} ${className}`} {...props} />;
};

export const Field: React.FC<{
  label: string;
  hint?: string;
  children: React.ReactNode;
}> = ({ label, hint, children }) => (
  <label className="block">
    <span className="mb-1 block text-xs font-medium uppercase tracking-wide text-ink-soft">
      {label}
    </span>
    {children}
    {hint && <span className="mt-1 block text-xs text-ink-soft">{hint}</span>}
  </label>
);

export const Input: React.FC<React.InputHTMLAttributes<HTMLInputElement>> = ({
  className = "",
  ...props
}) => (
  <input
    className={`w-full rounded-lg border border-line bg-panel px-3 py-2 text-sm outline-none focus:border-ink ${className}`}
    {...props}
  />
);

export const Textarea: React.FC<React.TextareaHTMLAttributes<HTMLTextAreaElement>> = ({
  className = "",
  ...props
}) => (
  <textarea
    className={`w-full rounded-lg border border-line bg-panel px-3 py-2 text-sm outline-none focus:border-ink ${className}`}
    {...props}
  />
);

export const Select: React.FC<React.SelectHTMLAttributes<HTMLSelectElement>> = ({
  className = "",
  ...props
}) => (
  <select
    className={`w-full rounded-lg border border-line bg-panel px-3 py-2 text-sm outline-none focus:border-ink ${className}`}
    {...props}
  />
);

export const Badge: React.FC<{
  tone?: "neutral" | "good" | "warn" | "bad";
  children: React.ReactNode;
}> = ({ tone = "neutral", children }) => {
  const tones = {
    neutral: "bg-line/50 text-ink-soft",
    good: "bg-emerald-100 text-emerald-800",
    warn: "bg-amber-100 text-amber-800",
    bad: "bg-accent-soft text-accent",
  }[tone];
  return (
    <span className={`inline-block rounded-full px-2 py-0.5 text-xs font-medium ${tones}`}>
      {children}
    </span>
  );
};

export const Empty: React.FC<{ children: React.ReactNode }> = ({ children }) => (
  <p className="py-6 text-center text-sm text-ink-soft">{children}</p>
);

export const ErrorNote: React.FC<{ children: React.ReactNode }> = ({ children }) =>
  children ? (
    <p className="rounded-lg border border-accent/30 bg-accent-soft px-3 py-2 text-sm text-accent">
      {children}
    </p>
  ) : null;

export const Loading: React.FC = () => (
  <p className="py-6 text-center text-sm text-ink-soft">Chargement…</p>
);

export const PageTitle: React.FC<{
  title: string;
  subtitle?: React.ReactNode;
  action?: React.ReactNode;
}> = ({ title, subtitle, action }) => (
  <header className="mb-5 flex flex-wrap items-end justify-between gap-3">
    <div>
      <h1 className="text-2xl font-semibold tracking-tight">{title}</h1>
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
    <Link to={to} className="block hover:bg-paper/60">
      {content}
    </Link>
  ) : (
    content
  );
};
