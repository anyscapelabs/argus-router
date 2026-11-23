"use client";

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

  return (
    <main className="relative flex min-h-full flex-1 flex-col items-center justify-center gap-8">
      <div className="absolute top-6 left-6 text-xl font-medium text-white/80">
        Anyscape Platform
      </div>

      {!token && (
        <>
          <div className="flex flex-col items-center gap-2 text-center">
            <h1 className="text-3xl font-medium">
              Check your
              <br />
              inbox
            </h1>
            <p className="text-base text-white/60">Verify your email to continue.</p>
          </div>
          <p className="w-96 text-center text-sm leading-relaxed text-white/70">
            {email ? (
              <>
                We sent a verification email to{" "}
                <span className="font-medium text-white">{email}</span>. Click the link in the
                email to activate your account.
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
              className="flex w-96 cursor-pointer items-center justify-center gap-2 rounded-full border border-border px-4 py-3 text-sm font-medium text-white/80 transition-colors hover:bg-white/5 disabled:cursor-not-allowed disabled:opacity-70"
            >
              {resend.isPending && <FaSpinner className="size-4 animate-spin" />}
              {resend.isPending ? "Sending..." : "Resend verification email"}
            </button>
          )}
        </>
      )}

      {token && verify.isPending && (
        <>
          <div className="flex flex-col items-center gap-2 text-center">
            <h1 className="text-3xl font-medium">
              Verifying
              <br />
              your email
            </h1>
            <p className="text-base text-white/60">One moment.</p>
          </div>
          <p className="flex w-96 items-center justify-center gap-2 text-sm text-white/70">
            <FaSpinner className="size-4 animate-spin" /> Verifying your email...
          </p>
        </>
      )}

      {token && verify.isSuccess && (
        <>
          <div className="flex flex-col items-center gap-2 text-center">
            <h1 className="text-3xl font-medium">
              Email
              <br />
              verified
            </h1>
            <p className="text-base text-white/60">You are ready to continue.</p>
          </div>
          <p className="flex w-96 items-center justify-center gap-2 text-sm text-white/70">
            <FaCheckCircle className="size-4" /> Your email is verified.
          </p>
        </>
      )}

      {token && verify.isError && (
        <>
          <div className="flex flex-col items-center gap-2 text-center">
            <h1 className="text-3xl font-medium">
              Verification
              <br />
              failed
            </h1>
            <p className="text-base text-white/60">The link may have expired.</p>
          </div>
          <p className="w-96 text-center text-xs leading-relaxed text-red-400">
            {verify.error?.message ?? "This verification link is invalid or has expired."}
          </p>
        </>
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

export default function VerifyEmailPage() {
  return (
    <Suspense>
      <VerifyEmail />
    </Suspense>
  );
}
