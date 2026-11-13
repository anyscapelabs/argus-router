"use client";

import { useState } from "react";
import Link from "next/link";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { toast } from "sonner";

import { Skeleton } from "@/components/skeleton";
import { authClient } from "@/domains/auth/client";
import { useSession, useSignOut } from "@/domains/auth/hooks";
import { useProfile, type Profile } from "@/domains/profile/hooks";
import { useWorkspace } from "@/domains/workspace/hooks";

export default function AccountSettingsPage() {
  const signOut = useSignOut();
  const session = useSession();
  const profile = useProfile({ enabled: Boolean(session.data) });
  const workspace = useWorkspace({ enabled: Boolean(session.data) });
  const qc = useQueryClient();
  const [draft, setDraft] = useState<string | null>(null);

  const storedName = profile.data?.name ?? "";
  const name = draft ?? storedName;

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
    if (!next) {
      setDraft(null);
      return;
    }
    if (next === storedName) {
      setDraft(null);
      return;
    }
    updateName.mutate(next, { onSettled: () => setDraft(null) });
  };

  return (
    <div className="px-2 py-1">
      <div className="flex items-start justify-between gap-4">
        <div>
          <h1 className="text-base font-medium text-primary">Account Settings</h1>
          <p className="mt-1 text-sm text-white/60">
            Manage your personal account settings.
          </p>
        </div>
        <button
          type="button"
          onClick={() => signOut.mutate()}
          disabled={signOut.isPending}
          className="cursor-pointer rounded-sm bg-red-500/90 px-2 py-1 text-xs font-medium text-white transition-colors hover:bg-red-500 disabled:cursor-not-allowed disabled:opacity-50"
        >
          {signOut.isPending ? "Signing out…" : "Sign out"}
        </button>
      </div>

      <h2 className="mt-6 text-sm font-medium text-primary">General</h2>
      <section className="mt-2 rounded-md border border-border-secondary bg-[#262626]">
        <div className="flex items-center gap-4 p-4">
          <div className="flex-1">
            <h3 className="text-sm font-medium text-primary">Name</h3>
            <p className="text-xs text-white/60">How you appear in your profile</p>
          </div>
          {profile.data ? (
            <input
              type="text"
              value={name}
              onChange={(e) => setDraft(e.target.value)}
              onBlur={saveName}
              className="w-64 rounded-md border border-border-secondary bg-[#202020] px-3 py-1.5 text-sm text-primary outline-none focus:border-primary"
            />
          ) : (
            <Skeleton className="h-9 w-64" />
          )}
        </div>
        <div className="border-t border-border-secondary">
          <div className="flex items-center gap-4 p-4">
            <div className="flex-1">
              <h3 className="text-sm font-medium text-primary">Avatar</h3>
              <p className="text-xs text-white/60">Your profile picture</p>
            </div>
            <div className="flex size-10 items-center justify-center overflow-hidden rounded-full border border-border text-sm text-white/70">
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
        </div>
        <div className="border-t border-border-secondary">
          <div className="flex items-center gap-4 p-4">
            <div className="flex-1">
              <h3 className="text-sm font-medium text-primary">Email</h3>
              <p className="text-xs text-white/60">The email you use to sign in</p>
            </div>
            {profile.data ? (
              <input
                type="text"
                value={profile.data.email ?? ""}
                readOnly
                placeholder={profile.data.email ? undefined : "No email"}
                className="w-64 rounded-md border border-border-secondary bg-[#202020] px-3 py-1.5 text-sm text-white/70 outline-none"
              />
            ) : (
              <Skeleton className="h-9 w-64" />
            )}
          </div>
        </div>
      </section>

      <h2 className="mt-6 text-sm font-medium text-primary">Connected Organization</h2>
      <section className="mt-2 rounded-md border border-border-secondary bg-[#262626] p-4">
        <div className="flex items-center justify-between gap-4">
          <div className="flex items-center gap-3">
            <div className="flex size-10 items-center justify-center overflow-hidden rounded-full border border-border text-sm text-white/70">
              {workspace.data?.avatarUrl ? (
                <img
                  src={workspace.data.avatarUrl}
                  alt=""
                  className="size-full object-cover"
                  referrerPolicy="no-referrer"
                />
              ) : (
                workspace.data?.initials ?? "?"
              )}
            </div>
            <div>
              <div className="text-sm font-medium text-primary">
                {workspace.data?.name ?? <Skeleton className="h-4 w-28" />}
              </div>
              <p className="text-xs text-white/60">
                {workspace.data?.role
                  ? workspace.data.role.charAt(0).toUpperCase() + workspace.data.role.slice(1)
                  : "Owner"}
                <span className="mx-1.5">·</span>
                1 member
              </p>
            </div>
          </div>
          <div className="flex items-center gap-3">
            <p className="text-xs text-white/60">Can&apos;t leave as Owner</p>
            <Link
              href="/console/settings/general"
              className="inline-flex cursor-pointer items-center rounded-sm border border-border-secondary bg-tertiary px-2 py-1 text-xs font-medium text-primary transition-colors hover:bg-white/10"
            >
              Open
            </Link>
          </div>
        </div>
      </section>

      <h2 className="mt-6 text-sm font-medium text-primary">Support</h2>
      <section className="mt-2 rounded-md border border-border-secondary bg-[#262626] p-4">
        <div className="flex items-center justify-between gap-4">
          <div>
            <p className="text-sm font-medium text-primary">Contact support</p>
            <p className="mt-1 text-xs text-white/60">
              Get help with your account, billing, or other questions.
            </p>
          </div>
          <button
            type="button"
            className="inline-flex shrink-0 cursor-pointer items-center rounded-sm border border-border-secondary bg-tertiary px-2 py-1 text-xs font-medium text-primary transition-colors hover:bg-white/10"
          >
            Contact support
          </button>
        </div>
      </section>

      <h2 className="mt-6 text-sm font-medium text-primary">Delete Account</h2>
      <section className="mt-2 rounded-md border border-border-secondary bg-[#262626] p-4">
        <div className="flex items-center justify-between gap-4">
          <div>
            <p className="text-sm font-medium text-primary">
              If you no longer use Argus Agent and Gateway Console
            </p>
            <p className="mt-1 text-xs text-white/60">
              This decision is permanent and can&apos;t be undone.
            </p>
          </div>
          <button
            type="button"
            className="inline-flex shrink-0 cursor-pointer items-center rounded-sm bg-red-500/90 px-2 py-1 text-xs font-medium text-white transition-colors hover:bg-red-500"
          >
            Delete account
          </button>
        </div>
      </section>
    </div>
  );
}
