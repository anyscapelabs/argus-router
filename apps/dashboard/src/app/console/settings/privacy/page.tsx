"use client";

import { useSession } from "@/domains/auth/hooks";
import { useSettings, useUpdateSettings } from "@/domains/settings/hooks";

export default function PrivacySettingsPage() {
  const session = useSession();
  const settings = useSettings({ enabled: Boolean(session.data) });
  const update = useUpdateSettings();

  const allowTraining = settings.data?.allowFreeTraining ?? true;

  const toggle = () => {
    update.mutate({ allowFreeTraining: !allowTraining });
  };

  return (
    <div className="px-2 py-1">
      <h1 className="text-base font-medium text-primary">Privacy</h1>
      <p className="mt-1 text-sm text-white/60">
        Manage your privacy and data preferences.
      </p>

      <h2 className="mt-6 text-sm font-medium text-primary">Training</h2>
      <div className="mt-2 flex items-center justify-between gap-4 rounded-md border border-border-secondary bg-[#262626] p-3">
        <div>
          <p className="text-sm font-medium text-primary">
            Allow free endpoints that train on request data
          </p>
          <p className="mt-1 text-xs text-white/60">
            Free models may train on your requests. Turn off to disable all free models.
          </p>
        </div>
        <button
          type="button"
          role="switch"
          aria-checked={allowTraining}
          aria-label="Allow free endpoints that train on request data"
          disabled={update.isPending}
          onClick={toggle}
          className={`relative inline-flex h-5 w-9 shrink-0 cursor-pointer rounded-full transition-colors disabled:cursor-wait ${
            allowTraining ? "bg-[#3b82f6]" : "bg-white/20"
          }`}
        >
          <span
            className={`absolute top-0.5 left-0.5 block size-4 rounded-full bg-white transition-transform ${
              allowTraining ? "translate-x-4" : ""
            }`}
          />
        </button>
      </div>
    </div>
  );
}
