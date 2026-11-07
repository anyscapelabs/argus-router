"use client";

import { useRouter } from "next/navigation";
import { useEffect } from "react";
import { FaSpinner } from "react-icons/fa";

import { useSession } from "@/domains/auth/hooks";

/**
 * Root route — the dashboard entry point.
 *
 * - Signed-in users land on the console (`/console/overview`) immediately.
 * - Signed-out users land on the login page.
 *
 * Session lookup goes through the shared QueryClient, so this guard and the
 * console page both read from one cached `useSession()` request.
 */
export default function Home() {
  const router = useRouter();
  const session = useSession();

  useEffect(() => {
    if (session.isPending) {
      return;
    }
    if (session.isSuccess && session.data) {
      router.replace("/console/overview");
    } else if (session.isSuccess) {
      router.replace("/auth/login");
    }
  }, [session.isPending, session.isSuccess, session.data, router]);

  return (
    <main className="flex min-h-full flex-1 items-center justify-center">
      <FaSpinner className="size-5 animate-spin text-white/60" />
    </main>
  );
}
