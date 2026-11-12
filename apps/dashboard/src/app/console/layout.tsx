"use client";

import { usePathname, useRouter } from "next/navigation";
import { Suspense, useEffect } from "react";

import { useSession } from "@/domains/auth/hooks";
import { NavBar } from "@/components/nav-bar";
import { SettingsSidebar } from "@/components/settings-sidebar";
import { TopBar } from "@/components/top-bar";

export default function ConsoleLayout({ children }: { children: React.ReactNode }) {
  const router = useRouter();
  const pathname = usePathname();
  const session = useSession();
  const isSettings = pathname === "/console/settings" || pathname?.startsWith("/console/settings/");
  const isServiceAccountDetail =
    /^\/console\/settings\/service-accounts\/[^/]+/.test(pathname ?? "");

  useEffect(() => {
    if (session.isSuccess && !session.data) {
      router.replace("/auth/login");
    }
  }, [session.isSuccess, session.data, router]);

  return (
    <div className="flex h-dvh flex-col overflow-hidden">
      <TopBar />
      <main className="flex min-h-0 min-w-0 flex-1 flex-col px-2 pb-2">
        <div className="flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden rounded-md bg-secondary px-2 pb-2">
          <Suspense fallback={<nav className="flex shrink-0 flex-row gap-4 border-b border-border-secondary -ml-2" />}>
            <NavBar />
          </Suspense>
          {isSettings ? (
            isServiceAccountDetail ? (
              <div className="min-h-0 min-w-0 w-full flex-1 overflow-auto">
                <div className="mx-auto w-full max-w-5xl px-2 pb-1 pt-10">
                  <Suspense fallback={<div className="flex flex-col gap-2 p-4"><div className="h-3 w-40 animate-pulse rounded-sm bg-white/10" /><div className="h-3 w-64 animate-pulse rounded-sm bg-white/10" /></div>}>
                    {children}
                  </Suspense>
                </div>
              </div>
            ) : (
              <div className="min-h-0 min-w-0 w-full flex-1 overflow-y-auto overflow-x-hidden">
                <div className="mx-auto flex w-full max-w-5xl gap-8 pt-10 pb-1">
                  <Suspense fallback={<aside className="sticky top-10 w-44 shrink-0 self-start px-2 py-3" />}>
                    <SettingsSidebar />
                  </Suspense>
                  <div className="min-w-0 flex-1 px-2 py-1">
                    <Suspense fallback={<div className="flex flex-col gap-2 p-4"><div className="h-3 w-40 animate-pulse rounded-sm bg-white/10" /><div className="h-3 w-64 animate-pulse rounded-sm bg-white/10" /></div>}>
                      {children}
                    </Suspense>
                  </div>
                </div>
              </div>
            )
          ) : (
            <div className="pt-10 min-h-0 min-w-0 w-full flex-1 overflow-y-auto overflow-x-hidden px-2 py-1">
              <Suspense fallback={<div className="flex flex-col gap-2 p-4"><div className="h-3 w-40 animate-pulse rounded-sm bg-white/10" /><div className="h-3 w-64 animate-pulse rounded-sm bg-white/10" /></div>}>
                {children}
              </Suspense>
            </div>
          )}
        </div>
      </main>
    </div>
  );
}
