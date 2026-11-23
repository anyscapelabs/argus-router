"use client";

import { useEffect } from "react";
import { FaSpinner } from "react-icons/fa";
import { GoGlobe } from "react-icons/go";
import { IoExtensionPuzzleOutline } from "react-icons/io5";
import { MdOutlineDataUsage } from "react-icons/md";
import { VscPlug, VscVoiceMode } from "react-icons/vsc";
import { SiGooglegemini } from "react-icons/si";
import type { ComponentType } from "react";

import { useCheckout } from "@/domains/billing/hooks";
import { usePlans } from "@/domains/plans/hooks";

type PlanFeature = { Icon: ComponentType<{ className?: string }>; label: string };

const planOrder = ["free", "starter", "max", "max-plus"] as const;
type PlanId = (typeof planOrder)[number];

const planTitles: Record<PlanId, string> = {
  free: "Try Argus",
  starter: "Expanded Access",
  max: "Advanced Capabilities",
  "max-plus": "Heavy Usage",
};

const planDescriptions: Record<PlanId, string> = {
  free: "Explore Argus with essential models.",
  starter: "More room for everyday projects.",
  max: "Advanced models for complex work.",
  "max-plus": "The highest limits for heavy usage.",
};

const planPrices: Record<PlanId, string> = {
  free: "$0",
  starter: "$5",
  max: "$10",
  "max-plus": "$15",
};

const planFeatures: Record<PlanId, PlanFeature[]> = {
  free: [
    { Icon: MdOutlineDataUsage, label: "Limited Monthly usage" },
    { Icon: GoGlobe, label: "Limited Web search" },
    { Icon: VscVoiceMode, label: "Limited Voice chats" },
    { Icon: IoExtensionPuzzleOutline, label: "Unlimited Integrations" },
    { Icon: VscPlug, label: "Limited Skills Access" },
    { Icon: SiGooglegemini, label: "Lite Models Only" },
  ],
  starter: [
    { Icon: MdOutlineDataUsage, label: "Expanded Monthly usage" },
    { Icon: GoGlobe, label: "More Web searches" },
    { Icon: VscVoiceMode, label: "More Voice chats" },
    { Icon: IoExtensionPuzzleOutline, label: "Unlimited Integrations" },
    { Icon: VscPlug, label: "Standard Skills Access" },
    { Icon: SiGooglegemini, label: "Standard Models" },
  ],
  max: [
    { Icon: MdOutlineDataUsage, label: "High Monthly usage" },
    { Icon: GoGlobe, label: "More Web searches" },
    { Icon: VscVoiceMode, label: "More Voice chats" },
    { Icon: IoExtensionPuzzleOutline, label: "Unlimited Integrations" },
    { Icon: VscPlug, label: "Advanced Skills Access" },
    { Icon: SiGooglegemini, label: "Premium Models" },
  ],
  "max-plus": [
    { Icon: MdOutlineDataUsage, label: "Heavy Monthly usage" },
    { Icon: GoGlobe, label: "Most Web searches" },
    { Icon: VscVoiceMode, label: "Most Voice chats" },
    { Icon: IoExtensionPuzzleOutline, label: "Unlimited Integrations" },
    { Icon: VscPlug, label: "Full Skills Access" },
    { Icon: SiGooglegemini, label: "Ultra Models" },
  ],
};

export function UpgradePlansModal({
  open,
  onClose,
  currentPlanId,
  availablePlanIds,
}: {
  open: boolean;
  onClose: () => void;
  currentPlanId: string;
  availablePlanIds: string[];
}) {
  const checkout = useCheckout();
  const plans = usePlans();

  useEffect(() => {
    if (!open) return;
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape" && !checkout.isPending) onClose();
    };
    document.addEventListener("keydown", onKeyDown);
    return () => document.removeEventListener("keydown", onKeyDown);
  }, [open, onClose, checkout.isPending]);

  if (!open) return null;

  const visiblePlans = planOrder.filter(
    (id) =>
      (availablePlanIds.length === 0 || availablePlanIds.includes(id)) &&
      (id !== "free" || currentPlanId === "free"),
  );

  return (
    <div
      role="presentation"
      onClick={() => {
        if (!checkout.isPending) onClose();
      }}
      className="fixed inset-0 z-50 flex items-center justify-center overflow-y-auto bg-background/80 p-4"
    >
      <div
        role="dialog"
        aria-modal="true"
        aria-label="Choose your plan"
        onClick={(event) => event.stopPropagation()}
        className="w-full max-w-6xl rounded-xl border border-border bg-secondary p-6"
      >
        <div className="flex flex-col items-center gap-0">
          <h2 className="text-center text-2xl font-medium text-primary">Choose your plan</h2>
          <p className="mt-1 max-w-md text-center text-sm leading-relaxed text-primary/60">
            Start experimenting for free or unlock more usage for your agent
          </p>
        </div>
        <div className="mt-6 grid grid-cols-[repeat(auto-fit,minmax(240px,1fr))] gap-4">
          {visiblePlans.map((id) => {
            const name = plans.data?.plans.find((plan) => plan.id === id)?.name ?? id;
            const isCurrent = id === currentPlanId;
            const pending = checkout.isPending && checkout.variables?.planId === id;
            return (
              <div key={id} className="min-h-96 rounded-xl border border-border p-4">
                <span className="block text-sm font-medium text-primary">{name}</span>
                <p className="mt-4 text-2xl font-medium text-primary">{planTitles[id]}</p>
                <p className="mt-2 text-sm text-primary/60">{planDescriptions[id]}</p>
                <button
                  type="button"
                  disabled={isCurrent || checkout.isPending}
                  onClick={() => checkout.mutate({ planId: id })}
                  className={`mt-4 flex w-full cursor-pointer items-center justify-center gap-2 rounded-full px-4 py-3 text-sm font-medium transition-colors disabled:cursor-not-allowed disabled:opacity-70 ${
                    isCurrent
                      ? "border border-border text-primary/70 hover:bg-primary/5"
                      : "bg-accent text-background hover:opacity-80"
                  }`}
                >
                  {pending && <FaSpinner className="size-4 animate-spin" />}
                  {pending
                    ? "Opening checkout..."
                    : isCurrent
                      ? "Your Current Plan"
                      : "Start Now"}
                </button>
                {id !== "free" && <p className="mt-2 text-xs text-primary/50">No commitment. Cancel anytime</p>}
                <p className="mt-3 text-left">
                  <span className="text-4xl font-medium text-primary">{planPrices[id]}</span>
                  <span className="text-sm text-primary/60">/month</span>
                </p>
                <div className="mt-4 flex flex-col gap-2">
                  {planFeatures[id].map(({ Icon, label }) => (
                    <div key={label} className="flex items-center gap-2">
                      <Icon className="size-5 text-primary/60" />
                      <span className="text-sm font-medium text-primary/70">{label}</span>
                    </div>
                  ))}
                </div>
              </div>
            );
          })}
        </div>
      </div>
    </div>
  );
}
