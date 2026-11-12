"use client";

import { useRouter } from "next/navigation";
import { useEffect } from "react";
import { FaSpinner } from "react-icons/fa";

import { useSession } from "@/domains/auth/hooks";

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
