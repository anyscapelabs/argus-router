import type { Metadata } from "next";

export const metadata: Metadata = {
  title: "API Keys | Anyscape Gateway",
  description: "Manage API keys.",
};

export default function Layout({ children }: { children: React.ReactNode }) {
  return children;
}
