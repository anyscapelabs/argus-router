"use client";

import { Skeleton } from "@/components/skeleton";

const FIRST_WIDTHS = ["w-44", "w-36", "w-40", "w-32", "w-40"];
const COL_WIDTHS = [
  "",
  "w-16",
  "w-28",
  "w-24",
  "w-14",
  "w-16",
  "w-14",
  "w-20",
];

export function SkeletonRows({ columns }: { columns: number }) {
  return (
    <>
      {FIRST_WIDTHS.map((first, row) => (
        <tr key={row} className="border-b border-border-secondary/50 last:border-b-0">
          {Array.from({ length: columns }, (_, col) => (
            <td key={col} className="whitespace-nowrap px-2 py-2.5">
              <Skeleton
                className={`h-3 ${
                  col === 0 ? first : (COL_WIDTHS[col] ?? "w-20")
                } ${col === columns - 1 ? "ml-auto" : ""}`}
              />
            </td>
          ))}
        </tr>
      ))}
    </>
  );
}