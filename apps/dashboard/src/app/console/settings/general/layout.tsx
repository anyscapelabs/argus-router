import type { Metadata } from "next";

export const metadata: Metadata = {
  title: "General | Anyscape Gateway",
  description: "Workspace name and avatar settings.",
};

export const instant = false;

export default function Layout({ children }: { children: React.ReactNode }) {
  return children;
}
