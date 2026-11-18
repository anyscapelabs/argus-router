"use client";

import Image from "next/image";
import Link from "next/link";
import { useSearchParams } from "next/navigation";
import { Suspense, useState } from "react";
import { FaCheckCircle, FaSpinner } from "react-icons/fa";

import { useConfirmAccountDeletion } from "@/domains/auth/hooks";

function ConfirmDeletion() {
  const params = useSearchParams();
  const token = params.get("token");
  const deletion = useConfirmAccountDeletion();
  const [confirmed, setConfirmed] = useState(false);

  if (!token) {
    return (
      <main className="flex min-h-full flex-1 flex-col items-center justify-center gap-6">
        <div className="flex flex-col items-center gap-3">
          <Image src="/icon-white.svg" alt="Anyscape Gateway" width={40} height={32} priority />
          <h1 className="text-xl font-medium">Invalid deletion link</h1>
        </div>
        <p className="w-80 text-center text-sm leading-relaxed text-white/70">
          This link is missing its confirmation token. Use the link from the
          deletion confirmation email.
        </p>
        <Link href="/" className="text-sm text-white/70 underline hover:text-white">
          Back to home
        </Link>
      </main>
    );
  }

  if (confirmed && deletion.isSuccess) {
    return (
      <main className="flex min-h-full flex-1 flex-col items-center justify-center gap-6">
        <div className="flex flex-col items-center gap-3">
          <Image src="/icon-white.svg" alt="Anyscape Gateway" width={40} height={32} priority />
          <h1 className="text-xl font-medium">Your account has been deleted</h1>
        </div>
        <p className="w-80 text-center text-sm leading-relaxed text-white/70">
          Your account and all associated data have been permanently removed.
        </p>
        <Link href="/auth/login" className="text-sm text-white/70 underline hover:text-white">
          Back to sign in
        </Link>
      </main>
    );
  }

  return (
    <main className="flex min-h-full flex-1 flex-col items-center justify-center gap-6">
      <div className="flex flex-col items-center gap-3">
        <Image src="/icon-white.svg" alt="Anyscape Gateway" width={40} height={32} priority />
        <h1 className="text-xl font-medium">Delete your account?</h1>
      </div>
      <p className="w-80 text-center text-sm leading-relaxed text-white/70">
        This permanently deletes your account and all your data. This decision
        can&apos;t be undone.
      </p>
      <div className="flex flex-col items-center gap-3">
        <button
          type="button"
          onClick={() => {
            setConfirmed(true);
            deletion.mutate(token);
          }}
          disabled={deletion.isPending}
          className="cursor-pointer rounded-sm bg-red-500/90 px-3 py-1.5 text-xs font-medium text-white transition-colors hover:bg-red-500 disabled:cursor-not-allowed disabled:opacity-50"
        >
          Permanently delete my account
        </button>
        <Link href="/console/overview" className="text-sm text-white/70 underline hover:text-white">
          Keep my account
        </Link>
      </div>
      {deletion.isPending && confirmed && (
        <p className="flex w-80 items-center justify-center gap-2 text-sm text-white/70">
          <FaSpinner className="size-4 animate-spin" /> Deleting your account...
        </p>
      )}
      {deletion.isError && confirmed && (
        <div className="flex w-80 flex-col items-center gap-4">
          <p className="text-center text-xs leading-relaxed text-red-400">
            {deletion.error?.message ??
              "This link is invalid, expired, or you're signed out. Sign in and try again."}
          </p>
          <Link href="/auth/login" className="text-sm text-white underline hover:text-white/80">
            Sign in
          </Link>
        </div>
      )}
    </main>
  );
}

export default function DeleteAccountPage() {
  return (
    <Suspense>
      <ConfirmDeletion />
    </Suspense>
  );
}
