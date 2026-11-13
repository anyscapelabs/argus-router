"use client";

import { useMemo, useState } from "react";
import { useRouter } from "next/navigation";
import { GoCopy, GoTrash, GoKebabHorizontal, GoCircleSlash } from "react-icons/go";

import { ConfirmDialog } from "@/components/confirm-dialog";
import { EmptyState } from "@/components/empty-state";
import { MenuDropdown } from "@/components/menu-dropdown";
import { SelectDropdown } from "@/components/select-dropdown";
import { useApiKeys, useCreateApiKey, useRevokeApiKey } from "@/domains/keys/hooks";
import { useProfile } from "@/domains/profile/hooks";
import { useResendVerification, useSession } from "@/domains/auth/hooks";
import { useSettings } from "@/domains/settings/hooks";
import type { ApiKeyRecord, CreateKeyResult } from "@/domains/keys/api";

function formatDate(value: string): string {
  return new Date(value).toLocaleDateString(undefined, {
    year: "numeric",
    month: "short",
    day: "numeric",
  });
}

const EXPIRATION_OPTIONS = [
  { label: "No expiration", value: "never" },
  { label: "7 days", value: String(7 * 24 * 60 * 60) },
  { label: "30 days", value: String(30 * 24 * 60 * 60) },
  { label: "60 days", value: String(60 * 24 * 60 * 60) },
  { label: "90 days", value: String(90 * 24 * 60 * 60) },
  { label: "1 year", value: String(365 * 24 * 60 * 60) },
] as const;

function SkeletonRows() {
  const widths = ["w-40", "w-32", "w-20"];
  return (
    <>
      {[0, 1, 2].map((row) => (
        <tr key={row} className="border-b border-border-secondary/50">
          <td className="px-2 py-2.5">
            <div className={`h-3 animate-pulse rounded-sm bg-white/10 ${widths[row] ?? "w-40"}`} />
          </td>
          <td className="whitespace-nowrap px-2 py-2.5">
            <div className="h-3 w-28 animate-pulse rounded-sm bg-white/10" />
          </td>
          <td className="whitespace-nowrap px-2 py-2.5">
            <div className="h-3 w-24 animate-pulse rounded-sm bg-white/10" />
          </td>
          <td className="whitespace-nowrap px-2 py-2.5">
            <div className="h-3 w-24 animate-pulse rounded-sm bg-white/10" />
          </td>
          <td className="w-px whitespace-nowrap px-2 py-2.5">
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
  const profile = useProfile();
  const resend = useResendVerification();
  const session = useSession();
  const settings = useSettings({ enabled: Boolean(session.data) });
  const verified = profile.data?.emailVerified ?? false;

  const maxLifetimeDays = settings.data?.maxKeyLifetimeDays ?? 0;
  const expirationOptions = useMemo(() => {
    if (!maxLifetimeDays) return [...EXPIRATION_OPTIONS];
    const cap = maxLifetimeDays * 24 * 60 * 60;
    return EXPIRATION_OPTIONS.filter(
      (option) => option.value !== "never" && Number(option.value) <= cap,
    );
  }, [maxLifetimeDays]);

  const [open, setOpen] = useState(false);
  const [name, setName] = useState("");
  const [expiresIn, setExpiresIn] = useState<string>("never");
  const [fresh, setFresh] = useState<CreateKeyResult | null>(null);
  const [copied, setCopied] = useState(false);
  const [confirm, setConfirm] = useState<{ key: ApiKeyRecord; action: "revoke" | "delete" } | null>(null);

  const effectiveExpiresIn = expirationOptions.some(
    (option) => option.value === expiresIn,
  )
    ? expiresIn
    : (expirationOptions[expirationOptions.length - 1]?.value ?? "never");

  const openCreate = () => {
    setFresh(null);
    setName("");
    setExpiresIn("never");
    setOpen(true);
  };

  const submit = async () => {
    const trimmed = name.trim();
    if (!trimmed) return;
    const exp = effectiveExpiresIn === "never" ? null : Number(effectiveExpiresIn);
    const result = await create.mutateAsync({ name: trimmed, expiresIn: exp });
    setFresh(result);
    setName("");
    setExpiresIn("never");
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
            Create and manage service accounts
          </p>
        </div>
        <button
          type="button"
          onClick={openCreate}
          disabled={!verified}
          title={
            verified
              ? undefined
              : "Verify your email address before creating a key — check your inbox."
          }
          className="shrink-0 cursor-pointer rounded-sm bg-accent px-2 py-1 text-xs font-medium text-black transition-colors hover:bg-[#d4d4d4] disabled:cursor-not-allowed disabled:opacity-50"
        >
          Create Secret Token
        </button>
      </div>

      {profile.isSuccess && !verified && (
        <p className="mt-3 rounded-md border border-yellow-400/30 bg-yellow-400/10 px-3 py-2 text-xs text-yellow-200">
          Your email address is not verified yet, so the desktop agent will refuse
          sign-in.{" "}
          <button
            type="button"
            onClick={() => profile.data?.email && resend.mutate(profile.data.email)}
            disabled={resend.isPending}
            className="cursor-pointer underline underline-offset-2 hover:text-yellow-100 disabled:opacity-50"
          >
            {resend.isPending ? "Sending…" : "Resend verification email"}
          </button>
        </p>
      )}

      <div className="mt-6 overflow-x-auto rounded-md border border-border-secondary">
        <table className="w-full text-left text-xs">
          <thead>
            <tr className="border-b border-border-secondary bg-tertiary text-white/40">
              <th className="px-2 py-2 font-medium rounded-tl-md">Name</th>
              <th className="whitespace-nowrap px-2 py-2 font-medium">Key</th>
              <th className="whitespace-nowrap px-2 py-2 font-medium">Created</th>
              <th className="whitespace-nowrap px-2 py-2 font-medium">Expiration</th>
              <th className="w-px whitespace-nowrap px-2 py-2 text-right font-medium rounded-tr-md">Actions</th>
            </tr>
          </thead>
          <tbody>
            {keys.isPending ? (
              <SkeletonRows />
            ) : records.length === 0 ? (
              <tr>
                <td colSpan={5} className="px-2 py-10">
                  <EmptyState message="No keys yet — create the first one above." />
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
                  <td className="whitespace-nowrap px-2 py-2 font-mono text-white/70">
                    {key.start ?? "…"}…
                  </td>
                  <td className="whitespace-nowrap px-2 py-2 text-white/70">{formatDate(key.createdAt)}</td>
                  <td className="whitespace-nowrap px-2 py-2 text-white/70">
                    {key.expiresAt ? formatDate(key.expiresAt) : "No expiration"}
                  </td>
                  <td className="w-px whitespace-nowrap px-2 py-2 text-right">
                    <div onClick={(e) => e.stopPropagation()}>
                      <MenuDropdown
                        label="Account actions"
                        trigger={<GoKebabHorizontal className="size-3.5" />}
                        items={[
                          {
                            label: "Revoke secret",
                            icon: GoCircleSlash,
                            tone: "warning",
                            onSelect: () => {
                              revoke.reset();
                              setConfirm({ key, action: "revoke" });
                            },
                          },
                          {
                            label: "Delete service account",
                            icon: GoTrash,
                            tone: "danger",
                            onSelect: () => {
                              revoke.reset();
                              setConfirm({ key, action: "delete" });
                            },
                          },
                        ]}
                      />
                    </div>
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
                <p className="mt-2 text-xs text-white/60">
                  Expires:{" "}
                  <span className="text-white/80">
                    {fresh.expiresAt ? formatDate(fresh.expiresAt) : "No expiration"}
                  </span>
                </p>
              </div>
            ) : (
              <div className="mt-4 space-y-3">
                <div>
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
                <div>
                  <label className="block text-xs text-white/60">Expiration</label>
                  <SelectDropdown
                    value={effectiveExpiresIn}
                    options={expirationOptions}
                    onChange={setExpiresIn}
                  />
                </div>
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

      <ConfirmDialog
        open={confirm !== null}
        title={confirm?.action === "delete" ? "Delete service account?" : "Revoke secret token?"}
        description={
          confirm?.action === "delete" ? (
            <>
              This will permanently delete{" "}
              <span className="text-primary">{confirm.key.name ?? "this service account"}</span> and
              revoke its secret token. The key will stop working immediately. This action cannot be
              undone.
            </>
          ) : (
            <>
              This will revoke the secret token for{" "}
              <span className="text-primary">{confirm?.key.name ?? "this service account"}</span>.
              The key will stop working immediately. This action cannot be undone.
            </>
          )
        }
        confirmLabel={
          revoke.isPending
            ? confirm?.action === "delete"
              ? "Deleting…"
              : "Revoking…"
            : confirm?.action === "delete"
              ? "Delete service account"
              : "Revoke secret"
        }
        tone={confirm?.action === "delete" ? "danger" : "warning"}
        isPending={revoke.isPending}
        error={revoke.isError ? revoke.error.message : null}
        onCancel={() => {
          if (!revoke.isPending) {
            setConfirm(null);
            revoke.reset();
          }
        }}
        onConfirm={async () => {
          if (!confirm) return;
          try {
            await revoke.mutateAsync(confirm.key.id);
            setConfirm(null);
            revoke.reset();
          } catch {
          }
        }}
      />
    </div>
  );
}
