"use client";

import { SelectDropdown } from "@/components/select-dropdown";
import { useSession } from "@/domains/auth/hooks";
import { useSettings, useUpdateSettings } from "@/domains/settings/hooks";

const LIFETIME_OPTIONS = [
  { label: "No limit", value: "0" },
  { label: "7 days", value: "7" },
  { label: "30 days", value: "30" },
  { label: "60 days", value: "60" },
  { label: "90 days", value: "90" },
  { label: "180 days", value: "180" },
  { label: "1 year", value: "365" },
] as const;

export default function SecuritySettingsPage() {
  const session = useSession();
  const settings = useSettings({ enabled: Boolean(session.data) });
  const update = useUpdateSettings();

  const maxLifetime = String(settings.data?.maxKeyLifetimeDays ?? 0);

  const change = (value: string) => {
    update.mutate({ maxKeyLifetimeDays: Number(value) });
  };

  return (
    <div className="px-2 py-1">
      <h1 className="text-base font-medium text-primary">Security</h1>
      <p className="mt-1 text-sm text-white/60">
        Manage your account security and authentication settings.
      </p>

      <h2 className="mt-6 text-sm font-medium text-primary">Service Account Keys</h2>
      <div className="mt-2 flex items-center justify-between gap-4 rounded-md border border-border-secondary bg-[#262626] p-3">
        <div>
          <p className="text-sm font-medium text-primary">Maximum Key Lifetime</p>
          <p className="mt-1 text-xs text-white/60">
            New API keys expire within this time. Existing keys keep their expiry.
          </p>
        </div>
        <div className="w-44 shrink-0">
          <SelectDropdown
            value={maxLifetime}
            options={LIFETIME_OPTIONS}
            onChange={change}
            className="relative"
          />
        </div>
      </div>
    </div>
  );
}
