"use client";

import type { Icon } from "@phosphor-icons/react";
import type { ReactNode } from "react";

export const cn = (...c: (string | false | null | undefined)[]) => c.filter(Boolean).join(" ");

export function Card({ children, className }: { children: ReactNode; className?: string }) {
  return <section className={cn("bg-card rounded-2xl p-5 ring-1 ring-white/5", className)}>{children}</section>;
}

export function SectionTitle({ icon: I, children }: { icon: Icon; children: ReactNode }) {
  return (
    <h2 className="text-muted-foreground mb-4 flex items-center gap-2 text-xs font-medium tracking-[0.14em] uppercase">
      <I size={15} weight="bold" className="text-primary" />
      {children}
    </h2>
  );
}

export function Hint({ children }: { children: ReactNode }) {
  return <p className="text-muted-foreground mt-2 text-sm">{children}</p>;
}

/** One choice out of a few, like a segmented control. Disabled until the headset reported its value. */
export function Segmented<T extends string | number>({
  label,
  options,
  value,
  onChange,
  disabled,
}: {
  label: string;
  options: { value: T; label: string }[];
  value: T | undefined;
  onChange: (v: T) => void;
  disabled?: boolean;
}) {
  return (
    <div role="radiogroup" aria-label={label} className="bg-muted flex gap-1 rounded-xl p-1">
      {options.map((o) => {
        const on = o.value === value;
        return (
          <button
            key={String(o.value)}
            type="button"
            role="radio"
            aria-checked={on}
            disabled={disabled}
            onClick={() => !on && onChange(o.value)}
            className={cn(
              "min-w-0 flex-1 truncate rounded-lg px-2 py-2 text-sm font-medium transition-colors disabled:opacity-40",
              on ? "bg-primary text-primary-foreground shadow-sm" : "text-muted-foreground hover:text-foreground",
            )}
          >
            {o.label}
          </button>
        );
      })}
    </div>
  );
}

export function Switch({
  label,
  checked,
  onChange,
  disabled,
}: {
  label: string;
  checked: boolean;
  onChange: (on: boolean) => void;
  disabled?: boolean;
}) {
  return (
    <button
      type="button"
      role="switch"
      aria-label={label}
      aria-checked={checked}
      disabled={disabled}
      onClick={() => onChange(!checked)}
      className={cn(
        "relative h-6 w-11 shrink-0 rounded-full transition-colors disabled:opacity-40",
        checked ? "bg-primary" : "bg-accent",
      )}
    >
      <span
        className={cn(
          "bg-foreground absolute top-1 left-1 size-4 rounded-full transition-transform",
          checked && "bg-primary-foreground translate-x-5",
        )}
      />
    </button>
  );
}

/** A setting with a switch. `value` undefined = not read yet. */
export function SwitchRow({
  icon: I,
  title,
  subtitle,
  value,
  onChange,
  disabled,
}: {
  icon: Icon;
  title: string;
  subtitle?: string;
  value: boolean | undefined;
  onChange: (on: boolean) => void;
  disabled?: boolean;
}) {
  const off = disabled || value === undefined;
  return (
    <div className={cn("flex items-center gap-3 py-2.5", off && "opacity-60")}>
      <I size={20} className="text-muted-foreground shrink-0" />
      <div className="min-w-0 flex-1">
        <div className="text-[15px]">{title}</div>
        {subtitle && <div className="text-muted-foreground text-sm">{subtitle}</div>}
      </div>
      <Switch label={title} checked={value ?? false} onChange={onChange} disabled={off} />
    </div>
  );
}

/** A setting with a segmented choice under its title. */
export function ChoiceRow<T extends string | number>({
  icon: I,
  title,
  options,
  value,
  onChange,
}: {
  icon: Icon;
  title: string;
  options: { value: T; label: string }[];
  value: T | undefined;
  onChange: (v: T) => void;
}) {
  return (
    <div className="py-2.5">
      <div className="mb-2 flex items-center gap-3">
        <I size={20} className="text-muted-foreground shrink-0" />
        <div className="text-[15px]">{title}</div>
      </div>
      <Segmented label={title} options={options} value={value} onChange={onChange} disabled={value === undefined} />
    </div>
  );
}

export function Divider() {
  return <div className="bg-border my-3 h-px" />;
}
