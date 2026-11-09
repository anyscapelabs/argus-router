import type { Metadata } from "next";

export const metadata: Metadata = {
  title: "Billing & Plans | Argus Router",
  description: "Choose a plan and manage billing.",
};

export const instant = false;

export default function Layout({ children }: { children: React.ReactNode }) {
  return children;
}

