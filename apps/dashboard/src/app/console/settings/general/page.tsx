"use client";

import { useState } from "react";
import { toast } from "sonner";

import { useSession } from "@/domains/auth/hooks";
import { useRenameWorkspace, useWorkspace } from "@/domains/workspace/hooks";

export default function GeneralSettingsPage() {
  // The workspace is read from the API; `draft` only exists while the field is
  // being edited, so the value always resolves back to the stored name. The
  // console layout renders its segment even while signed out, so the request
  // waits for a session like the top bar does.
  const session = useSession();
  const workspace = useWorkspace({ enabled: Boolean(session.data) });
  const rename = useRenameWorkspace();
  const [draft, setDraft] = useState<string | null>(null);

  const storedName = workspace.data?.name ?? "";
  const name = draft ?? storedName;

  // Blur-save: an untouched or blank field never reaches the API, and the draft
  // is cleared once the request settles so a rejected name reverts to what the
  // server has instead of keeping the typo on screen.
  const saveName = () => {
    if (!workspace.data) {
      // Nothing stored yet — keep the typing and say why instead of silently
      // wiping it, because a failed workspace read otherwise looks like a
      // field that simply refuses to save.
      if (name.trim()) {
        toast.error(
          workspace.error?.message ?? "The workspace is still loading",
        );
      }
      return;
    }
    const next = name.trim();
    if (next === storedName) {
      setDraft(null);
      return;
    }
    rename.mutate(next, { onSettled: () => setDraft(null) });
  };

  return (
    <div className="px-2 py-1">
      <h1 className="text-base font-medium text-primary">General</h1>
      <p className="mt-1 text-sm text-white/60">
        Manage your workspace name, region, and other general settings.
      </p>
      <section className="mt-6 rounded-md border border-border-secondary bg-[#262626]">
        <div className="flex items-center gap-4 p-4">
          <div className="flex-1">
            <h2 className="text-sm font-medium text-primary">Workspace Name</h2>
            <p className="text-xs text-white/60">
              This name is used for display only
            </p>
          </div>
          <input
            type="text"
            value={name}
            onChange={(event) => setDraft(event.target.value)}
            onBlur={saveName}
            placeholder={workspace.data ? undefined : "Loading…"}
            className="w-64 rounded-md border border-border-secondary bg-[#202020] px-3 py-1.5 text-sm text-primary outline-none focus:border-primary"
          />
        </div>
        <div className="border-t border-border-secondary">
          <div className="flex items-center gap-4 p-4">
            <div className="flex-1">
              <h2 className="text-sm font-medium text-primary">Avatar</h2>
              <p className="text-xs text-white/60">
                Shown next to your workspace name
              </p>
            </div>
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
          </div>
        </div>
      </section>
    </div>
  );
}
