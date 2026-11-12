import type { Metadata } from "next";

export const metadata: Metadata = {
  title: "Logs | Anyscape Gateway",
  description: "Recent gateway request logs.",
};

export const instant = false;

export default function Layout({ children }: { children: React.ReactNode }) {
  return children;
}
