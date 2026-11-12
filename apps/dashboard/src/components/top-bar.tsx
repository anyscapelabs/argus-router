"use client";

import Image from "next/image";
import { useRouter } from "next/navigation";
import { GoGear, GoHistory, GoMail, GoPerson, GoPulse, GoSignOut } from "react-icons/go";

import { useSession, useSignOut } from "@/domains/auth/hooks";
import { useProfile } from "@/domains/profile/hooks";
import { useWorkspace } from "@/domains/workspace/hooks";
import { MenuDropdown } from "./menu-dropdown";
import { WorkspaceAvatar } from "./workspace-avatar";

export function TopBar() {
  const session = useSession();
  const hasSession = Boolean(session.data);
  const profile = useProfile({ enabled: hasSession });
  const workspace = useWorkspace({ enabled: hasSession });

  const image = hasSession ? profile.data?.image : null;
  const name = hasSession ? profile.data?.name : null;
  const email = hasSession ? profile.data?.email : null;
  const router = useRouter();
  const signOut = useSignOut();
  const initials = (name ?? email ?? "?").charAt(0).toUpperCase();
  const workspaceName = workspace.data?.name ?? "Default";

  const openExternal = (url?: string) => {
    if (url) window.open(url, "_blank", "noopener,noreferrer");
  };

  return (
    <header className="flex items-center px-2 py-1.5">
      <Image src="/icon-white.svg" alt="Anyscape Gateway" width={20} height={16} priority />
      <div className="ml-2 h-4 w-px bg-border" />
      <button
        type="button"
        className="ml-2 flex items-center gap-2 rounded-sm px-2 py-1 text-sm text-white/70 transition-colors hover:bg-white/5"
      >
        <WorkspaceAvatar image={workspace.data?.avatarUrl} initials={workspace.data?.initials} />
        {workspaceName}
      </button>
      <div className="ml-auto">
        <MenuDropdown
          label="Account menu"
          triggerClassName="flex cursor-pointer items-center justify-center rounded-full p-1 transition-colors hover:bg-white/10"
          header={
            <div className="flex items-center gap-2">
              {image ? (
                <img
                  src={image}
                  alt=""
                  className="size-6 shrink-0 rounded-full border border-border object-cover"
                  referrerPolicy="no-referrer"
                />
              ) : (
                <div className="flex size-6 shrink-0 items-center justify-center rounded-full border border-border bg-[#202020] text-[11px] text-white/70">
                  {initials}
                </div>
              )}
              <span className="truncate text-sm text-white/80">{email ?? name ?? "Account"}</span>
            </div>
          }
          items={[
            {
              label: "Account settings",
              icon: GoPerson,
              onSelect: () => router.push("/console/settings/account"),
            },
            {
              label: "Workspace settings",
              icon: GoGear,
              onSelect: () => router.push("/console/settings/general"),
              dividerAfter: true,
            },
            {
              label: "Changelog",
              icon: GoHistory,
              onSelect: () => openExternal(process.env.NEXT_PUBLIC_CHANGELOG_URL),
            },
            {
              label: "Contact support",
              icon: GoMail,
              onSelect: () => openExternal(process.env.NEXT_PUBLIC_SUPPORT_URL),
            },
            {
              label: "System status",
              icon: GoPulse,
              onSelect: () => openExternal(process.env.NEXT_PUBLIC_STATUS_URL),
              dividerAfter: true,
            },
            {
              label: "Sign out",
              icon: GoSignOut,
              tone: "danger",
              onSelect: () => signOut.mutate(),
            },
          ]}
          trigger={
            image ? (
              <img
                src={image}
                alt=""
                className="size-6 rounded-full border border-border object-cover"
                referrerPolicy="no-referrer"
              />
            ) : (
              <div
                className="flex size-6 items-center justify-center rounded-full border border-border bg-[#202020] text-[11px] text-white/70"
                aria-hidden="true"
              >
                {initials}
              </div>
            )
          }
        />
      </div>
    </header>
  );
}
