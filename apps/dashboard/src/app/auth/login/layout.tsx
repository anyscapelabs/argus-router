import type { Metadata } from "next";

export const metadata: Metadata = {
  title: "Login | Anyscape Gateway",
  description: "Sign in to Anyscape Gateway.",
};

export const instant = false;

export default function Layout({ children }: { children: React.ReactNode }) {
  return children;
}
