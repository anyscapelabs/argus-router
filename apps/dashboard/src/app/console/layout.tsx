"use client";

import { usePathname, useRouter } from "next/navigation";
import { useEffect } from "react";

import { useSession } from "@/domains/auth/hooks";
import { NavBar } from "@/components/nav-bar";
import { SettingsSidebar } from "@/components/settings-sidebar";
import { TopBar } from "@/components/top-bar";

/**
 * Guards the console layout — every route under `/console/*` requires a valid
 * session. Signed-out users are sent to `/auth/login`. Session data comes from
 * the shared QueryClient (`useSession()`), so this guard and the overview page
 * read from one cached request.
 *
 * The guard redirects; it does not withhold the segment. `children` is always
 * rendered, because returning a spinner in place of `{children}` drops the page
 * from the server render entirely — Next.js then can't validate it for instant
 * navigation (`instant-unrendered-segment`) and reports the page as "dropped
 * from rendering". Loading and error UI belong to the pages themselves.
 */
export default function ConsoleLayout({ children }: { children: React.ReactNode }) {
  const router = useRouter();
  const pathname = usePathname();
  const session = useSession();
  const isSettings = pathname === "/console/settings" || pathname?.startsWith("/console/settings/");

  useEffect(() => {
    if (session.isSuccess && !session.data) {
      router.replace("/auth/login");
    }
  }, [session.isSuccess, session.data, router]);

  return (
    /* h-dvh pins the whole shell to the viewport: ancestors here have auto
       heights, so `min-h-full` chains never resolve and the card would grow
       with its content, pushing the scroll out to the page. The only
       scrollbar is the content column's below. */
    <div className="flex h-dvh flex-col overflow-hidden">
      <TopBar />
      <main className="flex min-h-0 min-w-0 flex-1 flex-col p-2">
        <div className="flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden rounded-md bg-secondary p-2">
          {/* The tab bar stays pinned at the top of the card — only the
              content column below scrolls. Settings adds its own left rail. */}
          <NavBar />
          {isSettings ? (
            <div className="mt-6 flex min-h-0 min-w-0 w-full flex-1">
              <SettingsSidebar />
              <div className="min-h-0 min-w-0 flex-1 overflow-y-auto overflow-x-hidden px-2 py-1">
                <div className="mx-auto w-full max-w-2xl">{children}</div>
              </div>
            </div>
          ) : (
            <div className="mt-10 min-h-0 min-w-0 w-full flex-1 overflow-y-auto overflow-x-hidden px-2 py-1">{children}</div>
          )}
        </div>
      </main>
    </div>
  );
}
