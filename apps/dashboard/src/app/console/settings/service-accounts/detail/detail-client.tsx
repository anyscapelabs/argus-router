"use client";

import { useSearchParams } from "next/navigation";
import ServiceAccountDetail from "./service-account-detail";

export default function Detail() {
  const searchParams = useSearchParams();
  const id = searchParams.get("id");
  if (!id) {
    return <div className="p-4 text-sm text-white/60">Missing service account id.</div>;
  }
  return <ServiceAccountDetail id={id} />;
}
