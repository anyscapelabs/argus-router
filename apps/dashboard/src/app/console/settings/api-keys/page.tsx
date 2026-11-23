"use client";

import { useRouter } from "next/navigation";
import { useState } from "react";
import { GoKebabHorizontal } from "react-icons/go";
import { ApiKeyActionDialog, type ApiKeyAction } from "@/components/api-key-action-dialog";
import { CreateApiKeyDialog } from "@/components/create-api-key-dialog";
import { MenuDropdown } from "@/components/menu-dropdown";
import { Skeleton } from "@/components/skeleton";
import type { ApiKeyRecord } from "@/domains/keys/api";
import {
  useApiKeys,
  useDeleteApiKey,
  useRenameApiKey,
  useRevokeApiKey,
} from "@/domains/keys/hooks";

function displayKey(key: ApiKeyRecord) {
  const prefix = key.prefix || "asp_live";
  const suffix = key.start?.slice(prefix.length);
  return suffix ? `${prefix}.....${suffix}` : "—";
}

export default function ApiKeysSettingsPage() {
  const router = useRouter();
  const keys = useApiKeys();
  const revoke = useRevokeApiKey();
  const rename = useRenameApiKey();
  const remove = useDeleteApiKey();
  const records = keys.data?.apiKeys ?? [];
  const [pending, setPending] = useState<{ action: ApiKeyAction; key: ApiKeyRecord } | null>(null);
  const [nameValue, setNameValue] = useState("");
  const [createOpen, setCreateOpen] = useState(false);

  return (
    <div className="mx-auto w-full max-w-5xl">
      <div className="flex items-start justify-between gap-4">
        <div>
          <h1 className="text-2xl font-medium text-primary">API Keys</h1>
          <p className="mt-1 text-sm text-primary/60">
            Create, rotate, and revoke your API keys.
          </p>
        </div>
        <button
          type="button"
          onClick={() => setCreateOpen(true)}
          className="cursor-pointer rounded-full bg-accent px-4 py-2 text-sm font-medium text-background transition-opacity hover:opacity-80"
        >
          Create API Key
        </button>
      </div>
      <div className="mt-6 grid grid-cols-[1fr_1fr_1fr_1fr_5rem] gap-4 border-b border-border p-1 text-xs text-primary/60">
        <span>Name</span>
        <span>Key</span>
        <span>Status</span>
        <span className="text-right">Last used</span>
        <span />
      </div>
      {records.map((key) => (
        <div
          key={key.id}
          className="grid grid-cols-[1fr_1fr_1fr_1fr_5rem] gap-4 border-b border-border/50 p-1 py-3 text-sm transition-colors duration-200 hover:bg-tertiary"
        >
          <span className="truncate text-primary">{key.name ?? "Untitled"}</span>
          <span className="truncate font-mono text-primary/70">{displayKey(key)}</span>
          <span>
            <span className="inline-flex rounded-full bg-primary/10 px-2 py-0.5 text-xs text-primary/70">
              {key.enabled ? "Active" : "Revoked"}
            </span>
          </span>
          <span className="text-right text-primary/60">—</span>
          <div className="justify-self-end">
            <MenuDropdown
              label="Key actions"
              trigger={<GoKebabHorizontal aria-hidden="true" className="size-4" />}
              triggerClassName="rounded-sm p-1 text-primary/60 transition-colors hover:bg-primary/10 hover:text-primary"
              items={[
                {
                  label: "Rename",
                  onSelect: () => {
                    setNameValue(key.name ?? "");
                    setPending({ action: "rename", key });
                  },
                },
                { label: "View logs", onSelect: () => router.push(`/console/logs?keyId=${encodeURIComponent(key.id)}`) },
                { label: "Revoke", onSelect: () => setPending({ action: "revoke", key }) },
                {
                  label: "Delete key",
                  tone: "danger",
                  dividerBefore: true,
                  onSelect: () => setPending({ action: "delete", key }),
                },
              ]}
            />
          </div>
        </div>
      ))}
      {keys.isPending &&
        Array.from({ length: 5 }, (_, row) => (
          <div
            key={row}
            className="grid grid-cols-[1fr_1fr_1fr_1fr_5rem] gap-4 border-b border-border/50 p-1 py-3"
          >
            {Array.from({ length: 5 }, (_, col) => (
              <Skeleton key={col} className="rounded-full px-2 py-1" />
            ))}
          </div>
        ))}
      {keys.isSuccess && records.length === 0 && (
        <p className="mt-4 text-sm text-primary/60">No API keys yet.</p>
      )}
      {keys.isError && <p className="mt-4 text-sm text-danger">{keys.error.message}</p>}
      <CreateApiKeyDialog open={createOpen} onClose={() => setCreateOpen(false)} />
      <ApiKeyActionDialog
        action={pending?.action ?? null}
        keyName={pending?.key.name ?? pending?.key.start ?? "this key"}
        nameValue={nameValue}
        onNameChange={setNameValue}
        isPending={revoke.isPending || rename.isPending || remove.isPending}
        error={revoke.error?.message ?? rename.error?.message ?? remove.error?.message}
        onConfirm={async () => {
          if (!pending) return;
          try {
            if (pending.action === "rename") {
              const name = nameValue.trim();
              if (!name) return;
              await rename.mutateAsync({ keyId: pending.key.id, name });
            } else if (pending.action === "revoke") {
              await revoke.mutateAsync(pending.key.id);
            } else {
              await remove.mutateAsync(pending.key.id);
            }
            setPending(null);
          } catch {
          }
        }}
        onCancel={() => setPending(null)}
      />
    </div>
  );
}
