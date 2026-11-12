import type { Metadata } from "next";

export const metadata: Metadata = {
  title: "Account | Anyscape Gateway",
  description: "Personal account settings.",
};

export const instant = false;

export default function Layout({ children }: { children: React.ReactNode }) {
  return children;
}
