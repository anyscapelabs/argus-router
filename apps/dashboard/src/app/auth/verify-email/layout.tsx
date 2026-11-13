import type { Metadata } from "next";

export const metadata: Metadata = {
  title: "Verify Email | Anyscape Gateway",
  description: "Verify your email address.",
};

export default function Layout({ children }: { children: React.ReactNode }) {
  return children;
}
