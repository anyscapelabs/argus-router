import type { Metadata } from "next";

export const metadata: Metadata = {
  title: "Onboarding | Anyscape Gateway",
  description: "Finish setting up your Anyscape Gateway account.",
};

export const instant = false;

export default function Layout({ children }: { children: React.ReactNode }) {
  return children;
}
