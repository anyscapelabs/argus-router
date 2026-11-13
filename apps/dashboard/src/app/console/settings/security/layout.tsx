import type { Metadata } from "next";

export const metadata: Metadata = {
  title: "Security | Anyscape Gateway",
  description: "Security settings.",
};

export default function Layout({ children }: { children: React.ReactNode }) {
  return children;
}
