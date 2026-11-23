import type { Metadata } from "next";

export const metadata: Metadata = {
  title: "My Account | Anyscape Gateway",
  description: "Personal account settings.",
};

export default function Layout({ children }: { children: React.ReactNode }) {
  return children;
}
