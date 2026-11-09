import type { Metadata } from "next";

export const metadata: Metadata = {
  title: "Subscription | Argus Router",
  description: "Manage your plan and billing details.",
};

export const instant = false;

export default function Layout({ children }: { children: React.ReactNode }) {
  return children;
}

