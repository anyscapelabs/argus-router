"use client";

import { useState } from "react";
import { GoCopy } from "react-icons/go";
import { toast } from "sonner";

import { useCreateApiKey } from "@/domains/keys/hooks";
import type { CreateKeyResult } from "@/domains/keys/api";

export function CreateApiKeyDialog({
  open,
  onClose,
}: {
  open: boolean;
  onClose: () => void;
}) {
  const [name, setName] = useState("");
  const [created, setCreated] = useState<CreateKeyResult | null>(null);
  const create = useCreateApiKey();

  if (!open) return null;

  const close = () => {
    if (create.isPending) return;
    setName("");
    setCreated(null);
    onClose();
  };

  return (
    <div
      role="presentation"
      onClick={close}
      className="fixed inset-0 z-50 flex items-center justify-center bg-background/80 p-4"
    >
      <div
        role="dialog"
        aria-modal="true"
        aria-label={created ? "API key created" : "Create API key"}
        onClick={(event) => event.stopPropagation()}
        className="w-full max-w-md rounded-xl border border-border bg-secondary p-4"
      >
        {created ? (
          <>
            <h2 className="text-lg font-medium text-primary">API key created</h2>
            <p className="mt-1 text-xs text-primary/60">
              Copy this key now — it won&apos;t be shown again.
            </p>
            <div className="mt-4 flex items-center gap-2 rounded-full border border-border-secondary bg-background px-3 py-2">
              <code className="flex-1 truncate text-sm text-primary">{created.key}</code>
              <button
                type="button"
                onClick={() => {
                  void navigator.clipboard.writeText(created.key);
                  toast.success("Key copied");
                }}
                className="cursor-pointer rounded-full border border-border-secondary px-2 py-1 text-xs text-primary transition-colors hover:bg-primary/10"
              >
                <GoCopy className="size-4" aria-label="Copy key" />
              </button>
            </div>
            <button
              type="button"
              onClick={close}
              className="mt-4 w-full cursor-pointer rounded-full bg-accent px-4 py-2 text-sm font-medium text-background transition-opacity hover:opacity-80"
            >
              Done
            </button>
          </>
        ) : (
          <>
            <h2 className="text-lg font-medium text-primary">Create API key</h2>
            <p className="mt-1 text-xs text-primary/60">
              Give the key a name so you can recognize it later.
            </p>
            <input
              type="text"
              value={name}
              onChange={(event) => setName(event.target.value)}
              placeholder="My API key"
              className="mt-4 w-full rounded-full border border-border-secondary bg-background px-3 py-2 text-sm text-primary outline-none focus:border-primary/40"
            />
            {create.isError && (
              <p className="mt-2 text-xs text-danger">{create.error.message}</p>
            )}
            <div className="mt-4 flex justify-end gap-2">
              <button
                type="button"
                onClick={close}
                disabled={create.isPending}
                className="cursor-pointer rounded-full px-3 py-1.5 text-xs text-primary/60 transition-colors hover:text-primary disabled:opacity-50"
              >
                Cancel
              </button>
              <button
                type="button"
                disabled={!name.trim() || create.isPending}
                onClick={async () => {
                  try {
                    const result = await create.mutateAsync({ name: name.trim() });
                    setCreated(result);
                  } catch {
                  }
                }}
                className="cursor-pointer rounded-full bg-accent px-3 py-1.5 text-xs font-medium text-background transition-opacity hover:opacity-80 disabled:cursor-not-allowed disabled:opacity-50"
              >
                {create.isPending ? "Creating…" : "Create key"}
              </button>
            </div>
          </>
        )}
      </div>
    </div>
  );
}
