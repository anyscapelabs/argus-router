import type { Metadata } from "next";

export const metadata: Metadata = {
  title: "Models | Anyscape Gateway",
  description: "Manage available models and their access.",
};

export default function Layout({ children }: { children: React.ReactNode }) {
  return children;
}
