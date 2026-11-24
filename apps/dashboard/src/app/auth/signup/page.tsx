"use client";

import { useRouter, useSearchParams } from "next/navigation";
import { useEffect } from "react";
import { FaGithub, FaGoogle, FaSpinner } from "react-icons/fa";

import { sanitizeNextPath, withNext } from "@/domains/auth/client";
import { useEmailSignUp, useSession, useSocialSignIn } from "@/domains/auth/hooks";

export default function SignupPage() {
  const router = useRouter();
  const searchParams = useSearchParams();
  const nextPath = sanitizeNextPath(searchParams.get("next"), "/onboarding");
  const session = useSession();
  const signUp = useEmailSignUp(nextPath);
  const google = useSocialSignIn("google", nextPath);
  const github = useSocialSignIn("github", nextPath);
  const isMutating = signUp.isPending || google.isPending || github.isPending;

  useEffect(() => {
    if (session.isSuccess && session.data) {
      router.replace(nextPath);
    }
  }, [session.isSuccess, session.data, router, nextPath]);

  return (
    <main className="relative flex min-h-full flex-1 flex-col items-center justify-center gap-8">
      <div className="absolute top-6 left-6 text-xl font-medium text-white/80">
        Anyscape Platform
      </div>
      {session.isPending ? (
        <FaSpinner className="size-5 animate-spin text-white/60" />
      ) : (
        <>
          <div className="flex flex-col items-center gap-2 text-center">
            <h1 className="text-3xl font-medium">
              Build on the Anyscape
              <br />
              Platform
            </h1>
            <p className="text-base text-white/60">Sign up to build with Argus Agent.</p>
          </div>
          {(google.isError || github.isError) && (
            <p className="w-96 text-center text-xs leading-relaxed text-red-400">
              {(google.error ?? github.error)?.message}
            </p>
          )}
          <div className="flex w-96 flex-col gap-3">
            <button
              type="button"
              disabled={isMutating}
              onClick={() => google.mutate()}
              className="flex w-full cursor-pointer items-center justify-center gap-3 rounded-full border border-border px-4 py-3 text-sm font-medium transition-colors hover:bg-white/5 disabled:cursor-not-allowed disabled:opacity-70"
            >
              {google.isPending ? (
                <FaSpinner className="size-4 animate-spin" />
              ) : (
                <FaGoogle className="size-4" />
              )}
              {google.isPending ? "Redirecting..." : "Continue with Google"}
            </button>
            <button
              type="button"
              disabled={isMutating}
              onClick={() => github.mutate()}
              className="flex w-full cursor-pointer items-center justify-center gap-3 rounded-full border border-border px-4 py-3 text-sm font-medium transition-colors hover:bg-white/5 disabled:cursor-not-allowed disabled:opacity-70"
            >
              {github.isPending ? (
                <FaSpinner className="size-4 animate-spin" />
              ) : (
                <FaGithub className="size-4" />
              )}
              {github.isPending ? "Redirecting..." : "Continue with GitHub"}
            </button>
          </div>
          <div className="flex w-96 items-center gap-3">
            <div className="h-px flex-1 bg-border" />
            <span className="text-xs text-white/50">or</span>
            <div className="h-px flex-1 bg-border" />
          </div>
          <form
            className="flex w-96 flex-col gap-3"
            autoComplete="off"
            onSubmit={(e) => {
              e.preventDefault();
              const data = new FormData(e.currentTarget);
              const email = String(data.get("email") ?? "");
              const name = email.split("@")[0] || "Argus user";
              signUp.mutate({ email, name });
            }}
          >
            <label className="flex flex-col gap-1.5 text-xs font-medium text-white/70">
              Email
              <input
                type="email"
                name="email"
                placeholder="Email address"
                autoComplete="off"
                className="rounded-full border border-border bg-transparent px-4 py-3 text-sm text-white placeholder:text-white/30 outline-none focus:border-white/40"
              />
            </label>
            <button
              type="submit"
              disabled={isMutating}
              className="mt-1 flex w-full cursor-pointer items-center justify-center gap-2 rounded-full bg-accent px-4 py-3 text-sm font-medium text-black transition-colors hover:bg-white/90 disabled:cursor-not-allowed disabled:opacity-70"
            >
              {signUp.isPending && <FaSpinner className="size-4 animate-spin" />}
              {signUp.isPending ? "Sending link..." : "Sign up"}
            </button>
          </form>
          <p className="w-96 text-center text-sm text-white/70">
            Already have an account?{" "}
            <a href={withNext("/auth/login", nextPath)} className="font-medium text-white underline hover:text-white/80">
              Sign in
            </a>
          </p>
          <p className="w-96 text-center text-xs leading-relaxed text-white/50">
            By signing up you agree to our{" "}
            <a href="https://argus-agent.anyscapelabs.com/terms-of-service" target="_blank" rel="noopener noreferrer" className="underline hover:text-white/80">
              Terms
            </a>{" "}
            and{" "}
            <a href="https://argus-agent.anyscapelabs.com/privacy-policy" target="_blank" rel="noopener noreferrer" className="underline hover:text-white/80">
              Privacy
            </a>{" "}
            and to receive updates and emails from Anyscape.
          </p>
        </>
      )}
    </main>
  );
}
