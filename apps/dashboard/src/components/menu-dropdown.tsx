"use client";

import { useEffect, useRef, useState, type ComponentType, type ReactNode } from "react";

export type MenuItem = {
  label: string;
  icon?: ComponentType<{ size?: number; className?: string }>;
  tone?: "default" | "warning" | "danger";
  onSelect: () => void;
  dividerAfter?: boolean;
};

const TONES: Record<NonNullable<MenuItem["tone"]>, string> = {
  default: "hover:text-primary",
  warning: "hover:text-yellow-400",
  danger: "hover:text-red-400",
};

export function MenuDropdown({
  label = "Menu",
  items,
  trigger,
  header,
  triggerClassName,
}: {
  label?: string;
  items: MenuItem[];
  trigger?: ReactNode;
  header?: ReactNode;
  triggerClassName?: string;
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
        className={
          triggerClassName ??
          "flex size-6 cursor-pointer items-center justify-center rounded-sm bg-[#202020] text-white/60 transition-colors hover:text-primary"
        }
        aria-label={label}
        aria-haspopup="menu"
        aria-expanded={open}
      >
        {trigger ?? <KebabGlyph />}
      </button>
      {open && (
        <div
          role="menu"
          className="absolute top-full right-0 z-20 mt-1 w-52 overflow-hidden rounded-md border border-border-secondary bg-secondary py-1 shadow-lg"
        >
          {header && (
            <>
              <div className="px-3 py-2">{header}</div>
              <div className="my-1 border-t border-border-secondary" />
            </>
          )}
          {items.map((item) => {
            const Icon = item.icon;
            const tone = TONES[item.tone ?? "default"];
            return (
              <div key={item.label}>
                <button
                  type="button"
                  role="menuitem"
                  onClick={() => {
                    setOpen(false);
                    item.onSelect();
                  }}
                  className={`mx-1 flex w-[calc(100%-0.5rem)] cursor-pointer items-center gap-2 rounded-sm px-2 py-1 text-left text-sm text-white/80 transition-colors hover:bg-tertiary ${tone}`}
                >
                  {Icon && <Icon size={12} />}
                  {item.label}
                </button>
                {item.dividerAfter && <div className="my-1 border-t border-border-secondary" />}
              </div>
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
