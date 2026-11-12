import type { Metadata } from "next";

export const metadata: Metadata = {
  title: "Usage | Anyscape Gateway",
  description: "Token usage, cost, and model breakdowns.",
};

export const instant = false;

export default function Layout({ children }: { children: React.ReactNode }) {
  return children;
}
