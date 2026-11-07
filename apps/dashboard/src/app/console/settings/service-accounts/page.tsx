"use client";

import { useState } from "react";
import { useRouter } from "next/navigation";
import { GoCopy } from "react-icons/go";

import { useApiKeys, useCreateApiKey, useRevokeApiKey } from "@/domains/keys/hooks";
import type { CreateKeyResult } from "@/domains/keys/api";

function formatDate(value: string): string {
  return new Date(value).toLocaleDateString(undefined, {
    year: "numeric",
    month: "short",
    day: "numeric",
  });
}

/** Placeholder rows while the key list is in flight — shimmering bars, no copy. */
function SkeletonRows() {
  const widths = ["w-40", "w-32", "w-20"];
  return (
    <>
      {[0, 1, 2].map((row) => (
        <tr key={row} className="border-b border-border-secondary/50">
          <td className="px-2 py-2.5">
            <div className={`h-3 animate-pulse rounded-sm bg-white/10 ${widths[row] ?? "w-40"}`} />
          </td>
          <td className="px-2 py-2.5">
            <div className="h-3 w-28 animate-pulse rounded-sm bg-white/10" />
          </td>
          <td className="px-2 py-2.5">
            <div className="h-3 w-24 animate-pulse rounded-sm bg-white/10" />
          </td>
          <td className="px-2 py-2.5">
            <div className="ml-auto h-3 w-14 animate-pulse rounded-sm bg-white/10" />
          </td>
        </tr>
      ))}
    </>
  );
}

export default function ServiceAccountsSettingsPage() {
  const keys = useApiKeys();
  const create = useCreateApiKey();
  const revoke = useRevokeApiKey();
  const router = useRouter();

  const [open, setOpen] = useState(false);
  const [name, setName] = useState("");
  const [fresh, setFresh] = useState<CreateKeyResult | null>(null);
  const [copied, setCopied] = useState(false);

  const openCreate = () => {
    setFresh(null);
    setName("");
    setOpen(true);
  };

  const submit = async () => {
    const trimmed = name.trim();
    if (!trimmed) return;
    const result = await create.mutateAsync(trimmed);
    setFresh(result);
    setName("");
    setCopied(false);
  };

  const copy = async (value: string) => {
    await navigator.clipboard.writeText(value);
    setCopied(true);
  };

  const records = keys.data?.apiKeys ?? [];

  return (
    <div className="px-2 py-1">
      <div className="flex items-start justify-between gap-4">
        <div>
          <h1 className="text-base font-medium text-primary">Service Accounts</h1>
          <p className="mt-1 text-sm text-white/60">
            Keys the Argus desktop app pairs with — paste one into the app&apos;s sign-in screen.
          </p>
        </div>
        <button
          type="button"
          onClick={openCreate}
          className="shrink-0 cursor-pointer rounded-sm bg-accent px-2 py-1 text-xs font-medium text-black transition-colors hover:bg-[#d4d4d4]"
        >
          Create Secret Token
        </button>
      </div>

      <div className="mt-6 overflow-x-auto rounded-md border border-border-secondary">
        <table className="w-full text-left text-xs">
          <thead>
            <tr className="border-b border-border-secondary bg-tertiary text-white/40">
              <th className="px-2 py-2 font-medium rounded-tl-md">Name</th>
              <th className="px-2 py-2 font-medium">Key</th>
              <th className="px-2 py-2 font-medium">Created</th>
              <th className="px-2 py-2 text-right font-medium rounded-tr-md">Actions</th>
            </tr>
          </thead>
          <tbody>
            {keys.isPending ? (
              <SkeletonRows />
            ) : records.length === 0 ? (
              <tr className="border-b border-border-secondary/50 last:border-b-0">
                <td colSpan={4} className="px-2 py-8 text-center text-white/40">
                  No keys yet — create the first one above.
                </td>
              </tr>
            ) : (
              records.map((key) => (
                <tr
                  key={key.id}
                  onClick={() => router.push(`/console/settings/service-accounts/${key.id}`)}
                  className="cursor-pointer border-b border-border-secondary/50 transition-colors hover:bg-[#202020]"
                >
                  <td className="px-2 py-2 text-primary">{key.name ?? "Untitled"}</td>
                  <td className="px-2 py-2 font-mono text-white/70">
                    {key.prefix ?? ""}
                    {key.start ?? "…"}…
                  </td>
                  <td className="px-2 py-2 text-white/70">{formatDate(key.createdAt)}</td>
                  <td className="px-2 py-2 text-right">
                    <button
                      type="button"
                      onClick={(event) => {
                        event.stopPropagation();
                        void revoke.mutateAsync(key.id);
                      }}
                      disabled={revoke.isPending && revoke.variables === key.id}
                      title="Revoke key"
                      className="cursor-pointer rounded-sm px-2 py-1 text-xs text-white/60 transition-colors hover:text-red-400 disabled:cursor-not-allowed disabled:opacity-50"
                    >
                      Revoke
                    </button>
                  </td>
                </tr>
              ))
            )}
          </tbody>
        </table>
      </div>
      {(keys.isError || revoke.isError) && (
        <p className="mt-3 text-xs text-red-400">
          {keys.isError ? keys.error.message : revoke.error?.message}
        </p>
      )}

      {open && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60">
          <div className="w-full max-w-md rounded-md border border-border-secondary bg-[#262626] p-4">
            <h2 className="text-sm font-medium text-primary">Create Secret Token</h2>
            <p className="mt-1 text-xs text-white/60">
              Create a new secret token for a service account.
            </p>

            {fresh ? (
              <div className="mt-4">
                <p className="text-xs font-medium text-yellow-200">
                  Copy this key now — it is shown only once.
                </p>
                <div className="mt-2 flex items-center gap-2">
                  <code className="block flex-1 overflow-x-auto rounded-sm bg-[#111] px-2 py-1.5 text-xs text-green-300">
                    {fresh.key}
                  </code>
                  <button
                    type="button"
                    onClick={() => void copy(fresh.key)}
                    className="shrink-0 cursor-pointer rounded-md border border-border-secondary px-2 py-1.5 text-xs text-primary hover:bg-[#202020]"
                  >
                    <span className="flex items-center gap-1">
                      <GoCopy size={12} />
                      {copied ? "Copied" : "Copy"}
                    </span>
                  </button>
                </div>
              </div>
            ) : (
              <div className="mt-4">
                <label className="block text-xs text-white/60">Name</label>
                <input
                  type="text"
                  value={name}
                  autoFocus
                  onChange={(e) => setName(e.target.value)}
                  onKeyDown={(e) => {
                    if (e.key === "Enter") void submit();
                  }}
                  placeholder="Service account name"
                  className="mt-1 w-full rounded-md border border-border-secondary bg-[#202020] px-3 py-1.5 text-sm text-primary outline-none focus:border-primary"
                />
              </div>
            )}
            {create.isError && !fresh && (
              <p className="mt-2 text-xs text-red-400">{create.error.message}</p>
            )}

            <div className="mt-4 flex justify-end gap-2">
              <button
                type="button"
                onClick={() => setOpen(false)}
                className="cursor-pointer rounded-sm px-2 py-1 text-xs text-white/60 hover:text-primary"
              >
                {fresh ? "Done" : "Cancel"}
              </button>
              {!fresh && (
                <button
                  type="button"
                  onClick={() => void submit()}
                  disabled={create.isPending || !name.trim()}
                  className="cursor-pointer rounded-sm bg-accent px-2 py-1 text-xs font-medium text-black transition-colors hover:bg-[#d4d4d4] disabled:cursor-not-allowed disabled:opacity-50"
                >
                  {create.isPending ? "Creating…" : "Create"}
                </button>
              )}
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
