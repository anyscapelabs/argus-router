import type { Metadata } from "next";

export const metadata: Metadata = {
  title: "Sign Up | Argus Router",
  description: "Create an Argus Router account.",
};

export const instant = false;

export default function Layout({ children }: { children: React.ReactNode }) {
  return children;
}

