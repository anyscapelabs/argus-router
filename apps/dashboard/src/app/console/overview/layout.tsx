import type { Metadata } from "next";

export const metadata: Metadata = {
  title: "Overview | Argus Router",
  description: "Usage analytics, activity, and model performance.",
};

export const instant = false;

export default function Layout({ children }: { children: React.ReactNode }) {
  return children;
}

