"use client";

import { useState } from "react";
import { GoDownload } from "react-icons/go";
import { toast } from "sonner";

import { Skeleton } from "@/components/skeleton";
import { useSession } from "@/domains/auth/hooks";
import { useRenameWorkspace, useWorkspace } from "@/domains/workspace/hooks";

export default function GeneralSettingsPage() {
  const session = useSession();
  const workspace = useWorkspace({ enabled: Boolean(session.data) });
  const rename = useRenameWorkspace();
  const [draft, setDraft] = useState<string | null>(null);

  const storedName = workspace.data?.name ?? "";
  const name = draft ?? storedName;

  const saveName = () => {
    if (!workspace.data) {
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
          {workspace.data ? (
            <input
              type="text"
              value={name}
              onChange={(event) => setDraft(event.target.value)}
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

      <h2 className="mt-6 text-sm font-medium text-primary">Legal</h2>
      <section className="mt-2 rounded-md border border-border-secondary bg-[#262626]">
        <div className="flex items-center justify-between gap-4 p-4">
          <div>
            <h3 className="text-sm font-medium text-primary">Customer terms and conditions</h3>
            <p className="mt-1 text-xs text-white/60">
              Terms that apply to your use of Anyscape Gateway
            </p>
          </div>
          <button
            type="button"
            className="inline-flex shrink-0 cursor-pointer items-center gap-1.5 rounded-sm border border-border-secondary bg-tertiary px-2 py-1 text-xs font-medium text-primary transition-colors hover:bg-white/10"
          >
            <GoDownload className="size-3.5" />
            Download
          </button>
        </div>
        <div className="border-t border-border-secondary">
          <div className="flex items-center justify-between gap-4 p-4">
            <div>
              <h3 className="text-sm font-medium text-primary">Data Processing Agreement</h3>
              <p className="mt-1 text-xs text-white/60">
                How we process personal data on your behalf
              </p>
            </div>
            <button
              type="button"
              className="inline-flex shrink-0 cursor-pointer items-center gap-1.5 rounded-sm border border-border-secondary bg-tertiary px-2 py-1 text-xs font-medium text-primary transition-colors hover:bg-white/10"
            >
              <GoDownload className="size-3.5" />
              Download
            </button>
          </div>
        </div>
      </section>
    </div>
  );
}
