import type { Metadata } from "next";

export const metadata: Metadata = {
  title: "Privacy | Argus Router",
  description: "Privacy settings.",
};

export const instant = false;

export default function Layout({ children }: { children: React.ReactNode }) {
  return children;
}

