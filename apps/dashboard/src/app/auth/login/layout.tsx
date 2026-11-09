import type { Metadata } from "next";

export const metadata: Metadata = {
  title: "Login | Argus Router",
  description: "Sign in to Argus Router.",
};

export const instant = false;

export default function Layout({ children }: { children: React.ReactNode }) {
  return children;
}

