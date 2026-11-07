"use client";

import { usePathname } from "next/navigation";
import Link from "next/link";
import { GoGear, GoShield, GoLock, GoCreditCard, GoKey } from "react-icons/go";

const tabs = [
  { label: "General", href: "/console/settings/general", icon: GoGear },
  { label: "Service Accounts", href: "/console/settings/service-accounts", icon: GoKey },
  { label: "Billing", href: "/console/settings/billing", icon: GoCreditCard },
  { label: "Security", href: "/console/settings/security", icon: GoShield },
  { label: "Privacy", href: "/console/settings/privacy", icon: GoLock },
];

export function SettingsSidebar() {
  const pathname = usePathname();

  return (
    <aside className="ml-10 w-44 shrink-0 px-2 py-3">
      <nav className="flex flex-col gap-0.5">
        {tabs.map((tab) => {
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
