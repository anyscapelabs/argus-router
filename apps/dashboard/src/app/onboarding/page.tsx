"use client";

import Image from "next/image";
import { useRouter } from "next/navigation";
import { useEffect, useState } from "react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { FaSpinner } from "react-icons/fa";
import { toast } from "sonner";

import { apiFetch } from "@/api/http-client";
import { API_ROUTES } from "@/config/api";
import { authClient } from "@/domains/auth/client";
import { useSession } from "@/domains/auth/hooks";
import { useProfile } from "@/domains/profile/hooks";
import type { Workspace } from "@/domains/workspace/hooks";

type Step = "name" | "organization";

export default function OnboardingPage() {
  const router = useRouter();
  const queryClient = useQueryClient();
  const session = useSession();
  const profile = useProfile({ enabled: Boolean(session.data) });
  const [step, setStep] = useState<Step>("name");
  const [name, setName] = useState("");
  const [organization, setOrganization] = useState("");

  const isEmailUser = profile.data?.providers.includes("credential") ?? false;
  const activeStep: Step = isEmailUser ? step : "organization";

  const complete = useMutation({
    mutationFn: async (input: { name?: string; organization: string }) => {
      if (input.name) {
        const response = await authClient.updateUser({ name: input.name });
        if (response.error) {
          throw new Error(response.error.message ?? "Could not save your name");
        }
      }
      return apiFetch<Workspace>(API_ROUTES.workspace, {
        method: "PATCH",
        body: { name: input.organization },
      });
    },
    onSuccess: (updatedWorkspace) => {
      queryClient.setQueryData(["workspace"], updatedWorkspace);
      void queryClient.invalidateQueries({ queryKey: ["profile"] });
      void queryClient.invalidateQueries({ queryKey: ["auth", "session"] });
      toast.success("You're all set");
      router.replace("/console/overview");
    },
    onError: (error) => {
      toast.error(error.message);
    },
  });

  useEffect(() => {
    if (session.isSuccess && !session.data) {
      router.replace("/auth/login");
    }
  }, [session.isSuccess, session.data, router]);

  const submitName = () => {
    const next = name.trim();
    if (!next) return;
    setName(next);
    setStep("organization");
  };

  const submitOrganization = () => {
    const next = organization.trim();
    if (!next) return;
    complete.mutate({
      name: isEmailUser ? name.trim() : undefined,
      organization: next,
    });
  };

  if (!session.isSuccess || !session.data || !profile.isSuccess) {
    return (
      <main className="flex min-h-full flex-1 items-center justify-center">
        <FaSpinner className="size-5 animate-spin text-white/60" />
      </main>
    );
  }

  return (
    <main className="relative flex min-h-full flex-1 items-center justify-center px-6 py-16">
      <div className="absolute top-6 left-6">
        <Image src="/icon-white.svg" alt="Anyscape Gateway" width={28} height={22} priority />
      </div>

      <section className="relative w-full max-w-sm px-10 py-12">
        <span aria-hidden className="absolute top-0 left-0 h-full w-px bg-white/20" />
        <span aria-hidden className="absolute top-0 right-0 h-full w-px bg-white/20" />
        <span aria-hidden className="absolute top-0 left-0 h-px w-full bg-white/20" />
        <span aria-hidden className="absolute bottom-0 left-0 h-px w-full bg-white/20" />

        {activeStep === "name" ? (
          <>
            <h1 className="text-xl font-medium">What&apos;s your name?</h1>
            <form
              className="mt-6 flex flex-col gap-3"
              autoComplete="off"
              onSubmit={(event) => {
                event.preventDefault();
                submitName();
              }}
            >
              <label className="flex flex-col gap-1.5 text-xs font-medium text-white/70">
                Name
                <input
                  type="text"
                  name="name"
                  value={name}
                  onChange={(event) => setName(event.target.value)}
                  placeholder="Your name"
                  autoComplete="off"
                  autoFocus
                  className="border border-border bg-transparent px-3 py-2 text-sm text-white placeholder:text-white/30 outline-none focus:border-white/40"
                />
              </label>
              <button
                type="submit"
                disabled={!name.trim()}
                className="mt-1 flex w-full cursor-pointer items-center justify-center gap-2 bg-accent px-3 py-2 text-sm font-medium text-black transition-colors hover:bg-white/90 disabled:cursor-not-allowed disabled:opacity-70"
              >
                Continue
              </button>
            </form>
          </>
        ) : (
          <>
            <h1 className="text-xl font-medium">Create your organization</h1>
            <form
              className="mt-6 flex flex-col gap-3"
              autoComplete="off"
              onSubmit={(event) => {
                event.preventDefault();
                submitOrganization();
              }}
            >
              <label className="flex flex-col gap-1.5 text-xs font-medium text-white/70">
                Organization
                <input
                  type="text"
                  name="organization"
                  value={organization}
                  onChange={(event) => setOrganization(event.target.value)}
                  placeholder="Acme Inc."
                  autoComplete="off"
                  autoFocus
                  className="border border-border bg-transparent px-3 py-2 text-sm text-white placeholder:text-white/30 outline-none focus:border-white/40"
                />
              </label>
              <button
                type="submit"
                disabled={!organization.trim() || complete.isPending}
                className="mt-1 flex w-full cursor-pointer items-center justify-center gap-2 bg-accent px-3 py-2 text-sm font-medium text-black transition-colors hover:bg-white/90 disabled:cursor-not-allowed disabled:opacity-70"
              >
                {complete.isPending && <FaSpinner className="size-4 animate-spin" />}
                {complete.isPending ? "Finishing..." : "Get started"}
              </button>
            </form>
          </>
        )}
      </section>

      <p className="absolute inset-x-0 bottom-6 text-center text-xs text-white/50">
        <a href="/terms" className="underline hover:text-white/80">
          Terms and conditions
        </a>
        {" · "}
        <a href="/privacy" className="underline hover:text-white/80">
          Privacy
        </a>
      </p>
    </main>
  );
}
