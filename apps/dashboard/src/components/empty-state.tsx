"use client";

import { GoInfo } from "react-icons/go";

export function EmptyState({
  message,
  className = "",
}: {
  message: string;
  className?: string;
}) {
  return (
    <div
      className={`flex flex-col items-center justify-center gap-2 ${className}`}
      role="status"
    >
      <GoInfo className="size-5 text-white/35" aria-hidden />
      <p className="text-xs text-white/40">{message}</p>
    </div>
  );
}