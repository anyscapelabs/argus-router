"use client";

import { useRouter, useSearchParams } from "next/navigation";
import { Suspense, useEffect, useRef } from "react";
import { FaCheckCircle, FaSpinner } from "react-icons/fa";

import { useResendVerification, useVerifyEmail } from "@/domains/auth/hooks";

function VerifyEmail() {
  const router = useRouter();
  const token = useSearchParams().get("token");
  const verify = useVerifyEmail();
  const resend = useResendVerification();
  const attempted = useRef(false);

  useEffect(() => {
    if (token && !attempted.current) {
      attempted.current = true;
      verify.mutate(token);
    }
  }, [token, verify]);

  if (token) {
    return (
      <main className="flex min-h-full flex-1 flex-col items-center justify-center gap-6">
        <h1 className="font-pixel text-5xl">Anyscape Gateway</h1>
        {verify.isPending && (
          <p className="flex w-80 items-center justify-center gap-2 text-sm text-white/70">
            <FaSpinner className="size-4 animate-spin" /> Verifying your email...
          </p>
        )}
        {verify.isSuccess && (
          <>
            <p className="flex w-80 items-center justify-center gap-2 text-sm text-white/70">
              <FaCheckCircle className="size-4" /> Your email is verified.
            </p>
            <button
              type="button"
              onClick={() => router.push("/auth/login")}
              className="flex w-80 cursor-pointer items-center justify-center gap-2 bg-accent px-3 py-2 text-sm font-medium text-black transition-colors hover:bg-white/90"
            >
              Sign in
            </button>
          </>
        )}
        {verify.isError && (
          <div className="flex w-80 flex-col items-center gap-4">
            <a
              href="/auth/signup"
              className="text-sm text-white underline hover:text-white/80"
            >
              Back to sign up
            </a>
          </div>
        )}
      </main>
    );
  }

  return (
    <main className="flex min-h-full flex-1 flex-col items-center justify-center gap-6">
      <h1 className="font-pixel text-5xl">Anyscape Gateway</h1>
      <p className="w-80 text-center text-sm leading-relaxed text-white/70">
        Check your inbox for a verification email and click the link, or resend it
        below.
      </p>
        <form
          className="flex w-80 flex-col gap-3"
          autoComplete="off"
          onSubmit={(e) => {
            e.preventDefault();
            const email = String(new FormData(e.currentTarget).get("email") ?? "");
            resend.mutate(email);
          }}
        >
          <label className="flex flex-col gap-1.5 text-xs font-medium text-white/70">
            Email
            <input
              type="email"
              name="email"
              placeholder="you@example.com"
              required
              className="border border-border bg-transparent px-3 py-2 text-sm text-white placeholder:text-white/30 outline-none focus:border-white/40"
            />
          </label>
          <button
            type="submit"
            disabled={resend.isPending}
            className="flex w-full cursor-pointer items-center justify-center gap-2 bg-accent px-3 py-2 text-sm font-medium text-black transition-colors hover:bg-white/90 disabled:cursor-not-allowed disabled:opacity-70"
          >
            {resend.isPending && <FaSpinner className="size-4 animate-spin" />}
            {resend.isPending ? "Sending..." : "Resend verification email"}
          </button>
        </form>
      <p className="w-80 text-center text-sm text-white/70">
        Wrong email?{" "}
        <a href="/auth/signup" className="font-medium text-white underline hover:text-white/80">
          Sign up again
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
        and to receive updates and emails from Anyscape.
      </p>
    </main>
  );
}

export default function VerifyEmailPage() {
  return (
    <Suspense>
      <VerifyEmail />
    </Suspense>
  );
}
