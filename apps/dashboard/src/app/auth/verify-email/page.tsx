"use client";

import Image from "next/image";
import { useRouter, useSearchParams } from "next/navigation";
import { Suspense, useEffect, useRef } from "react";
import { FaCheckCircle, FaSpinner } from "react-icons/fa";

import { useResendVerification, useVerifyEmail } from "@/domains/auth/hooks";

function VerifyEmail() {
  const router = useRouter();
  const params = useSearchParams();
  const token = params.get("token");
  const email = params.get("email");
  const verify = useVerifyEmail();
  const resend = useResendVerification();
  const attempted = useRef(false);

  useEffect(() => {
    if (token && !attempted.current) {
      attempted.current = true;
      verify.mutate(token);
    }
  }, [token, verify]);

  useEffect(() => {
    if (verify.isSuccess) {
      router.replace("/onboarding");
    }
  }, [verify.isSuccess, router]);

  if (token) {
    return (
      <main className="flex min-h-full flex-1 flex-col items-center justify-center gap-6">
        <div className="flex flex-col items-center gap-3">
          <Image src="/icon-white.svg" alt="Anyscape Gateway" width={40} height={32} priority />
          <h1 className="text-xl font-medium">Verify your email</h1>
        </div>
        {verify.isPending && (
          <p className="flex w-80 items-center justify-center gap-2 text-sm text-white/70">
            <FaSpinner className="size-4 animate-spin" /> Verifying your email...
          </p>
        )}
        {verify.isSuccess && (
          <p className="flex w-80 items-center justify-center gap-2 text-sm text-white/70">
            <FaCheckCircle className="size-4" /> Your email is verified. Redirecting...
          </p>
        )}
        {verify.isError && (
          <div className="flex w-80 flex-col items-center gap-4">
            <p className="text-center text-xs leading-relaxed text-red-400">
              {verify.error?.message ?? "This verification link is invalid or has expired."}
            </p>
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
      <div className="flex flex-col items-center gap-3">
        <Image src="/icon-white.svg" alt="Anyscape Gateway" width={40} height={32} priority />
        <h1 className="text-xl font-medium">Check your inbox</h1>
      </div>
      <p className="w-80 text-center text-sm leading-relaxed text-white/70">
        {email ? (
          <>
            We sent a verification email to{" "}
            <span className="font-medium text-white">{email}</span>. Click the link
            in the email to activate your account.
          </>
        ) : (
          "Check your inbox for a verification email and click the link to activate your account."
        )}
      </p>
      {email && (
        <button
          type="button"
          disabled={resend.isPending}
          onClick={() => resend.mutate(email)}
          className="cursor-pointer text-sm text-white/70 underline hover:text-white disabled:cursor-not-allowed disabled:opacity-70"
        >
          {resend.isPending ? "Sending..." : "Resend verification email"}
        </button>
      )}
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