import type { Metadata } from "next";

export const metadata: Metadata = {
  title: "Security | Argus Router",
  description: "Security settings.",
};

export const instant = false;

export default function Layout({ children }: { children: React.ReactNode }) {
  return children;
}

