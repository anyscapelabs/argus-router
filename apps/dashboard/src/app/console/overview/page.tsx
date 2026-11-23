"use client";

import { useSession } from "@/domains/auth/hooks";
import { UsageCards, UsageExtraCards } from "@/components/usage-cards";
import { GoArrowUpRight } from "react-icons/go";
import { RiBracketsLine } from "react-icons/ri";

export default function OverviewPage() {
  const session = useSession();
  const username = session.data?.user.name ?? session.data?.user.email ?? "there";

  return (
    <div className="mx-auto w-full max-w-5xl">
      <h1 className="text-2xl font-medium text-primary">Welcome, {username}</h1>
      <div className="mt-6 flex flex-col gap-4">
        <UsageCards />
        <UsageExtraCards />
      </div>
      <div className="mt-4 grid grid-cols-1 gap-4 md:grid-cols-2">
        <a
          href="https://argus-agent.anyscapelabs.com"
          target="_blank"
          rel="noopener noreferrer"
          className="relative flex min-h-10 cursor-pointer flex-col rounded-xl bg-secondary p-4 pb-6 transition-colors duration-200 hover:bg-tertiary"
        >
          <GoArrowUpRight
            aria-hidden="true"
            className="absolute right-4 top-4 size-4 text-primary/60"
          />
          <img src="/argus-logo.svg" alt="Argus" className="size-6" />
          <p className="mt-2 text-sm font-medium text-primary/80">Argus Agent</p>
          <p className="mt-1 text-xs text-primary/60">
            AI Teammate for your everyday work
          </p>
        </a>
        <div className="flex min-h-10 cursor-pointer flex-col rounded-xl bg-secondary p-4 pb-6 transition-colors duration-200 hover:bg-tertiary">
          <div className="flex items-center justify-between gap-2">
            <RiBracketsLine aria-hidden="true" className="size-6 text-primary" />
            <span className="rounded-full border border-border-secondary px-2 py-0.5 text-xs text-primary/60">
              Coming soon
            </span>
          </div>
          <p className="mt-2 text-sm font-medium text-primary/80">API</p>
          <p className="mt-1 text-xs text-primary/60">
            Build with Anyscape model catalog in your own apps
          </p>
        </div>
      </div>
    </div>
  );
}
