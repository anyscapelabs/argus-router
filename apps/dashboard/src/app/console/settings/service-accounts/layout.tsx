import type { Metadata } from "next";

export const metadata: Metadata = {
  title: "Service Accounts | Argus Router",
  description: "Manage service accounts and API keys.",
};

export const instant = false;

export default function Layout({ children }: { children: React.ReactNode }) {
  return children;
}

