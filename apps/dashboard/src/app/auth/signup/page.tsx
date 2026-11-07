"use client";

import { useRouter } from "next/navigation";
import { useEffect, useState } from "react";
import { FaEye, FaEyeSlash, FaGithub, FaGoogle, FaSpinner } from "react-icons/fa";

import { useEmailSignUp, useSession, useSocialSignIn } from "@/domains/auth/hooks";

export default function SignupPage() {
  const [showPassword, setShowPassword] = useState(false);
  const router = useRouter();
  const session = useSession();
  const signUp = useEmailSignUp();
  const google = useSocialSignIn("google");
  const github = useSocialSignIn("github");
  const isMutating = signUp.isPending || google.isPending || github.isPending;

  // Already signed in — skip the signup form and land on the console.
  useEffect(() => {
    if (session.isSuccess && session.data) {
      router.replace("/console/overview");
    }
  }, [session.isSuccess, session.data, router]);
  return (
    <main className="flex min-h-full flex-1 flex-col items-center justify-center gap-8">
      {session.isPending ? (
        <FaSpinner className="size-5 animate-spin text-white/60" />
      ) : (
        <>
      <h1 className="font-pixel text-5xl">Argus Agent</h1>
      {(google.isError || github.isError) && (
        <p className="w-80 text-center text-xs leading-relaxed text-red-400">
          {(google.error ?? github.error)?.message}
        </p>
      )}
      <div className="flex w-80 flex-col gap-3">
        <button
          type="button"
          disabled={isMutating}
          onClick={() => google.mutate()}
          className="flex w-full items-center justify-center gap-3 cursor-pointer border border-border px-3 py-2 text-sm font-medium transition-colors hover:bg-white/5 disabled:cursor-not-allowed disabled:opacity-70"
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
          className="flex w-full items-center justify-center gap-3 cursor-pointer border border-border px-3 py-2 text-sm font-medium transition-colors hover:bg-white/5 disabled:cursor-not-allowed disabled:opacity-70"
        >
          {github.isPending ? (
            <FaSpinner className="size-4 animate-spin" />
          ) : (
            <FaGithub className="size-4" />
          )}
          {github.isPending ? "Redirecting..." : "Continue with GitHub"}
        </button>
      </div>
      <div className="flex w-80 items-center gap-3">
        <div className="h-px flex-1 bg-border" />
        <span className="text-xs text-white/50">or</span>
        <div className="h-px flex-1 bg-border" />
      </div>
      <form
        className="flex w-80 flex-col gap-3"
        autoComplete="off"
        onSubmit={(e) => {
          e.preventDefault();
          const data = new FormData(e.currentTarget);
          const email = String(data.get("email") ?? "");
          const password = String(data.get("password") ?? "");
          const name = email.split("@")[0] || "Argus user";
          signUp.mutate({ email, password, name });
        }}
      >
        <label className="flex flex-col gap-1.5 text-xs font-medium text-white/70">
          Email
          <input
            type="email"
            name="email"
            placeholder="you@example.com"
            autoComplete="off"
            className="border border-border bg-transparent px-3 py-2 text-sm text-white placeholder:text-white/30 outline-none focus:border-white/40"
          />
        </label>
        <label className="flex flex-col gap-1.5 text-xs font-medium text-white/70">
          Password
          <div className="flex items-center border border-border focus-within:border-white/40">
            <input
              type={showPassword ? "text" : "password"}
              name="password"
              placeholder="••••••••"
              autoComplete="off"
              className="w-full bg-transparent px-3 py-2 text-sm text-white placeholder:text-white/30 outline-none"
            />
            <button
              type="button"
              onClick={() => setShowPassword((v) => !v)}
              className="cursor-pointer px-3 py-2 text-white/50 hover:text-white/80"
              aria-label={showPassword ? "Hide password" : "Show password"}
            >
              {showPassword ? <FaEyeSlash className="size-4" /> : <FaEye className="size-4" />}
            </button>
          </div>
        </label>
        <button
          type="submit"
          disabled={isMutating}
          className="mt-1 flex w-full cursor-pointer items-center justify-center gap-2 bg-accent px-3 py-2 text-sm font-medium text-black transition-colors hover:bg-white/90 disabled:cursor-not-allowed disabled:opacity-70"
        >
          {signUp.isPending && <FaSpinner className="size-4 animate-spin" />}
          {signUp.isPending ? "Creating account..." : "Sign up"}
        </button>
      </form>
      <p className="w-80 text-center text-sm text-white/70">
        Already have an account?{" "}
        <a href="/auth/login" className="font-medium text-white underline hover:text-white/80">
          Sign in
        </a>
      </p>
      <p className="w-80 text-center text-xs leading-relaxed text-white/50">
        By signing up you agree to our{" "}
        <a href="/terms" className="underline hover:text-white/80">
          Terms
        </a>{" "}
        and{" "}
        <a href="/privacy" className="underline hover:text-white/80">
          Privacy
        </a>{" "}
        and to receive updates and emails from Argus Agent.
      </p>
        </>
      )}
    </main>
  );
}
