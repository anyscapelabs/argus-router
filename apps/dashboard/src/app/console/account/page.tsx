"use client";

import { useState } from "react";
import { GoChevronDown } from "react-icons/go";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { toast } from "sonner";

import { ConfirmDialog } from "@/components/confirm-dialog";
import { MenuDropdown } from "@/components/menu-dropdown";
import { authClient } from "@/domains/auth/client";
import {
  useDeleteAccount,
  useSession,
  useSignOut,
} from "@/domains/auth/hooks";
import { useProfile, type Profile } from "@/domains/profile/hooks";
import { useSettings, useUpdateSettings } from "@/domains/settings/hooks";

const LIFETIME_OPTIONS = [
  { label: "No limit", value: "0" },
  { label: "7 days", value: "7" },
  { label: "30 days", value: "30" },
  { label: "60 days", value: "60" },
  { label: "90 days", value: "90" },
  { label: "180 days", value: "180" },
  { label: "1 year", value: "365" },
] as const;

export default function AccountSettingsPage() {
  const signOut = useSignOut();
  const session = useSession();
  const profile = useProfile({ enabled: Boolean(session.data) });
  const qc = useQueryClient();
  const settings = useSettings({ enabled: Boolean(session.data) });
  const updateSettings = useUpdateSettings();
  const [draft, setDraft] = useState<string | null>(null);

  const storedName = profile.data?.name ?? "";
  const name = draft ?? storedName;
  const maxLifetime = String(settings.data?.maxKeyLifetimeDays ?? 0);
  const allowTraining = settings.data?.allowFreeTraining ?? true;

  const updateName = useMutation({
    mutationFn: async (next: string) => {
      const response = await authClient.updateUser({ name: next });
      if (response.error) {
        throw new Error(response.error.message ?? "Failed to update name");
      }
      return next;
    },
    onSuccess: async (next) => {
      qc.setQueryData<Profile>(["profile"], (previous) =>
        previous ? { ...previous, name: next } : previous,
      );
      await qc.invalidateQueries({ queryKey: ["auth", "session"] });
      toast.success("Name saved");
    },
    onError: (error) => {
      toast.error(error.message);
    },
  });

  const saveName = () => {
    if (!profile.data) {
      if (name.trim()) {
        toast.error(profile.error?.message ?? "Your profile is still loading");
      }
      return;
    }
    const next = name.trim();
    if (!next || next === storedName) {
      setDraft(null);
      return;
    }
    updateName.mutate(next, { onSettled: () => setDraft(null) });
  };

  const deleteAccount = useDeleteAccount();
  const [confirmDelete, setConfirmDelete] = useState(false);
  const [deletionRequested, setDeletionRequested] = useState(false);

  const requestDeletion = async () => {
    try {
      await deleteAccount.mutateAsync();
      setConfirmDelete(false);
      setDeletionRequested(true);
    } catch {
      setConfirmDelete(false);
    }
  };

  return (
    <div className="mx-auto w-full max-w-5xl">
      <div className="flex items-start justify-between gap-4">
        <div>
          <h1 className="text-2xl font-medium text-primary">My Account</h1>
          <p className="mt-0.5 text-sm text-primary/60">
            Manage your personal account settings.
          </p>
        </div>
        <button
          type="button"
          onClick={() => signOut.mutate()}
          disabled={signOut.isPending}
          className="cursor-pointer rounded-full bg-danger/90 px-3 py-1.5 text-xs font-medium text-primary transition-colors hover:bg-danger disabled:cursor-not-allowed disabled:opacity-50"
        >
          {signOut.isPending ? "Signing out…" : "Sign out"}
        </button>
      </div>

      <div className="mt-6 flex flex-col gap-4">
        <section className="rounded-xl bg-secondary p-4 pb-2">
          <h2 className="text-sm font-medium text-primary/80">Profile</h2>
          <div className="mt-3 flex flex-col divide-y divide-border/50">
            <div className="flex items-center justify-between gap-4 py-3 first:pt-0">
              <div>
                <p className="text-sm font-medium text-primary">Name</p>
                <p className="mt-0.5 text-xs text-primary/60">How you appear in your profile</p>
              </div>
              <input
                type="text"
                value={name}
                onChange={(event) => setDraft(event.target.value)}
                onBlur={saveName}
                disabled={!profile.data}
                className="w-64 rounded-full border border-border-secondary bg-background px-3 py-1.5 text-sm text-primary outline-none focus:border-primary/40 disabled:opacity-50"
              />
            </div>
            <div className="flex items-center justify-between gap-4 py-3">
              <div>
                <p className="text-sm font-medium text-primary">Avatar</p>
                <p className="mt-0.5 text-xs text-primary/60">Your profile picture</p>
              </div>
              <div className="flex size-10 items-center justify-center overflow-hidden rounded-full border border-border text-sm text-primary/70">
                {profile.data?.image ? (
                  <img
                    src={profile.data.image}
                    alt=""
                    className="size-full object-cover"
                    referrerPolicy="no-referrer"
                  />
                ) : (
                  (storedName || profile.data?.email || "?").charAt(0).toUpperCase()
                )}
              </div>
            </div>
            <div className="flex items-center justify-between gap-4 py-3 last:pb-0">
              <div>
                <p className="text-sm font-medium text-primary">Email</p>
                <p className="mt-0.5 text-xs text-primary/60">The email you use to sign in</p>
              </div>
              <p className="text-sm text-primary/70">{profile.data?.email ?? "No email"}</p>
            </div>
          </div>
        </section>

        <section className="rounded-xl bg-secondary p-4">
          <h2 className="text-sm font-medium text-primary/80">Security</h2>
          <div className="mt-3 flex items-center justify-between gap-4">
            <div>
              <p className="text-sm font-medium text-primary">Maximum Key Lifetime</p>
              <p className="mt-0.5 text-xs text-primary/60">
                New API keys expire within this time. Existing keys keep their expiry.
              </p>
            </div>
            <MenuDropdown
              label="Maximum key lifetime"
              triggerClassName="flex w-44 shrink-0 cursor-pointer items-center justify-between rounded-full border border-border-secondary bg-background px-3 py-1.5 text-sm text-primary transition-colors hover:bg-primary/10"
              trigger={
                <>
                  <span className="truncate">
                    {LIFETIME_OPTIONS.find((option) => option.value === maxLifetime)?.label ?? "Select"}
                  </span>
                  <GoChevronDown className="size-3.5 shrink-0 text-primary/60" aria-hidden="true" />
                </>
              }
              items={LIFETIME_OPTIONS.map((option) => ({
                label: option.label,
                selected: option.value === maxLifetime,
                disabled: updateSettings.isPending,
                onSelect: () => updateSettings.mutate({ maxKeyLifetimeDays: Number(option.value) }),
              }))}
            />
          </div>
        </section>

        <section className="rounded-xl bg-secondary p-4">
          <h2 className="text-sm font-medium text-primary/80">Privacy</h2>
          <div className="mt-3 flex items-center justify-between gap-4">
            <div>
              <p className="text-sm font-medium text-primary">
                Allow free endpoints that train on request data
              </p>
              <p className="mt-0.5 text-xs text-primary/60">
                Free models may train on your requests. Turn off to disable all free models.
              </p>
            </div>
            <button
              type="button"
              role="switch"
              aria-checked={allowTraining}
              aria-label="Allow free endpoints that train on request data"
              disabled={updateSettings.isPending}
              onClick={() => updateSettings.mutate({ allowFreeTraining: !allowTraining })}
              className={`relative inline-flex h-5 w-9 shrink-0 cursor-pointer rounded-full transition-colors disabled:cursor-wait ${
                allowTraining ? "bg-accent" : "bg-primary/20"
              }`}
            >
              <span
                className={`absolute top-0.5 left-0.5 block size-4 rounded-full bg-background transition-transform ${
                  allowTraining ? "translate-x-4" : ""
                }`}
              />
            </button>
          </div>
        </section>

        <section className="rounded-xl bg-secondary p-4">
          <div className="flex items-center justify-between gap-4">
            <div>
              <p className="text-sm font-medium text-primary">Contact support</p>
              <p className="mt-0.5 text-xs text-primary/60">
                Get help with your account, billing, or other questions.
              </p>
            </div>
            <a
              href="https://anyscapelabs.com/forum"
              target="_blank"
              rel="noopener noreferrer"
              className="shrink-0 rounded-full border border-border-secondary px-3 py-1.5 text-xs font-medium text-primary transition-colors hover:bg-primary/10"
            >
              Contact support
            </a>
          </div>
        </section>

        <section className="rounded-xl bg-secondary p-4">
          {deletionRequested ? (
            <p className="text-xs leading-relaxed text-primary/60">
              We sent a confirmation email to{" "}
              <span className="font-medium text-primary">{profile.data?.email}</span>. Your
              account is deleted once you confirm from that email.
            </p>
          ) : (
            <div className="flex items-center justify-between gap-4">
              <div>
                <p className="text-sm font-medium text-primary">
                  If you no longer use Argus Agent and Gateway Console
                </p>
                <p className="mt-0.5 text-xs text-primary/60">
                  This decision is permanent and can&apos;t be undone.
                </p>
              </div>
              <button
                type="button"
                onClick={() => setConfirmDelete(true)}
                disabled={deleteAccount.isPending}
                className="shrink-0 cursor-pointer rounded-full bg-danger/90 px-3 py-1.5 text-xs font-medium text-primary transition-colors hover:bg-danger disabled:cursor-not-allowed disabled:opacity-50"
              >
                Delete account
              </button>
            </div>
          )}
        </section>
      </div>

      <ConfirmDialog
        open={confirmDelete}
        cardClassName="bg-secondary"
        titleClassName="text-xl"
        descriptionClassName="mt-2 text-sm leading-relaxed"
        title="Delete your account?"
        description={
          <>
            This permanently deletes your account, sign-in methods and all your data.
            We&apos;ll email{" "}
            <span className="font-medium text-primary">{profile.data?.email ?? "you"}</span>{" "}
            a confirmation link first.
          </>
        }
        confirmLabel="Email me a confirmation link"
        isPending={deleteAccount.isPending}
        onConfirm={requestDeletion}
        onCancel={() => setConfirmDelete(false)}
      />
    </div>
  );
}
