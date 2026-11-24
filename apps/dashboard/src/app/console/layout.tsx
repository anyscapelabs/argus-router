"use client";

import { usePathname, useRouter, useSearchParams } from "next/navigation";
import { Suspense, useEffect } from "react";

import { withNext } from "@/domains/auth/client";
import { useSession } from "@/domains/auth/hooks";
import { PageLoading } from "@/components/page-loading";
import { Sidebar } from "@/components/sidebar";

export default function ConsoleLayout({ children }: { children: React.ReactNode }) {
  const router = useRouter();
  const pathname = usePathname();
  const searchParams = useSearchParams();
  const session = useSession();

  useEffect(() => {
    if (session.isSuccess && !session.data) {
      const nextPath = `${pathname}${searchParams.toString() ? `?${searchParams.toString()}` : ""}`;
      router.replace(withNext("/auth/login", nextPath));
    }
  }, [session.isSuccess, session.data, router, pathname, searchParams]);

  return (
    <div className="flex h-dvh flex-col overflow-hidden">
      <main className="flex min-h-0 min-w-0 flex-1">
        <Suspense fallback={<aside className="w-64 shrink-0 border-r border-border-secondary px-4 py-4" />}>
          <Sidebar />
        </Suspense>
        <div className="flex min-h-0 min-w-0 flex-1 flex-col">
          <div className="min-h-0 min-w-0 w-full flex-1 overflow-y-auto overflow-x-hidden px-2 py-1 pt-10">
            <Suspense fallback={<PageLoading />}>
              {children}
            </Suspense>
          </div>
        </div>
      </main>
    </div>
  );
}
