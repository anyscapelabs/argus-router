"use client";

import { useEffect, type ReactNode } from "react";
import { FaSpinner } from "react-icons/fa";

type ConfirmDialogProps = {
  open: boolean;
  title: string;
  description: ReactNode;
  confirmLabel: string;
  cancelLabel?: string;
  tone?: "default" | "danger" | "warning";
  cardClassName?: string;
  titleClassName?: string;
  descriptionClassName?: string;
  isPending?: boolean;
  error?: string | null;
  onConfirm: () => void;
  onCancel: () => void;
};

export function ConfirmDialog({
  open,
  title,
  description,
  confirmLabel,
  cancelLabel = "Cancel",
  tone = "danger",
  cardClassName = "bg-tertiary",
  titleClassName = "",
  descriptionClassName = "",
  isPending = false,
  error,
  onConfirm,
  onCancel,
}: ConfirmDialogProps) {
  useEffect(() => {
    if (!open) return;
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape" && !isPending) onCancel();
    };
    document.addEventListener("keydown", onKeyDown);
    return () => document.removeEventListener("keydown", onKeyDown);
  }, [open, isPending, onCancel]);

  if (!open) return null;

  const confirmClass =
    tone === "danger"
      ? "bg-danger/90 text-primary hover:bg-danger"
      : tone === "warning"
        ? "bg-warning/90 text-background hover:bg-warning"
        : "bg-accent text-background hover:bg-accent/90";

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-background/80 p-4"
      onClick={() => {
        if (!isPending) onCancel();
      }}
      role="presentation"
    >
      <div
        role="dialog"
        aria-modal="true"
        aria-label={title}
        className={`w-full max-w-md rounded-xl border border-border-secondary p-4 shadow-lg ${cardClassName}`}
        onClick={(e) => e.stopPropagation()}
      >
        <h2 className={`text-sm font-medium text-primary ${titleClassName}`}>{title}</h2>
        <div className={`mt-1 text-sm text-primary/60 ${descriptionClassName}`}>{description}</div>
        {error && <p className="mt-3 text-xs text-danger">{error}</p>}
        <div className="mt-4 flex justify-end gap-2">
          <button
            type="button"
            onClick={onCancel}
            disabled={isPending}
            className="cursor-pointer rounded-full px-3 py-1.5 text-sm text-primary/60 transition-colors hover:text-primary disabled:cursor-not-allowed disabled:opacity-50"
          >
            {cancelLabel}
          </button>
          <button
            type="button"
            onClick={onConfirm}
            disabled={isPending}
            className={`inline-flex cursor-pointer items-center justify-center gap-1.5 rounded-full px-3 py-1.5 text-sm font-medium transition-colors disabled:cursor-not-allowed disabled:opacity-50 ${confirmClass}`}
          >
            {isPending && <FaSpinner className="size-3 animate-spin" />}
            {confirmLabel}
          </button>
        </div>
      </div>
    </div>
  );
}
