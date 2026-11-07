"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";

const tabs = [
  { label: "Overview", href: "/console/overview" },
  { label: "Subscription", href: "/console/subscription" },
  { label: "Models", href: "/console/models" },
  { label: "Provider", href: "/console/provider" },
  { label: "Settings", href: "/console/settings" },
];

export function NavBar() {
  const pathname = usePathname();

  return (
    <nav className="flex shrink-0 flex-row gap-4 border-b border-border-secondary -ml-2">
      {tabs.map((tab) => {
        const active = pathname === tab.href || pathname?.startsWith(`${tab.href}/`);
        return (
          <Link
            key={tab.label}
            href={tab.href}
            className={`-mb-px inline-flex shrink-0 border-b-2 px-3 py-2 text-sm font-medium transition-colors ${
              active
                ? "border-primary bg-white/10 text-primary"
                : "border-transparent text-white/60 hover:bg-white/5 hover:text-primary"
            }`}
          >
            {tab.label}
          </Link>
        );
      })}
    </nav>
  );
}
