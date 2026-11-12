"use client";

import { useEffect, useRef, useState } from "react";
import { GoChevronDown } from "react-icons/go";

export type SelectOption<T extends string = string> = {
  label: string;
  value: T;
};

export function SelectDropdown<T extends string>({
  value,
  options,
  onChange,
  className,
}: {
  value: T;
  options: readonly SelectOption<T>[];
  onChange: (value: T) => void;
  className?: string;
}) {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    const onOutside = (e: MouseEvent) => {
      if (!ref.current?.contains(e.target as Node)) setOpen(false);
    };
    document.addEventListener("mousedown", onOutside);
    return () => document.removeEventListener("mousedown", onOutside);
  }, [open]);

  const selected = options.find((o) => o.value === value);

  return (
    <div ref={ref} className={className ?? "relative mt-1"}>
      <button
        type="button"
        onClick={() => setOpen((v) => !v)}
        className="flex w-full cursor-pointer items-center justify-between rounded-md border border-border-secondary bg-[#202020] px-3 py-1.5 text-left text-sm text-primary outline-none transition-colors focus:border-primary"
        aria-haspopup="listbox"
        aria-expanded={open}
      >
        <span className="truncate">{selected?.label ?? "Select"}</span>
        <GoChevronDown
          className={`ml-2 size-3.5 shrink-0 text-white/60 transition-transform ${open ? "rotate-180" : ""}`}
        />
      </button>
      {open && (
        <div
          role="listbox"
          className="absolute z-30 mt-1 max-h-56 w-full overflow-auto rounded-md border border-border-secondary bg-secondary py-1 shadow-lg"
        >
          {options.map((o) => {
            const isSelected = o.value === value;
            return (
              <button
                key={o.value}
                type="button"
                role="option"
                aria-selected={isSelected}
                onClick={() => {
                  onChange(o.value);
                  setOpen(false);
                }}
                className={`mx-1 flex w-[calc(100%-0.5rem)] cursor-pointer items-center rounded-sm px-2 py-1 text-left text-sm transition-colors hover:bg-tertiary ${
                  isSelected ? "text-primary" : "text-white/80"
                }`}
              >
                {o.label}
              </button>
            );
          })}
        </div>
      )}
    </div>
  );
}
