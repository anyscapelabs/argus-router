"use client";

import Image from "next/image";

import { useSession } from "@/domains/auth/hooks";
import { useProfile } from "@/domains/profile/hooks";
import { useWorkspace } from "@/domains/workspace/hooks";
import { WorkspaceAvatar } from "./workspace-avatar";

export function TopBar() {
  const session = useSession();
  const hasSession = Boolean(session.data);
  const profile = useProfile({ enabled: hasSession });
  const workspace = useWorkspace({ enabled: hasSession });

  const image = hasSession ? profile.data?.image : null;
  const name = hasSession ? profile.data?.name : null;
  const email = hasSession ? profile.data?.email : null;
  const initials = (name ?? email ?? "?")
    .charAt(0)
    .toUpperCase();
  // The workspace name is owned by the API; "Default" is only the pre-read
  // placeholder, which is also what the API provisions for a new account.
  const workspaceName = workspace.data?.name ?? "Default";

  return (
    <header className="flex items-center px-2 py-1.5">
      <Image src="/icon-white.svg" alt="Argus Router" width={20} height={16} priority />
      <div className="ml-2 h-4 w-px bg-border" />
      <button
        type="button"
        className="ml-2 flex items-center gap-2 rounded-sm px-2 py-1 text-sm text-white/70 transition-colors hover:bg-white/5"
      >
        <WorkspaceAvatar
          image={workspace.data?.avatarUrl}
          initials={workspace.data?.initials}
        />
        {workspaceName}
      </button>
      <button
        type="button"
        className="ml-auto rounded-full p-1 transition-colors hover:bg-white/5"
        aria-label={name ?? email ?? "Account"}
      >
        {image ? (
          <img
            src={image}
            alt={name ?? email ?? ""}
            className="size-6 rounded-full border border-border object-cover"
            referrerPolicy="no-referrer"
          />
        ) : (
          <div className="flex size-6 items-center justify-center rounded-full border border-border text-[11px] text-white/70">
            {initials}
          </div>
        )}
      </button>
    </header>
  );
}
