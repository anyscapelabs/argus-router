"use client";

import { usePathname } from "next/navigation";
import Link from "next/link";
import { GoGear, GoShield, GoLock, GoCreditCard, GoKey, GoSearch, GoPerson } from "react-icons/go";

const workspaceTabs = [
  { label: "General", href: "/console/settings/general", icon: GoGear },
  { label: "Service Accounts", href: "/console/settings/service-accounts", icon: GoKey },
  { label: "Billing", href: "/console/settings/billing", icon: GoCreditCard },
  { label: "Security", href: "/console/settings/security", icon: GoShield },
  { label: "Privacy", href: "/console/settings/privacy", icon: GoLock },
];

const personalTabs = [
  { label: "Account", href: "/console/settings/account", icon: GoPerson },
];

export function SettingsSidebar() {
  const pathname = usePathname();

  return (
    <aside className="sticky top-10 w-44 shrink-0 self-start px-2 py-3">
      <div className="mb-2">
        <div className="relative">
          <GoSearch className="pointer-events-none absolute left-2 top-1/2 size-3 -translate-y-1/2 text-white/40" />
          <input
            type="text"
            placeholder="Search"
            aria-label="Search settings"
            className="w-full rounded-sm border border-border-secondary bg-transparent py-1 pl-7 pr-2 text-xs text-white outline-none placeholder:text-white/40"
          />
        </div>
      </div>
      <p className="mb-2 mt-4 px-2 text-xs font-medium text-white/40">Workspace</p>
      <nav className="flex flex-col gap-0.5">
        {workspaceTabs.map((tab) => {
          const active = pathname === tab.href || pathname?.startsWith(`${tab.href}/`);
          const Icon = tab.icon;
          return (
            <Link
              key={tab.label}
              href={tab.href}
              className={`flex items-center gap-2 rounded-md px-2 py-1 text-sm font-medium transition-colors ${
                active
                  ? "bg-white/10 text-primary"
                  : "text-white/60 hover:bg-white/5 hover:text-primary"
              }`}
            >
              <Icon size={13} />
              {tab.label}
            </Link>
          );
        })}
      </nav>
      <p className="mb-2 mt-4 px-2 text-xs font-medium text-white/40">Personal</p>
      <nav className="flex flex-col gap-0.5">
        {personalTabs.map((tab) => {
          const active = pathname === tab.href || pathname?.startsWith(`${tab.href}/`);
          const Icon = tab.icon;
          return (
            <Link
              key={tab.label}
              href={tab.href}
              className={`flex items-center gap-2 rounded-md px-2 py-1 text-sm font-medium transition-colors ${
                active
                  ? "bg-white/10 text-primary"
                  : "text-white/60 hover:bg-white/5 hover:text-primary"
              }`}
            >
              <Icon size={13} />
              {tab.label}
            </Link>
          );
        })}
      </nav>
    </aside>
  );
}
