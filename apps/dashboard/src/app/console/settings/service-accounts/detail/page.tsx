import { Suspense } from "react";
import Detail from "./detail-client";

export default function ServiceAccountDetailPage() {
  return (
    <Suspense fallback={<div className="flex flex-col gap-2 p-4"><div className="h-3 w-40 animate-pulse rounded-sm bg-white/10" /><div className="h-3 w-64 animate-pulse rounded-sm bg-white/10" /></div>}>
      <Detail />
    </Suspense>
  );
}
