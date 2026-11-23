"use client";

import Image from "next/image";
import { useRouter } from "next/navigation";
import { useEffect, useState, type ComponentType } from "react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { FaSpinner } from "react-icons/fa";
import { GoGlobe } from "react-icons/go";
import { IoExtensionPuzzleOutline } from "react-icons/io5";
import { MdOutlineDataUsage } from "react-icons/md";
import { SiGooglegemini } from "react-icons/si";
import { VscPlug, VscVoiceMode } from "react-icons/vsc";
import { toast } from "sonner";

import { apiFetch } from "@/api/http-client";
import { API_ROUTES } from "@/config/api";
import { authClient } from "@/domains/auth/client";
import { useBilling, useCheckout } from "@/domains/billing/hooks";
import { useCreateApiKey } from "@/domains/keys/hooks";
import type { Plan } from "@/domains/plans/api";
import { usePlans } from "@/domains/plans/hooks";
import { useSession } from "@/domains/auth/hooks";
import { useProfile } from "@/domains/profile/hooks";
import type { Workspace } from "@/domains/workspace/hooks";

type Step = "profile" | "project" | "agent" | "done";
type PlanFeature = { Icon: ComponentType<{ className?: string }>; label: string };

const fieldClassName =
  "w-96 rounded-full border border-border bg-transparent px-4 py-3 text-sm text-white placeholder:text-white/50 outline-none focus:border-white/40";

const planTitles: Record<string, string> = {
  Free: "Try Argus",
  Starter: "Expanded Acess",
  Max: "Advanced Capabilities",
  "Max Plus": "Heavy Usage",
};
const planDescriptions: Record<string, string> = {
  Free: "Explore Argus with essential models.",
  Starter: "More room for everyday projects.",
  Max: "Advanced models for complex work.",
  "Max Plus": "The highest limits for heavy usage.",
};
const planPrices: Record<string, string> = {
  Free: "$0",
  Starter: "$5",
  Max: "$10",
  "Max Plus": "$15",
};
const planFeatures: Record<string, PlanFeature[]> = {
  Free: [
    { Icon: MdOutlineDataUsage, label: "Limited Monthly usage" },
    { Icon: GoGlobe, label: "Limited Web search" },
    { Icon: VscVoiceMode, label: "Limited Voice chats" },
    { Icon: IoExtensionPuzzleOutline, label: "Unlimited Intergrations" },
    { Icon: VscPlug, label: "Limited Skills Acess" },
    { Icon: SiGooglegemini, label: "Lite Models Only" },
  ],
  Starter: [
    { Icon: MdOutlineDataUsage, label: "Expanded Monthly usage" },
    { Icon: GoGlobe, label: "More Web searches" },
    { Icon: VscVoiceMode, label: "More Voice chats" },
    { Icon: IoExtensionPuzzleOutline, label: "Unlimited Integrations" },
    { Icon: VscPlug, label: "Standard Skills Access" },
    { Icon: SiGooglegemini, label: "Standard Models" },
  ],
  Max: [
    { Icon: MdOutlineDataUsage, label: "High Monthly usage" },
    { Icon: GoGlobe, label: "More Web searches" },
    { Icon: VscVoiceMode, label: "More Voice chats" },
    { Icon: IoExtensionPuzzleOutline, label: "Unlimited Integrations" },
    { Icon: VscPlug, label: "Advanced Skills Access" },
    { Icon: SiGooglegemini, label: "Premium Models" },
  ],
  "Max Plus": [
    { Icon: MdOutlineDataUsage, label: "Heavy Monthly usage" },
    { Icon: GoGlobe, label: "Most Web searches" },
    { Icon: VscVoiceMode, label: "Most Voice chats" },
    { Icon: IoExtensionPuzzleOutline, label: "Unlimited Integrations" },
    { Icon: VscPlug, label: "Full Skills Access" },
    { Icon: SiGooglegemini, label: "Ultra Models" },
  ],
};

function ProfileStep({
  initialName,
  email,
  age,
  onAgeChange,
  ageValid,
  isPending,
  onSubmit,
}: {
  initialName: string;
  email: string;
  age: string;
  onAgeChange: (value: string) => void;
  ageValid: boolean;
  isPending: boolean;
  onSubmit: (name: string) => void;
}) {
  const [name, setName] = useState(initialName);

  return (
    <div className="flex flex-col items-center gap-3 text-center">
      <Image src="/icon-white.svg" alt="Anyscape Platform" width={40} height={32} priority />
      <div className="flex flex-col items-center gap-1">
        <h1 className="text-2xl font-medium whitespace-nowrap">
          Welcome to Anyscape Platform
        </h1>
        <p className="w-96 text-base leading-relaxed text-white/60">
          Tell us a bit about yourself to get your workspace ready.
        </p>
      </div>
      <label className="mt-2 flex w-96 flex-col gap-1.5 text-left text-xs font-medium text-white/70">
        Full name
        <input
          type="text"
          name="name"
          value={name}
          onChange={(event) => setName(event.target.value)}
          placeholder="Full name"
          autoComplete="name"
          className={fieldClassName}
        />
      </label>
      <label className="flex w-96 flex-col gap-1.5 text-left text-xs font-medium text-white/70">
        Age
        <input
          type="number"
          name="age"
          min={18}
          max={120}
          value={age}
          onChange={(event) => onAgeChange(event.target.value)}
          placeholder="Age"
          className={fieldClassName}
        />
      </label>
      <button
        type="button"
        disabled={!name.trim() || !ageValid || isPending}
        onClick={() => onSubmit(name)}
        className="flex w-96 cursor-pointer items-center justify-center gap-2 rounded-full bg-accent px-4 py-3 text-sm font-medium text-black transition-colors hover:bg-white/90 disabled:cursor-not-allowed disabled:opacity-70"
      >
        {isPending && <FaSpinner className="size-4 animate-spin" />}
        {isPending ? "Saving..." : "Continue"}
      </button>
      <p className="text-xs text-white/50">Signed in as {email}</p>
    </div>
  );
}

export default function OnboardingPage() {
  const router = useRouter();
  const queryClient = useQueryClient();
  const session = useSession();
  const profile = useProfile({ enabled: Boolean(session.data) });
  const billing = useBilling();
  const plansQuery = usePlans();
  const checkout = useCheckout();
  const createKey = useCreateApiKey();

  const [step, setStep] = useState<Step>("profile");
  const [age, setAge] = useState("");
  const [projectName, setProjectName] = useState("Default Project");
  const [keyName, setKeyName] = useState("");
  const [generatedKey, setGeneratedKey] = useState<string | null>(null);
  const [modalOpen, setModalOpen] = useState(false);
  const [copied, setCopied] = useState(false);
  const [pendingPlanId, setPendingPlanId] = useState<string | null>(null);

  const saveProfile = useMutation({
    mutationFn: async (input: { name: string; age: number }) => {
      const response = await authClient.updateUser({
        name: input.name,
        age: input.age,
      } as unknown as Parameters<typeof authClient.updateUser>[0]);
      if (response.error) {
        throw new Error(response.error.message ?? "Could not save your profile");
      }
    },
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: ["profile"] });
      void queryClient.invalidateQueries({ queryKey: ["auth", "session"] });
    },
  });

  const saveProject = useMutation({
    mutationFn: (input: { project: string }) =>
      apiFetch<Workspace>(API_ROUTES.workspace, {
        method: "PATCH",
        body: { name: input.project },
      }),
    onSuccess: (updatedWorkspace) => {
      queryClient.setQueryData(["workspace"], updatedWorkspace);
      void queryClient.invalidateQueries({ queryKey: ["profile"] });
      void queryClient.invalidateQueries({ queryKey: ["auth", "session"] });
    },
  });

  const completeOnboarding = useMutation({
    mutationFn: () => apiFetch(API_ROUTES.onboarding, { method: "POST" }),
  });

  useEffect(() => {
    if (session.isSuccess && !session.data) {
      router.replace("/auth/login");
    }
  }, [session.isSuccess, session.data, router]);

  const email = session.data?.user.email ?? "your account";
  const parsedAge = Number(age);
  const ageValid = Number.isInteger(parsedAge) && parsedAge >= 18 && parsedAge <= 120;
  const isSavingProfile = saveProfile.isPending;
  const isSavingProject = saveProject.isPending;

  const submitProfile = (value: string) => {
    const next = value.trim();
    if (!next) {
      toast.error("Enter your full name.");
      return;
    }
    if (!ageValid) {
      toast.error("You must be at least 18 years old.");
      return;
    }
    saveProfile.mutate(
      { name: next, age: parsedAge },
      {
        onSuccess: () => setStep("project"),
        onError: (error) => toast.error(error.message),
      },
    );
  };

  const submitProject = () => {
    const next = projectName.trim();
    if (!next) return;
    saveProject.mutate(
      { project: next },
      {
        onSuccess: () => setStep("agent"),
        onError: (error) => toast.error(error.message),
      },
    );
  };

  const generateKey = () => {
    const next = keyName.trim();
    if (!next) return;
    createKey.mutate(
      { name: next },
      {
        onSuccess: (result) => {
          setGeneratedKey(result.key);
          setModalOpen(true);
        },
        onError: (error) => toast.error(error.message),
      },
    );
  };

  const closeModal = () => {
    setModalOpen(false);
    setStep("done");
  };

  const copyKey = () => {
    if (!generatedKey) return;
    void navigator.clipboard?.writeText(generatedKey);
    setCopied(true);
    setTimeout(() => setCopied(false), 1500);
  };

  const finish = async (after?: () => void): Promise<boolean> => {
    try {
      await completeOnboarding.mutateAsync();
      toast.success("You're all set");
      after?.();
      return true;
    } catch (error) {
      toast.error(error instanceof Error ? error.message : "Could not finish onboarding");
      return false;
    }
  };

  const plans: Plan[] = plansQuery.data?.plans ?? [];
  const currentPlanId = billing.data?.planId ?? "free";
  const availablePlanIds = billing.data?.availablePlanIds ?? [];

  if (!session.isSuccess || !session.data || !profile.isSuccess) {
    return (
      <main className="flex min-h-full flex-1 items-center justify-center">
        <FaSpinner className="size-5 animate-spin text-white/60" />
      </main>
    );
  }

  return (
    <main className="relative flex min-h-full flex-1 flex-col items-center justify-center gap-8 px-6 py-16">
      <div className="absolute top-6 left-6 text-xl font-medium text-white/80">
        Anyscape Platform
      </div>

      <section
        className={
          step === "done"
            ? "flex w-full max-w-7xl flex-col items-center gap-8"
            : "flex w-96 flex-col items-center gap-8"
        }
      >
        {step === "profile" ? (
          <ProfileStep
            initialName={session.data.user.name ?? ""}
            email={email}
            age={age}
            onAgeChange={setAge}
            ageValid={ageValid}
            isPending={isSavingProfile}
            onSubmit={submitProfile}
          />
        ) : step === "project" ? (
          <div className="flex flex-col items-center gap-3 text-center">
            <Image src="/icon-white.svg" alt="Anyscape Platform" width={40} height={32} priority />
            <div className="flex flex-col items-center gap-1">
              <h1 className="text-2xl font-medium whitespace-nowrap">
                Welcome to Anyscape Platform
              </h1>
              <p className="w-96 text-base leading-relaxed text-white/60">
                Give your workspace a name so your projects stay organized and ready from day one.
              </p>
            </div>
            <label className="mt-2 flex w-96 flex-col gap-1.5 text-left text-xs font-medium text-white/70">
              Project name
              <input
                type="text"
                name="projectName"
                value={projectName}
                onChange={(event) => setProjectName(event.target.value)}
                placeholder="Project name"
                autoComplete="off"
                className={fieldClassName}
              />
            </label>
            <button
              type="button"
              disabled={!projectName.trim() || isSavingProject}
              onClick={submitProject}
              className="flex w-96 cursor-pointer items-center justify-center gap-2 rounded-full bg-accent px-4 py-3 text-sm font-medium text-black transition-colors hover:bg-white/90 disabled:cursor-not-allowed disabled:opacity-70"
            >
              {isSavingProject && <FaSpinner className="size-4 animate-spin" />}
              {isSavingProject ? "Saving..." : "Continue"}
            </button>
            <p className="text-xs text-white/50">Signed in as {email}</p>
          </div>
        ) : step === "agent" ? (
          <div className="flex w-96 flex-col gap-8">
            <div className="size-12 self-center rounded-lg border border-border bg-secondary" />
            <div className="flex flex-col items-center gap-1">
              <h1 className="text-center text-2xl font-medium">Connect your agent</h1>
              <p className="w-96 text-center text-base leading-relaxed text-white/60">
                Create your first API key to connect to your agent
                <br />
                and start working.
              </p>
            </div>
            <label className="flex w-96 flex-col gap-1.5 text-left text-xs font-medium text-white/70">
              Api key name
              <input
                type="text"
                name="apiKeyName"
                value={keyName}
                onChange={(event) => setKeyName(event.target.value)}
                placeholder="Api key name"
                autoComplete="off"
                className={fieldClassName}
              />
            </label>
            <button
              type="button"
              disabled={!keyName.trim() || createKey.isPending}
              onClick={generateKey}
              className="flex w-96 cursor-pointer items-center justify-center gap-2 rounded-full bg-accent px-4 py-3 text-sm font-medium text-black transition-colors hover:bg-white/90 disabled:cursor-not-allowed disabled:opacity-70"
            >
              {createKey.isPending && <FaSpinner className="size-4 animate-spin" />}
              {createKey.isPending ? "Generating..." : "Generate api key"}
            </button>
            <button
              type="button"
              disabled={createKey.isPending}
              onClick={() => setStep("done")}
              className="cursor-pointer text-sm text-white/60 hover:text-white disabled:cursor-not-allowed disabled:opacity-70"
            >
              I&apos;ll do this later
            </button>
          </div>
        ) : (
          <>
            <div className="flex flex-col items-center gap-0">
              <h1 className="text-center text-2xl font-medium">Choose your plan</h1>
              <p className="w-96 text-center text-base leading-relaxed text-white/60">
                Start experimenting for free or unlock more usage for your agent
              </p>
            </div>
            <div className="grid w-full grid-cols-[repeat(auto-fit,minmax(260px,1fr))] gap-4">
              {plans.map((plan) => (
                <div key={plan.id} className="min-h-96 rounded-xl border border-border p-4">
                  <span className="block text-sm font-medium text-primary">{plan.name}</span>
                  {planTitles[plan.name] && (
                    <p className="mt-4 text-2xl font-medium text-primary">{planTitles[plan.name]}</p>
                  )}
                  {planDescriptions[plan.name] && (
                    <p className="mt-2 text-sm text-white/60">{planDescriptions[plan.name]}</p>
                  )}
                  <button
                    type="button"
                    disabled={
                      plan.id === currentPlanId ||
                      pendingPlanId !== null ||
                      checkout.isPending
                    }
                    onClick={() => {
                      if (!availablePlanIds.includes(plan.id)) return;
                      setPendingPlanId(plan.id);
                      void finish(() =>
                        checkout.mutate(
                          { planId: plan.id },
                          { onError: () => setPendingPlanId(null) },
                        ),
                      ).then((ok) => {
                        if (!ok) setPendingPlanId(null);
                      });
                    }}
                    className={
                      plan.id === currentPlanId
                        ? "mt-4 flex w-full cursor-pointer items-center justify-center gap-2 rounded-full border border-border px-4 py-3 text-sm font-medium text-white/80 transition-colors hover:bg-white/5 disabled:cursor-default disabled:opacity-70"
                        : "mt-4 flex w-full cursor-pointer items-center justify-center gap-2 rounded-full bg-accent px-4 py-3 text-sm font-medium text-black transition-colors hover:bg-white/90 disabled:cursor-not-allowed disabled:opacity-70"
                    }
                  >
                    {pendingPlanId === plan.id && <FaSpinner className="size-4 animate-spin" />}
                    {pendingPlanId === plan.id
                      ? "Opening checkout..."
                      : plan.id === currentPlanId
                        ? "Your Current Plan"
                        : "Start Now"}
                  </button>
                  {plan.name !== "Free" && (
                    <p className="mt-2 text-xs text-white/50">No commitment. Cancel anytime</p>
                  )}
                  {planPrices[plan.name] && (
                    <p className="mt-3 text-left">
                      <span className="text-4xl font-medium text-primary">{planPrices[plan.name]}</span>
                      <span className="text-sm text-white/60">/month</span>
                    </p>
                  )}
                  <div className="mt-4 flex flex-col gap-2">
                    {(planFeatures[plan.name] ?? []).map(({ Icon, label }) => (
                      <div key={label} className="flex items-center gap-2">
                        <Icon className="size-5 text-white/60" />
                        <span className="text-sm font-medium text-white/80">{label}</span>
                      </div>
                    ))}
                  </div>
                </div>
              ))}
            </div>
            <button
              type="button"
              disabled={pendingPlanId !== null}
              onClick={() => {
                setPendingPlanId("free");
                void finish(() => router.replace("/console/overview")).then((ok) => {
                  if (!ok) setPendingPlanId(null);
                });
              }}
              className="flex w-96 cursor-pointer items-center justify-center gap-2 rounded-full bg-accent px-4 py-3 text-sm font-medium text-black transition-colors hover:bg-white/90 disabled:cursor-not-allowed disabled:opacity-70"
            >
              {pendingPlanId === "free" && <FaSpinner className="size-4 animate-spin" />}
              {pendingPlanId === "free" ? "Opening dashboard..." : "Try Free"}
            </button>
          </>
        )}
      </section>

      {modalOpen && generatedKey && (
        <div
          role="presentation"
          onClick={closeModal}
          className="fixed inset-0 z-50 flex items-center justify-center bg-black/70 p-4"
        >
          <div
            role="dialog"
            aria-modal="true"
            aria-label="API key created"
            onClick={(event) => event.stopPropagation()}
            className="w-96 rounded-xl border border-border bg-secondary p-4"
          >
            <h2 className="text-lg font-medium text-primary">API key created</h2>
            <p className="mt-1 text-xs text-white/50">
              Copy it now. You won&apos;t see it again.
            </p>
            <div className="mt-4 flex items-center gap-3">
              <code className="flex-1 truncate text-sm text-primary">{generatedKey}</code>
              <button
                type="button"
                onClick={copyKey}
                className="cursor-pointer rounded-full border border-border px-3 py-1.5 text-xs font-medium text-white/80 transition-colors hover:bg-white/5"
              >
                {copied ? "Copied" : "Copy"}
              </button>
            </div>
            <button
              type="button"
              onClick={closeModal}
              className="mt-4 w-full cursor-pointer rounded-full bg-accent px-4 py-3 text-sm font-medium text-black transition-colors hover:bg-white/90"
            >
              Close and continue
            </button>
          </div>
        </div>
      )}

      <div className="flex items-center gap-4 text-xs text-white/50">
        <a
          href="https://argus-agent.anyscapelabs.com/privacy-policy"
          target="_blank"
          rel="noopener noreferrer"
          className="underline hover:text-white/80"
        >
          Privacy Policy
        </a>
        <a
          href="https://argus-agent.anyscapelabs.com/terms-of-service"
          target="_blank"
          rel="noopener noreferrer"
          className="underline hover:text-white/80"
        >
          Terms of Use
        </a>
      </div>
    </main>
  );
}
