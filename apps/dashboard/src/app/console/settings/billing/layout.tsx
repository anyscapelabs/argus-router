import type { Metadata } from "next";

export const metadata: Metadata = {
  title: "Billing & Plans | Anyscape Gateway",
  description: "Manage your subscription and plan.",
};

export default function Layout({ children }: { children: React.ReactNode }) {
  return children;
}
