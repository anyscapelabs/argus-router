"use client";

import Image from "next/image";
import Link from "next/link";
import { usePathname, useRouter } from "next/navigation";
import { useState } from "react";
import { BsLayoutSidebarReverse, BsReverseLayoutSidebarReverse } from "react-icons/bs";
import { GoDownload, GoGear, GoQuestion, GoSignOut } from "react-icons/go";
import { HiOutlineHome } from "react-icons/hi2";
import { LuGauge, LuLogs } from "react-icons/lu";
import { RiBox3Line, RiKeyLine, RiUser3Line, RiWallet3Line } from "react-icons/ri";

import { MenuDropdown } from "@/components/menu-dropdown";
import { useSession, useSignOut } from "@/domains/auth/hooks";
import { useBilling } from "@/domains/billing/hooks";
import { usePlans } from "@/domains/plans/hooks";
import { useProfile } from "@/domains/profile/hooks";

export function Sidebar() {
  const pathname = usePathname();
  const router = useRouter();
  const signOut = useSignOut();
  const session = useSession();
  const profile = useProfile({ enabled: Boolean(session.data) });
  const billing = useBilling();
  const plans = usePlans();
  const [collapsed, setCollapsed] = useState(false);
  const overviewActive = pathname === "/console/overview";
  const apiKeyPath = "/console/settings/api-keys";
  const apiKeyActive = pathname === apiKeyPath || pathname?.startsWith(`${apiKeyPath}/`);
  const profileActive = pathname === "/console/account";
  const logsActive = pathname === "/console/logs";
  const modelsActive = pathname === "/console/models";
  const billingActive = pathname === "/console/billing";
  const usageActive = pathname === "/console/usage";
  const email = profile.data?.email ?? session.data?.user.email ?? "";
  const name = profile.data?.name ?? session.data?.user.name ?? "";
  const image = profile.data?.image;
  const initials = (profile.data?.name ?? email ?? "?").charAt(0).toUpperCase();
  const planName =
    plans.data?.plans.find((plan) => plan.id === billing.data?.planId)?.name ??
    billing.data?.planId ??
    "Free";
  const tabClass = (active: boolean) =>
    `flex items-center transition-colors ${
      collapsed
        ? "w-fit self-center justify-center rounded-lg p-1"
        : "justify-start gap-2.5 rounded-lg px-2 py-1"
    } ${
      active
        ? "bg-white/10 text-primary"
        : "text-white/70 hover:bg-white/10 hover:text-primary"
    }`;

  return (
    <aside
      className={`flex shrink-0 flex-col border-r border-border-secondary p-1 transition-[width] duration-200 ${
        collapsed ? "w-12" : "w-64"
      }`}
    >
      <div className={collapsed ? "flex justify-center" : "flex items-center justify-between"}>
        {!collapsed && (
          <Image
            src="/icon-white.svg"
            alt="Anyscape Gateway"
            width={20}
            height={16}
            style={{ height: "auto" }}
            priority
          />
        )}
        <button
          type="button"
          aria-label={collapsed ? "Expand sidebar" : "Collapse sidebar"}
          aria-expanded={!collapsed}
          onClick={() => setCollapsed((value) => !value)}
          className="cursor-pointer rounded-sm p-1 text-white/70 transition-colors hover:bg-white/10 hover:text-primary"
        >
          {collapsed ? (
            <BsLayoutSidebarReverse className="size-4" />
          ) : (
            <BsReverseLayoutSidebarReverse className="size-4" />
          )}
        </button>
      </div>
      <nav className="mt-6 flex flex-col gap-1">
        <Link
          href="/console/overview"
          aria-label="Overview"
          aria-current={overviewActive ? "page" : undefined}
          title="Overview"
          className={tabClass(overviewActive)}
        >
          <HiOutlineHome className="size-[18px] shrink-0" />
          {!collapsed && <span className="text-sm font-medium">Overview</span>}
        </Link>
        <Link
          href="/console/account"
          aria-label="My Profile"
          aria-current={profileActive ? "page" : undefined}
          title="My Profile"
          className={tabClass(profileActive)}
        >
          <RiUser3Line className="size-[18px] shrink-0" />
          {!collapsed && <span className="text-sm font-medium">My Profile</span>}
        </Link>
        <div className="mt-6 flex flex-col gap-1">
          {!collapsed && <p className="px-2 text-xs font-medium text-white/70">API</p>}
          <Link
            href={apiKeyPath}
            aria-label="API Keys"
            aria-current={apiKeyActive ? "page" : undefined}
            title="API Keys"
            className={tabClass(Boolean(apiKeyActive))}
          >
            <RiKeyLine className="size-[18px] shrink-0" />
            {!collapsed && <span className="text-sm font-medium">API Keys</span>}
          </Link>
          <Link
            href="/console/logs"
            aria-label="Logs"
            aria-current={logsActive ? "page" : undefined}
            title="Logs"
            className={tabClass(logsActive)}
          >
            <LuLogs className="size-[18px] shrink-0" />
            {!collapsed && <span className="text-sm font-medium">Logs</span>}
          </Link>
          <Link
            href="/console/models"
            aria-label="Models"
            aria-current={modelsActive ? "page" : undefined}
            title="Models"
            className={tabClass(modelsActive)}
          >
            <RiBox3Line className="size-[18px] shrink-0" />
            {!collapsed && <span className="text-sm font-medium">Models</span>}
          </Link>
        </div>
        <div className="mt-6 flex flex-col gap-1">
          {!collapsed && <p className="px-2 text-xs font-medium text-white/70">Subscription</p>}
          <Link
            href="/console/billing"
            aria-label="Billing"
            aria-current={billingActive ? "page" : undefined}
            title="Billing"
            className={tabClass(billingActive)}
          >
            <RiWallet3Line className="size-[18px] shrink-0" />
            {!collapsed && <span className="text-sm font-medium">Billing</span>}
          </Link>
          <Link
            href="/console/usage"
            aria-label="Usage"
            aria-current={usageActive ? "page" : undefined}
            title="Usage"
            className={tabClass(usageActive)}
          >
            <LuGauge className="size-[18px] shrink-0" />
            {!collapsed && <span className="text-sm font-medium">Usage</span>}
          </Link>
        </div>
      </nav>
      <div className="mt-auto">
        <MenuDropdown
          label="Profile menu"
          placement="top"
          triggerClassName={`w-full cursor-pointer rounded-lg p-2 transition-colors hover:bg-primary/10 ${
            collapsed ? "flex items-center justify-center" : "flex items-center gap-2 text-left"
          }`}
          trigger={
            <>
              {image ? (
                <Image
                  src={image}
                  alt=""
                  width={28}
                  height={28}
                  unoptimized
                  className="size-7 shrink-0 rounded-full border border-border object-cover"
                  referrerPolicy="no-referrer"
                />
              ) : (
                <div className="flex size-7 shrink-0 items-center justify-center rounded-full border border-border bg-tertiary text-[11px] text-primary/70">
                  {initials}
                </div>
              )}
              {!collapsed && (
                <div className="min-w-0">
                  <p className="truncate text-sm text-primary/80">{name}</p>
                  <p className="truncate text-xs text-primary/60">{planName}</p>
                </div>
              )}
            </>
          }
          header={
            <div className="flex items-center gap-2">
              <div className="flex size-8 shrink-0 items-center justify-center overflow-hidden rounded-full border border-border text-xs text-primary/70">
                {image ? (
                  <Image src={image} alt="" width={32} height={32} unoptimized className="size-full object-cover" referrerPolicy="no-referrer" />
                ) : (
                  initials
                )}
              </div>
              <div className="min-w-0">
                <p className="truncate text-xs font-medium text-primary">{name || email || "Account"}</p>
                <p className="text-[10px] text-primary/60">{planName}</p>
              </div>
            </div>
          }
          items={[
            {
              label: "Download Argus",
              icon: GoDownload,
              onSelect: () => window.open("https://argus-agent.anyscapelabs.com/download", "_blank", "noopener"),
            },
            {
              label: "Help",
              icon: GoQuestion,
              onSelect: () => window.open("https://anyscapelabs.com/support", "_blank", "noopener"),
            },
            {
              label: "Settings",
              icon: GoGear,
              onSelect: () => router.push("/console/account"),
            },
            {
              label: "Logout",
              icon: GoSignOut,
              dividerBefore: true,
              disabled: signOut.isPending,
              onSelect: () => signOut.mutate(),
            },
          ]}
        />
      </div>
    </aside>
  );
}
