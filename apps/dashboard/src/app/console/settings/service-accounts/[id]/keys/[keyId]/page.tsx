"use client";

import { useParams } from "next/navigation";
import Link from "next/link";

const requests = [
  { id: "req_1", model: "GPT-5", status: 200, tokens: 1200, date: "2026-11-01" },
  { id: "req_2", model: "Claude 4", status: 200, tokens: 890, date: "2026-11-02" },
  { id: "req_3", model: "GPT-5", status: 429, tokens: 0, date: "2026-11-03" },
  { id: "req_4", model: "Llama 3", status: 200, tokens: 450, date: "2026-11-04" },
];

const modelCounts = [
  { model: "GPT-5", count: 2 },
  { model: "Claude 4", count: 1 },
  { model: "Llama 3", count: 1 },
];

export default function ApiKeyDetailPage() {
  const params = useParams<{ id: string; keyId: string }>();
  const id = params?.id ?? "";
  const keyId = params?.keyId ?? "";

  return (
    <div className="px-2 py-1">
      <div className="mb-4 flex items-center gap-2 text-xs text-white/60">
        <Link href="/console/settings/service-accounts" className="hover:text-primary">
          Service Accounts
        </Link>
        <span>/</span>
        <Link href={`/console/settings/service-accounts/${id}`} className="hover:text-primary">
          {id}
        </Link>
        <span>/</span>
        <span className="text-primary">{keyId}</span>
      </div>

      <h1 className="text-base font-medium text-primary">API Key Logs</h1>
      <p className="mt-1 text-sm text-white/60">
        Logs for this API key.
      </p>

      <div className="mt-6">
        <h2 className="text-sm font-medium text-primary">Models per request</h2>
        <div className="mt-3 flex items-end gap-2">
          {modelCounts.map((m) => (
            <div key={m.model} className="flex flex-col items-center gap-1">
              <div
                className="w-8 rounded-t-sm bg-accent"
                style={{ height: `${m.count * 24}px` }}
              />
              <span className="text-[10px] text-white/60">{m.model}</span>
            </div>
          ))}
        </div>
      </div>

      <div className="mt-6">
        <h2 className="text-sm font-medium text-primary">Requests</h2>
        <div className="mt-3 overflow-x-auto rounded-md border border-border-secondary">
          <table className="w-full text-left text-xs">
            <thead>
              <tr className="border-b border-border-secondary bg-tertiary text-white/40">
                <th className="px-2 py-2 font-medium rounded-tl-md">Request</th>
                <th className="px-2 py-2 font-medium">Model</th>
                <th className="px-2 py-2 font-medium">Status</th>
                <th className="px-2 py-2 font-medium">Tokens</th>
                <th className="px-2 py-2 font-medium rounded-tr-md">Date</th>
              </tr>
            </thead>
            <tbody>
              {requests.map((r) => (
                <tr key={r.id} className="border-b border-border-secondary/50 transition-colors hover:bg-[#202020]">
                  <td className="px-2 py-2 text-primary">{r.id}</td>
                  <td className="px-2 py-2 text-white/70">{r.model}</td>
                  <td className="px-2 py-2 text-white/70">{r.status}</td>
                  <td className="px-2 py-2 text-white/70">{r.tokens}</td>
                  <td className="px-2 py-2 text-white/70">{r.date}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </div>
    </div>
  );
}