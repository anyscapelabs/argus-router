import { Suspense } from "react";
import ServiceAccountDetail from "./service-account-detail";

export default function ServiceAccountDetailPage({
  params,
}: {
  params: Promise<{ id: string }>;
}) {
  return (
    <Suspense fallback={<div className="flex flex-col gap-2 p-4"><div className="h-3 w-40 animate-pulse rounded-sm bg-white/10" /><div className="h-3 w-64 animate-pulse rounded-sm bg-white/10" /></div>}>
      <ServiceAccountDetailContent params={params} />
    </Suspense>
  );
}

async function ServiceAccountDetailContent({
  params,
}: {
  params: Promise<{ id: string }>;
}) {
  const { id } = await params;
  return <ServiceAccountDetail id={id} />;
}
