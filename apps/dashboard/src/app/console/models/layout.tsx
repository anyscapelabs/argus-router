import type { Metadata } from "next";

export const metadata: Metadata = {
  title: "Models | Argus Router",
  description: "Manage available models and their access.",
};

export const instant = false;

export default function Layout({ children }: { children: React.ReactNode }) {
  return children;
}

