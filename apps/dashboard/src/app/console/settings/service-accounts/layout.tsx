import type { Metadata } from "next";

export const metadata: Metadata = {
  title: "Service Accounts | Anyscape Gateway",
  description: "Manage service accounts and API keys.",
};

export default function Layout({ children }: { children: React.ReactNode }) {
  return children;
}
