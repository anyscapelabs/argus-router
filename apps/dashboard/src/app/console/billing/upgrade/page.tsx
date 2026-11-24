import { redirect } from "next/navigation";

export default function BillingUpgradePage() {
  redirect("/console/billing?upgrade=1");
}
