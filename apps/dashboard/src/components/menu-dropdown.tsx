"use client";

import { useEffect, useRef, useState, type ComponentType, type ReactNode } from "react";

export type MenuItem = {
  label: string;
  icon?: ComponentType<{ size?: number; className?: string }>;
  /** Tailwind classes for the row's resting text — defaults to white/80. */
  tone?: "default" | "warning" | "danger";
  onSelect: () => void;
};

const TONES: Record<NonNullable<MenuItem["tone"]>, string> = {
  default: "hover:text-primary",
  warning: "hover:text-yellow-400",
  danger: "hover:text-red-400",
};

/**
 * The ⋯ buttons of the console: a trigger icon and its dropdown, drawn as an
 * overlay above the current section (this overlay is z-layered over content
 * and closes on outside click — the page owns the *actions*, not the menu).
 */
export function MenuDropdown({
  label = "Menu",
  items,
  trigger,
}: {
  label?: string;
  items: MenuItem[];
  /** Defaults to the ⋯ glyph. */
  trigger?: ReactNode;
}) {
  const [open, setOpen] = useState(false);
  const menuRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    const onOutside = (event: MouseEvent) => {
      if (!menuRef.current?.contains(event.target as Node)) setOpen(false);
    };
    document.addEventListener("mousedown", onOutside);
    return () => document.removeEventListener("mousedown", onOutside);
  }, [open]);

  return (
    <div ref={menuRef} className="relative shrink-0">
      <button
        type="button"
        onClick={() => setOpen((v) => !v)}
        className="flex size-6 cursor-pointer items-center justify-center rounded-sm bg-[#202020] text-white/60 transition-colors hover:text-primary"
        aria-label={label}
        aria-haspopup="menu"
        aria-expanded={open}
      >
        {trigger ?? <KebabGlyph />}
      </button>
      {open && (
        <div
          role="menu"
          className="absolute top-full right-0 z-20 mt-1 w-52 overflow-hidden rounded-md border border-border-secondary bg-[#262626] py-1 shadow-lg"
        >
          {items.map((item) => {
            const Icon = item.icon;
            const tone = TONES[item.tone ?? "default"];
            return (
              <button
                key={item.label}
                type="button"
                role="menuitem"
                onClick={() => {
                  setOpen(false);
                  item.onSelect();
                }}
                className={`flex w-full cursor-pointer items-center gap-2 px-3 py-1.5 text-left text-xs text-white/80 transition-colors hover:bg-[#202020] ${tone}`}
              >
                {Icon && <Icon size={12} />}
                {item.label}
              </button>
            );
          })}
        </div>
      )}
    </div>
  );
}

function KebabGlyph(): ReactNode {
  return (
    <svg viewBox="0 0 16 16" className="size-3.5 fill-current">
      <circle cx="8" cy="3" r="1.4" />
      <circle cx="8" cy="8" r="1.4" />
      <circle cx="8" cy="13" r="1.4" />
    </svg>
  );
}
