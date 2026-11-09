import type { Metadata } from "next";

export const metadata: Metadata = {
  title: "API Key Logs | Argus Router",
  description: "API key usage logs and details.",
};

export const instant = false;

export default function Layout({ children }: { children: React.ReactNode }) {
  return children;
}

