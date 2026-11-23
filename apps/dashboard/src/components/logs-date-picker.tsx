"use client";

import { useEffect, useRef, useState } from "react";
import { GoChevronLeft, GoChevronRight } from "react-icons/go";
import { IoCalendarNumberOutline } from "react-icons/io5";

export type LogsDateRange = { start: Date; end: Date | null };

const PRESETS = [
  "Today",
  "Last 7 days",
  "Last 30 days",
  "This month",
  "Last month",
] as const;

type Preset = (typeof PRESETS)[number];

const WEEKDAYS = ["Su", "Mo", "Tu", "We", "Th", "Fr", "Sa"];
const MONTH_FORMAT = new Intl.DateTimeFormat("en-GB", { month: "long", year: "numeric" });
const DAY_MONTH_FORMAT = new Intl.DateTimeFormat("en-GB", { day: "numeric", month: "short" });

function sameDay(a: Date, b: Date) {
  return a.getFullYear() === b.getFullYear() && a.getMonth() === b.getMonth() && a.getDate() === b.getDate();
}

function todayDate() {
  const now = new Date();
  return new Date(now.getFullYear(), now.getMonth(), now.getDate());
}

export function getPresetRange(preset: Preset): LogsDateRange {
  const today = todayDate();
  if (preset === "Today") return { start: today, end: today };
  if (preset === "This month") {
    return { start: new Date(today.getFullYear(), today.getMonth(), 1), end: today };
  }
  if (preset === "Last month") {
    return {
      start: new Date(today.getFullYear(), today.getMonth() - 1, 1),
      end: new Date(today.getFullYear(), today.getMonth(), 0),
    };
  }
  const start = new Date(today);
  start.setDate(today.getDate() - (preset === "Last 7 days" ? 6 : 29));
  return { start, end: today };
}

function formatRange(range: LogsDateRange) {
  const end = range.end ?? range.start;
  const year = end.getFullYear();
  if (sameDay(range.start, end)) {
    return `${DAY_MONTH_FORMAT.format(range.start)} ${year}`;
  }
  if (range.start.getFullYear() === year) {
    if (range.start.getMonth() === end.getMonth()) {
      return `${range.start.getDate()} - ${end.getDate()} ${DAY_MONTH_FORMAT.format(end).split(" ")[1]} ${year}`;
    }
    return `${DAY_MONTH_FORMAT.format(range.start)} - ${DAY_MONTH_FORMAT.format(end)} ${year}`;
  }
  return `${DAY_MONTH_FORMAT.format(range.start)} ${range.start.getFullYear()} - ${DAY_MONTH_FORMAT.format(end)} ${year}`;
}

function MonthCalendar({
  month,
  range,
  onPick,
}: {
  month: Date;
  range: LogsDateRange;
  onPick: (date: Date) => void;
}) {
  const year = month.getFullYear();
  const monthIndex = month.getMonth();
  const leading = new Date(year, monthIndex, 1).getDay();
  const days = new Date(year, monthIndex + 1, 0).getDate();
  const cells: (number | null)[] = [
    ...Array.from({ length: leading }, () => null),
    ...Array.from({ length: days }, (_, index) => index + 1),
  ];

  return (
    <div className="w-52">
      <p className="mb-2 text-center text-xs font-medium text-primary">
        {MONTH_FORMAT.format(month)}
      </p>
      <div className="grid grid-cols-7 text-center text-[10px] text-primary/40">
        {WEEKDAYS.map((day) => (
          <span key={day}>{day}</span>
        ))}
      </div>
      <div className="mt-1 grid grid-cols-7 gap-y-1 text-center text-xs">
        {cells.map((day, index) => {
          if (day === null) return <span key={`empty-${index}`} />;
          const date = new Date(year, monthIndex, day);
          const isStart = sameDay(date, range.start);
          const isEnd = range.end ? sameDay(date, range.end) : false;
          const inRange = range.end ? date > range.start && date < range.end : false;
          return (
            <button
              key={day}
              type="button"
              onClick={() => onPick(date)}
              className={`mx-auto flex size-7 items-center justify-center rounded-full transition-colors ${
                isStart || isEnd
                  ? "bg-accent text-background"
                  : inRange
                    ? "bg-primary/10 text-primary"
                    : "text-primary/70 hover:bg-primary/10"
              }`}
            >
              {day}
            </button>
          );
        })}
      </div>
    </div>
  );
}

export function LogsDatePicker({
  range,
  onRangeChange,
}: {
  range: LogsDateRange;
  onRangeChange: (range: LogsDateRange) => void;
}) {
  const [open, setOpen] = useState(false);
  const [pickingEnd, setPickingEnd] = useState(false);
  const [monthOffset, setMonthOffset] = useState(0);
  const wrapper = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    const onPointerDown = (event: PointerEvent) => {
      if (!wrapper.current?.contains(event.target as Node)) setOpen(false);
    };
    document.addEventListener("pointerdown", onPointerDown);
    return () => document.removeEventListener("pointerdown", onPointerDown);
  }, [open]);

  const pick = (date: Date) => {
    if (!pickingEnd) {
      onRangeChange({ start: date, end: null });
      setPickingEnd(true);
      return;
    }
    const start = range.start <= date ? range.start : date;
    const end = range.start <= date ? date : range.start;
    onRangeChange({ start, end });
    setPickingEnd(false);
  };

  const firstMonth = new Date(todayDate().getFullYear(), todayDate().getMonth() + monthOffset, 1);
  const secondMonth = new Date(firstMonth.getFullYear(), firstMonth.getMonth() + 1, 1);

  return (
    <div ref={wrapper} className="relative">
      <button
        type="button"
        onClick={() => setOpen((value) => !value)}
        className="inline-flex cursor-pointer items-center gap-2 rounded-full bg-tertiary px-3 py-1.5 text-xs text-primary transition-colors hover:bg-primary/10"
      >
        <IoCalendarNumberOutline className="size-4" aria-hidden="true" />
        <span suppressHydrationWarning>{formatRange(range)}</span>
      </button>
      {open && (
        <div className="absolute right-0 top-full z-50 mt-2 flex rounded-xl border border-border-secondary bg-secondary p-3 shadow-lg">
          <div className="flex w-32 flex-col gap-1 border-r border-border pr-3 text-xs text-primary/60">
            {PRESETS.map((preset) => (
              <button
                key={preset}
                type="button"
                onClick={() => {
                  onRangeChange(getPresetRange(preset));
                  setPickingEnd(false);
                }}
                className="cursor-pointer whitespace-nowrap rounded-full px-2 py-1 text-left transition-colors hover:bg-primary/10 hover:text-primary"
              >
                {preset}
              </button>
            ))}
          </div>
          <div className="flex items-start gap-3 px-1">
            <button
              type="button"
              aria-label="Previous month"
              onClick={() => setMonthOffset((value) => value - 1)}
              className="mt-1 cursor-pointer rounded-full p-1 text-primary/60 transition-colors hover:bg-primary/10 hover:text-primary"
            >
              <GoChevronLeft className="size-4" aria-hidden="true" />
            </button>
            <MonthCalendar month={firstMonth} range={range} onPick={pick} />
            <MonthCalendar month={secondMonth} range={range} onPick={pick} />
            <button
              type="button"
              aria-label="Next month"
              onClick={() => setMonthOffset((value) => value + 1)}
              className="mt-1 cursor-pointer rounded-full p-1 text-primary/60 transition-colors hover:bg-primary/10 hover:text-primary"
            >
              <GoChevronRight className="size-4" aria-hidden="true" />
            </button>
          </div>
        </div>
      )}
    </div>
  );
}
