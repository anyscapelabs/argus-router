import type { Metadata } from "next";

export const metadata: Metadata = {
  title: "Sign Up | Anyscape Gateway",
  description: "Create an Anyscape Gateway account.",
};

export const instant = false;

export default function Layout({ children }: { children: React.ReactNode }) {
  return children;
}
