import type { Metadata } from "next";

export const metadata: Metadata = {
  title: "Verify Email | Argus Router",
  description: "Verify your email address.",
};

export const instant = false;

export default function Layout({ children }: { children: React.ReactNode }) {
  return children;
}

