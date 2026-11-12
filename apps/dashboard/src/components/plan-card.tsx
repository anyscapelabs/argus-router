"use client";

import { useState } from "react";
import { FaArrowRight } from "react-icons/fa6";
import { IoCalendarOutline } from "react-icons/io5";
import { CiClock2 } from "react-icons/ci";
import { GoSync } from "react-icons/go";
import { AiOutlineRobot } from "react-icons/ai";
import { GiToken } from "react-icons/gi";

import type { Plan } from "@/domains/plans/api";

function Skeleton({ className = "" }: { className?: string }) {
  return <div className={`animate-pulse rounded-sm bg-white/10 ${className}`} aria-hidden />;
}

export function PlanCardSkeleton() {
  return (
    <div className="relative flex h-auto flex-col border border-border-secondary p-2" aria-label="Loading plan">
      <Skeleton className="absolute top-2 left-2 h-[26px] w-16" />
      <div className="mt-20 flex items-end gap-1">
        <Skeleton className="h-[60px] w-24" />
        <Skeleton className="mb-2 h-3 w-8" />
      </div>
      <Skeleton className="mt-1 h-3 w-20" />
      <Skeleton className="-mx-2 mt-2 h-9 w-[calc(100%+1rem)]" />
      <Skeleton className="mt-2 h-3 w-24" />
      <div className="mt-2 space-y-1.5">
        <div className="flex items-center gap-1.5">
          <Skeleton className="size-3 shrink-0" />
          <Skeleton className="h-3 w-full" />
        </div>
        <div className="flex items-center gap-1.5">
          <Skeleton className="size-3 shrink-0" />
          <Skeleton className="h-3 w-5/6" />
        </div>
        <div className="flex items-center gap-1.5">
          <Skeleton className="size-3 shrink-0" />
          <Skeleton className="h-3 w-full" />
        </div>
        <div className="flex items-center gap-1.5">
          <Skeleton className="size-3 shrink-0" />
          <Skeleton className="h-3 w-2/3" />
        </div>
        <div className="flex items-center gap-1.5">
          <Skeleton className="size-3 shrink-0" />
          <Skeleton className="h-3 w-1/2" />
        </div>
      </div>
      <div className="mt-2 border-t border-border-secondary pt-2">
        <div className="space-y-0.5">
          <Skeleton className="h-[11px] w-full" />
          <Skeleton className="h-[11px] w-5/6" />
          <Skeleton className="h-[11px] w-full" />
          <Skeleton className="h-[11px] w-2/3" />
          <Skeleton className="h-[11px] w-full" />
          <Skeleton className="h-[11px] w-1/2" />
          <Skeleton className="h-[11px] w-3/4" />
          <Skeleton className="h-[11px] w-1/3" />
        </div>
      </div>
    </div>
  );
}

export function PlanCard({ plan, isCurrent }: { plan: Plan; isCurrent: boolean }) {
  const [expanded, setExpanded] = useState(false);
  const modelNames = plan.modelNames ?? [];
  const visible = expanded ? modelNames : modelNames.slice(0, 8);

  return (
    <div className="relative flex h-auto flex-col border border-border-secondary p-2">
      <span className="absolute top-2 left-2 bg-accent px-2 py-1 text-xs font-medium text-background">
        {plan.name}
      </span>
      <span className="font-mono text-6xl text-primary mt-20">
        ${plan.price}
        <span className="text-xs text-white/40">/mo</span>
      </span>
      <p className="pt-1 text-xs font-medium text-white/60">{plan.tagline ?? ""}</p>
      <button
        type="button"
        className="-mx-2 mt-2 flex w-[calc(100%+1rem)] cursor-pointer items-center justify-between border-y border-border-secondary px-3 py-2 text-xs font-medium text-primary transition-colors hover:bg-accent hover:text-background"
      >
        {isCurrent ? "Manage" : "Upgrade"}
        <FaArrowRight className="size-3" />
      </button>
      <h4 className="pt-2 text-xs font-medium text-white/60">Whats Included</h4>
      <div className="mt-2 space-y-1.5">
        {plan.limits && (
          <>
            <div className="flex items-center gap-1.5">
              <GiToken className="size-3 shrink-0 text-white/60" />
              <span className="text-xs text-white/60">{plan.limits.monthlyTokens}</span>
            </div>
            <div className="flex items-center gap-1.5">
              <IoCalendarOutline className="size-3 shrink-0 text-white/60" />
              <span className="text-xs text-white/60">{plan.limits.weeklyTokens}</span>
            </div>
            <div className="flex items-center gap-1.5">
              <GoSync className="size-3 shrink-0 text-white/60" />
              <span className="text-xs text-white/60">{plan.limits.dailyRequests}</span>
            </div>
            <div className="flex items-center gap-1.5">
              <CiClock2 className="size-3 shrink-0 text-white/60" />
              <span className="text-xs text-white/60">{plan.limits.ratePerMin}</span>
            </div>
          </>
        )}
        <div className="flex items-center gap-1.5">
          <AiOutlineRobot className="size-3 shrink-0 text-white/60" />
          <span className="text-xs text-white/60">{plan.modelCount ?? `${plan.models.length} models`}</span>
        </div>
      </div>
      {modelNames.length > 0 && (
        <div className="mt-2 border-t border-border-secondary pt-2">
          <ul className="space-y-0.5">
            {visible.map((m) => (
              <li key={m} className="text-[11px] leading-relaxed text-white/40">
                {m}
              </li>
            ))}
          </ul>
          {modelNames.length > 8 && !expanded && (
            <button
              type="button"
              onClick={() => setExpanded(true)}
              className="mt-1 cursor-pointer text-[11px] font-medium text-white/60 transition-colors hover:text-primary"
            >
              +{modelNames.length - 8} more
            </button>
          )}
          {expanded && modelNames.length > 8 && (
            <button
              type="button"
              onClick={() => setExpanded(false)}
              className="mt-1 cursor-pointer text-[11px] font-medium text-white/60 transition-colors hover:text-primary"
            >
              Show less
            </button>
          )}
        </div>
      )}
    </div>
  );
}
