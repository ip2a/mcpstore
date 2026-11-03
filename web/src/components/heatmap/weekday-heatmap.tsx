"use client";

import type { Locale, Day as WeekDay } from "date-fns";
import { format } from "date-fns";
import type {
  ComponentPropsWithRef,
  CSSProperties,
  HTMLAttributes,
  ReactNode,
} from "react";
import { createContext, Fragment, use, useMemo } from "react";
import { cn } from "@/lib/utils";

export type WeekdayIndex = 0 | 1 | 2 | 3 | 4 | 5 | 6;

export type WeekdayHour =
  | 0
  | 1
  | 2
  | 3
  | 4
  | 5
  | 6
  | 7
  | 8
  | 9
  | 10
  | 11
  | 12
  | 13
  | 14
  | 15
  | 16
  | 17
  | 18
  | 19
  | 20
  | 21
  | 22
  | 23;

export type WeekdayHourlyActivity = {
  weekday: number; // 0 to 6 (Sunday to Saturday)
  hour: number; // 0 to 23
  value: number;
};

export type WeekdayHourlyActivityWithLevel = WeekdayHourlyActivity & {
  level: number;
  extra?: "row" | "column";
};

export type WeekdayExtraRowSpec = {
  label: ReactNode;
  compute: (data: WeekdayHourlyActivity[]) => number[]; // length 24
};

export type WeekdayExtraColumnSpec = {
  label: ReactNode;
  compute: (data: WeekdayHourlyActivity[]) => number[]; // length 7, indexed by weekday 0 to 6
};

type ResolvedExtraRow = {
  label: ReactNode;
  values: { hour: number; value: number; level: number }[];
};

type ResolvedExtraColumn = {
  label: ReactNode;
  values: { weekday: number; value: number; level: number }[];
};

type WeekdayHeatmapContextType = {
  data: WeekdayHourlyActivityWithLevel[];
  extraRow: ResolvedExtraRow | null;
  extraColumn: ResolvedExtraColumn | null;
  blockMargin: number;
  blockRadius: number;
  blockSize: number;
  blockAspectRatio: number;
  blockWidth: number;
  fontSize: number;
  labels: WeekdayHeatmapLabels;
  labelWidth: number;
  labelHeight: number;
  levels: number;
  isNormalized: boolean;
  totalCount: number;
  width: number;
  height: number;
  weekStart: WeekDay;
  colors?: ColorConfig;
};

export type WeekdayHeatmapLabels = {
  hours?: string[];
  endHour?: string | null;
  weekdays?: string[];
  stat?: string; // Stat text template. Placeholder: {{value}}
  cellLabel?: string; // aria-label template. Placeholders: {{weekday}}, {{hour}}, {{value}}
  heatmapLabel?: string; // aria-label for the heatmap SVG
  legendLabel?: string; // aria-label for the legend fieldset
  legendLevelLabel?: string; // aria-label template for legend swatches. Placeholder: {{level}}
};

export type ColorConfig = {
  empty?: string;
  scale?: string;
};

// Non-normalized: 1 empty + (levels-1) colored steps. Normalized: levels colored steps, no empty.
const colorStepCount = (levels: number, isNormalized: boolean) =>
  Math.max(1, isNormalized ? levels : levels - 1);

const calculateLevel = (
  value: number,
  minValue: number,
  maxValue: number,
  levels: number,
  isNormalized: boolean,
): number => {
  const steps = colorStepCount(levels, isNormalized);
  if (!Number.isFinite(value)) return isNormalized ? 1 : 0;
  if (isNormalized) {
    if (maxValue <= minValue) return 1;
    const range = maxValue - minValue;
    // Halving keeps opposite-signed finite extremes from overflowing.
    const percentage = Number.isFinite(range)
      ? (value - minValue) / range
      : (value / 2 - minValue / 2) / (maxValue / 2 - minValue / 2);
    return Math.max(1, Math.min(steps, Math.ceil(percentage * steps)));
  }
  if (value <= 0 || maxValue <= 0) return 0;
  const percentage = value / maxValue;
  return Math.max(1, Math.min(steps, Math.ceil(percentage * steps)));
};

const getLevelFill = (
  level: number,
  levels: number,
  isNormalized: boolean,
  highlighted = false,
  colors?: ColorConfig,
): string => {
  const emptyColor = colors?.empty ?? "var(--color-secondary)";
  const scaleColor = colors?.scale ?? "var(--color-chart-1)";

  if (level === 0) return emptyColor;
  const steps = colorStepCount(levels, isNormalized);
  const opacity =
    steps === 1 ? 100 : Math.round(20 + ((level - 1) * 80) / (steps - 1));
  const finalOpacity = highlighted ? Math.round(opacity * 0.6) : opacity;
  return `color-mix(in oklch, ${scaleColor} ${finalOpacity}%, transparent)`;
};

const DEFAULT_HOUR_LABELS = Array.from({ length: 24 }, (_, i) =>
  i.toString().padStart(2, "0"),
);

const TWELVE_HOUR_LABELS = Array.from({ length: 24 }, (_, i) => {
  if (i === 0) return "AM";
  if (i < 12) return String(i);
  if (i === 12) return "PM";
  return String(i - 12);
});

const HOURS = Array.from({ length: 24 }, (_, i) => i);

const DEFAULT_WEEKDAY_LABELS = [
  "Sun",
  "Mon",
  "Tue",
  "Wed",
  "Thu",
  "Fri",
  "Sat",
];

const generateWeekdayLabels = (locale: Locale): string[] =>
  Array.from({ length: 7 }, (_, i) => {
    const date = new Date(2000, 0, 2 + i); // Jan 2, 2000 is Sunday
    return format(date, "EEE", { locale });
  });

const normalizeData = (
  data: WeekdayHourlyActivity[],
): WeekdayHourlyActivity[] => {
  const cells = new Map<string, WeekdayHourlyActivity>();
  for (const activity of data) {
    if (!Number.isFinite(activity.value)) {
      throw new RangeError("Heatmap values must be finite numbers.");
    }
    if (
      !Number.isInteger(activity.hour) ||
      activity.hour < 0 ||
      activity.hour > 23
    ) {
      throw new RangeError("Heatmap hours must be integers from 0 to 23.");
    }
    if (
      !Number.isInteger(activity.weekday) ||
      activity.weekday < 0 ||
      activity.weekday > 6
    ) {
      throw new RangeError("Heatmap weekdays must be integers from 0 to 6.");
    }
    cells.set(`${activity.weekday}:${activity.hour}`, activity);
  }
  return [...cells.values()].sort(
    (a, b) => a.weekday - b.weekday || a.hour - b.hour,
  );
};

const validateAggregate = (values: number[], expectedLength: number) => {
  if (
    !Array.isArray(values) ||
    values.length !== expectedLength ||
    !Array.from(values).every(Number.isFinite)
  ) {
    throw new RangeError(
      `Aggregate must return exactly ${expectedLength} finite values.`,
    );
  }
};

const EMPTY_STYLE: CSSProperties = {};
const LABEL_MARGIN = 8;
const PADDING = 4;

const WeekdayHeatmapContext = createContext<WeekdayHeatmapContextType | null>(
  null,
);

const useWeekdayHeatmap = () => {
  const context = use(WeekdayHeatmapContext);

  if (!context) {
    throw new Error(
      "WeekdayHeatmap components must be used within a WeekdayHeatmap",
    );
  }

  return context;
};

export type WeekdayHeatmapProps = HTMLAttributes<HTMLDivElement> & {
  data: WeekdayHourlyActivity[];
  weekStart?: WeekDay;
  use12Hour?: boolean;
  blockSize?: number;
  blockMargin?: number;
  blockRadius?: number;
  blockAspectRatio?: number;
  levels?: number;
  isNormalized?: boolean;
  colors?: ColorConfig;
  locale?: Locale;
  labels?: WeekdayHeatmapLabels;
  fontSize?: number;
  emptyState?: ReactNode;
  totalCount?: number;
  extraRow?: WeekdayExtraRowSpec;
  extraColumn?: WeekdayExtraColumnSpec;
  style?: CSSProperties;
  className?: string;
  children: ReactNode;
};

export const WeekdayHeatmap = ({
  data: inputData,
  weekStart = 0,
  use12Hour = false,
  blockSize = 24,
  blockMargin = 4,
  blockRadius = 2,
  blockAspectRatio = 1,
  levels: levelsProp = 5,
  isNormalized = false,
  colors,
  locale,
  labels: labelsProp,
  fontSize = 14,
  emptyState,
  totalCount: totalCountProp,
  extraRow,
  extraColumn,
  style = EMPTY_STYLE,
  className,
  children,
  ...props
}: WeekdayHeatmapProps) => {
  const data = useMemo(() => normalizeData(inputData), [inputData]);
  for (const [name, value] of Object.entries({
    blockSize,
    blockAspectRatio,
    fontSize,
  })) {
    if (!Number.isFinite(value) || value <= 0) {
      throw new RangeError(`${name} must be a finite number greater than 0.`);
    }
  }
  for (const [name, value] of Object.entries({ blockMargin, blockRadius })) {
    if (!Number.isFinite(value) || value < 0) {
      throw new RangeError(
        `${name} must be a finite number greater than or equal to 0.`,
      );
    }
  }

  if (!Number.isInteger(weekStart) || weekStart < 0 || weekStart > 6) {
    throw new RangeError("weekStart must be an integer from 0 to 6.");
  }
  const levels = levelsProp;
  if (!Number.isInteger(levels) || levels < (isNormalized ? 1 : 2)) {
    throw new RangeError(
      "levels must be a positive integer, at least 2 when not normalized.",
    );
  }

  const dataWithLevels = useMemo((): WeekdayHourlyActivityWithLevel[] => {
    if (data.length === 0) return [];

    const maxRegular = data.reduce((m, d) => Math.max(m, d.value), -Infinity);
    const minRegular = isNormalized
      ? data.reduce((m, d) => Math.min(m, d.value), Infinity)
      : 0;

    return data.map((activity) => ({
      ...activity,
      level: calculateLevel(
        activity.value,
        minRegular,
        maxRegular,
        levels,
        isNormalized,
      ),
    }));
  }, [data, levels, isNormalized]);

  const resolvedExtraRow = useMemo<ResolvedExtraRow | null>(() => {
    if (!extraRow || data.length === 0) return null;
    const values = extraRow.compute(data);
    validateAggregate(values, 24);
    const maxVal = values.reduce((m, v) => Math.max(m, v), -Infinity);
    const minVal = isNormalized
      ? values.reduce((m, v) => Math.min(m, v), Infinity)
      : 0;
    return {
      label: extraRow.label,
      values: values.map((value, hour) => ({
        hour,
        value,
        level: calculateLevel(value, minVal, maxVal, levels, isNormalized),
      })),
    };
  }, [extraRow, data, levels, isNormalized]);

  const resolvedExtraColumn = useMemo<ResolvedExtraColumn | null>(() => {
    if (!extraColumn || data.length === 0) return null;
    const values = extraColumn.compute(data);
    validateAggregate(values, 7);
    const maxVal = values.reduce((m, v) => Math.max(m, v), -Infinity);
    const minVal = isNormalized
      ? values.reduce((m, v) => Math.min(m, v), Infinity)
      : 0;
    return {
      label: extraColumn.label,
      values: values.map((value, weekday) => ({
        weekday,
        value,
        level: calculateLevel(value, minVal, maxVal, levels, isNormalized),
      })),
    };
  }, [extraColumn, data, levels, isNormalized]);

  const labels = useMemo<WeekdayHeatmapLabels>(() => {
    const weekdayLabels = locale
      ? generateWeekdayLabels(locale)
      : DEFAULT_WEEKDAY_LABELS;

    return {
      hours: use12Hour ? TWELVE_HOUR_LABELS : DEFAULT_HOUR_LABELS,
      endHour: use12Hour ? "12" : "00",
      weekdays: weekdayLabels,
      cellLabel: "{{weekday}} {{hour}}: {{value}}",
      heatmapLabel: "Activity heatmap by weekday and hour",
      legendLabel: "Activity intensity legend",
      legendLevelLabel: "Level {{level}}",
      ...labelsProp,
    };
  }, [locale, use12Hour, labelsProp]);

  const labelWidth = fontSize * 3.5 + LABEL_MARGIN;

  const totalCount =
    totalCountProp !== undefined
      ? totalCountProp
      : dataWithLevels.reduce((sum, a) => sum + a.value, 0);
  if (!Number.isFinite(totalCount)) {
    throw new RangeError(
      "totalCount and the sum of heatmap values must be finite numbers.",
    );
  }

  const blockWidth = blockSize * blockAspectRatio;
  const labelHeight = fontSize + LABEL_MARGIN;
  const hasExtraColumn = resolvedExtraColumn !== null;
  const hasExtraRow = resolvedExtraRow !== null;
  const width =
    24 * (blockWidth + blockMargin) +
    (hasExtraColumn ? blockSize / 2 + (blockWidth + blockMargin) * 2 : 0) -
    blockMargin +
    labelWidth;
  const rowCount = hasExtraRow ? 8 : 7;
  const extraRowGap = hasExtraRow ? blockSize + blockMargin : 0;
  const height =
    rowCount * (blockSize + blockMargin) +
    extraRowGap -
    blockMargin +
    labelHeight;

  if (![blockWidth, width, height].every(Number.isFinite)) {
    throw new RangeError("Heatmap layout dimensions must remain finite.");
  }

  const contextValue = useMemo<WeekdayHeatmapContextType>(
    () => ({
      data: dataWithLevels,
      extraRow: resolvedExtraRow,
      extraColumn: resolvedExtraColumn,
      blockMargin,
      blockRadius,
      blockSize,
      blockAspectRatio,
      blockWidth,
      fontSize,
      labels,
      labelWidth,
      labelHeight,
      levels,
      isNormalized,
      totalCount,
      width,
      height,
      weekStart,
      colors,
    }),
    [
      dataWithLevels,
      resolvedExtraRow,
      resolvedExtraColumn,
      blockMargin,
      blockRadius,
      blockSize,
      blockAspectRatio,
      blockWidth,
      fontSize,
      labels,
      labelWidth,
      labelHeight,
      levels,
      isNormalized,
      totalCount,
      width,
      height,
      weekStart,
      colors,
    ],
  );

  if (data.length === 0) {
    return emptyState ?? null;
  }

  return (
    <WeekdayHeatmapContext value={contextValue}>
      <div
        data-slot="weekday-heatmap"
        className={cn("flex w-max max-w-full flex-col gap-2 p-4", className)}
        style={{ fontSize, ...style }}
        {...props}
      >
        {children}
      </div>
    </WeekdayHeatmapContext>
  );
};

export type WeekdayHeatmapBlockProps = ComponentPropsWithRef<"rect"> & {
  activity: WeekdayHourlyActivityWithLevel;
  extra?: "row" | "column";
  highlighted?: boolean;
  onCellClick?: (activity: WeekdayHourlyActivityWithLevel) => void;
  onCellHover?: (activity: WeekdayHourlyActivityWithLevel | null) => void;
  className?: string;
};

export const WeekdayHeatmapBlock = ({
  ref,
  activity,
  extra = activity.extra,
  highlighted = false,
  onCellClick,
  onCellHover,
  onClick,
  onKeyDown,
  onMouseEnter,
  onMouseLeave,
  className,
  style: styleProp,
  ...props
}: WeekdayHeatmapBlockProps) => {
  const {
    blockSize,
    blockWidth,
    blockMargin,
    blockRadius,
    labelWidth,
    labelHeight,
    levels,
    isNormalized,
    weekStart,
    labels,
    colors,
    extraRow,
    extraColumn,
  } = useWeekdayHeatmap();

  const level = Math.max(
    0,
    Math.min(colorStepCount(levels, isNormalized), activity.level),
  );

  const isExtraRow = extra === "row";
  const isExtraColumn = extra === "column";

  const rowIndex = isExtraRow ? 7 : (activity.weekday - weekStart + 7) % 7;
  const extraRowGap = isExtraRow ? blockSize + blockMargin : 0;
  const yPosition =
    labelHeight + (blockSize + blockMargin) * rowIndex + extraRowGap;

  const extraColumnGap = isExtraColumn ? blockWidth + blockMargin : 0;
  const xPosition = isExtraColumn
    ? labelWidth +
      24 * (blockWidth + blockMargin) +
      blockSize / 2 +
      extraColumnGap
    : labelWidth + (blockWidth + blockMargin) * activity.hour;

  const weekdayToken = isExtraRow
    ? typeof extraRow?.label === "string"
      ? extraRow.label
      : "Aggregate"
    : (labels.weekdays?.[activity.weekday] ?? String(activity.weekday));
  const hourToken = isExtraColumn
    ? typeof extraColumn?.label === "string"
      ? extraColumn.label
      : "Aggregate"
    : `${String(activity.hour).padStart(2, "0")}:00`;
  const ariaLabel = (labels.cellLabel ?? "{{weekday}} {{hour}}: {{value}}")
    .replaceAll("{{weekday}}", () => weekdayToken)
    .replaceAll("{{hour}}", () => hourToken)
    .replaceAll("{{value}}", String(activity.value));

  return (
    <rect
      ref={ref}
      data-slot="weekday-heatmap-block"
      role={onCellClick ? "button" : "img"}
      tabIndex={onCellClick ? 0 : -1}
      aria-label={ariaLabel}
      className={cn(
        "focus-visible:stroke-current focus-visible:stroke-2 focus-visible:outline-none motion-safe:transition-opacity motion-safe:hover:opacity-70",
        onCellClick &&
          "focus-visible:ring-ring cursor-pointer focus-visible:ring-2 focus-visible:outline-none",
        className,
      )}
      data-value={activity.value}
      data-weekday={isExtraRow ? "extra" : activity.weekday}
      data-hour={isExtraColumn ? "extra" : activity.hour}
      data-level={level}
      data-highlighted={highlighted || undefined}
      height={blockSize}
      rx={blockRadius}
      ry={blockRadius}
      width={blockWidth}
      x={xPosition}
      y={yPosition}
      style={{
        fill: getLevelFill(level, levels, isNormalized, highlighted, colors),
        ...styleProp,
      }}
      onClick={(event) => {
        onCellClick?.(activity);
        onClick?.(event);
      }}
      onKeyDown={(event) => {
        if (onCellClick && (event.key === "Enter" || event.key === " ")) {
          event.preventDefault();
          onCellClick(activity);
        }
        onKeyDown?.(event);
      }}
      onMouseEnter={(event) => {
        onCellHover?.(activity);
        onMouseEnter?.(event);
      }}
      onMouseLeave={(event) => {
        onCellHover?.(null);
        onMouseLeave?.(event);
      }}
      {...props}
    />
  );
};
WeekdayHeatmapBlock.displayName = "WeekdayHeatmapBlock";

export type WeekdayHeatmapBodyProps = Omit<
  HTMLAttributes<HTMLDivElement>,
  "children"
> & {
  hideHourLabels?: boolean;
  hideWeekdayLabels?: boolean;
  className?: string;
  labelClassName?: string;
  children: (props: {
    activity: WeekdayHourlyActivityWithLevel;
    weekdayIndex: number;
  }) => ReactNode;
  renderExtraRow?: (props: {
    activity: WeekdayHourlyActivityWithLevel;
    weekdayIndex: number;
  }) => ReactNode;
  renderExtraColumn?: (props: {
    activity: WeekdayHourlyActivityWithLevel;
    weekdayIndex: number;
  }) => ReactNode;
};

export const WeekdayHeatmapBody = ({
  hideHourLabels = false,
  hideWeekdayLabels = false,
  className,
  labelClassName,
  children,
  renderExtraRow,
  renderExtraColumn,
  ...props
}: WeekdayHeatmapBodyProps) => {
  const {
    data,
    extraRow,
    extraColumn,
    blockSize,
    blockWidth,
    blockMargin,
    labels,
    labelWidth,
    labelHeight,
    fontSize,
    weekStart,
  } = useWeekdayHeatmap();

  const hasExtraRow = extraRow !== null;
  const hasExtraColumn = extraColumn !== null;

  const activityMap = useMemo(() => {
    const map = new Map<string, WeekdayHourlyActivityWithLevel>();
    data.forEach((activity) => {
      map.set(`${activity.weekday}-${activity.hour}`, activity);
    });
    return map;
  }, [data]);

  const extraColumnMap = useMemo(() => {
    if (!extraColumn) return null;
    const map = new Map<
      number,
      { weekday: number; value: number; level: number }
    >();
    extraColumn.values.forEach((v) => {
      map.set(v.weekday, v);
    });
    return map;
  }, [extraColumn]);

  const regularActivities = useMemo(() => {
    const activities: {
      activity: WeekdayHourlyActivityWithLevel;
      weekdayIndex: number;
    }[] = [];
    for (let di = 0; di < 7; di++) {
      const weekday = (weekStart + di) % 7;
      for (let hour = 0; hour < 24; hour++) {
        activities.push({
          activity: activityMap.get(`${weekday}-${hour}`) ?? {
            weekday,
            hour,
            value: 0,
            level: 0,
          },
          weekdayIndex: di,
        });
      }
    }
    return activities;
  }, [activityMap, weekStart]);

  const extraColumnActivities = useMemo(() => {
    if (!extraColumnMap) return [];
    return Array.from({ length: 7 }, (_, di) => {
      const weekday = (weekStart + di) % 7;
      const entry = extraColumnMap.get(weekday);
      return entry
        ? {
            activity: {
              extra: "column" as const,
              weekday,
              hour: 0,
              value: entry.value,
              level: entry.level,
            },
            weekdayIndex: di,
          }
        : null;
    }).filter((a): a is NonNullable<typeof a> => a !== null);
  }, [extraColumnMap, weekStart]);

  const extraRowActivities = useMemo(() => {
    if (!extraRow) return [];
    return extraRow.values.map(({ hour, value, level }) => ({
      activity: { extra: "row" as const, weekday: 0, hour, value, level },
      weekdayIndex: 7,
    }));
  }, [extraRow]);

  const rowCount = hasExtraRow ? 8 : 7;
  const extraRowGap = hasExtraRow ? blockSize + blockMargin : 0;
  const svgHeight =
    rowCount * (blockSize + blockMargin) +
    extraRowGap -
    blockMargin +
    labelHeight;
  const extraColumnGap = hasExtraColumn ? blockWidth + blockMargin : 0;
  const svgWidth = hasExtraColumn
    ? labelWidth +
      24 * (blockWidth + blockMargin) +
      blockSize / 2 +
      (blockWidth + blockMargin) +
      extraColumnGap
    : labelWidth + 24 * (blockWidth + blockMargin) - blockMargin;

  const orderedWeekdayIndices = Array.from(
    { length: 7 },
    (_, i) => (weekStart + i) % 7,
  );

  return (
    <div
      data-slot="weekday-heatmap-body"
      className={cn("max-w-full overflow-x-auto overflow-y-hidden", className)}
      {...props}
    >
      <svg
        role="group"
        aria-label={
          labels.heatmapLabel ?? "Activity heatmap by weekday and hour"
        }
        className="focus-visible:ring-ring block overflow-visible rounded-sm focus-visible:ring-2 focus-visible:outline-none"
        height={svgHeight + PADDING * 2}
        viewBox={`${-PADDING} ${-PADDING} ${svgWidth + PADDING * 2} ${svgHeight + PADDING * 2}`}
        width={svgWidth + PADDING * 2}
      >
        {!hideHourLabels && (
          <g className={cn("fill-current font-mono", labelClassName)}>
            {HOURS.map((hour) => {
              const label = labels.hours?.[hour] ?? "";
              return (
                <text
                  key={`hour-${hour}`}
                  x={labelWidth + (blockWidth + blockMargin) * hour}
                  y={0}
                  textAnchor="middle"
                  dominantBaseline="hanging"
                  style={{ fontSize: `${fontSize * 0.75}px` }}
                >
                  {label}
                </text>
              );
            })}
            {labels.endHour != null && (
              <text
                key="hour-end"
                x={labelWidth + (blockWidth + blockMargin) * 24}
                y={0}
                textAnchor="middle"
                dominantBaseline="hanging"
                style={{ fontSize: `${fontSize * 0.75}px` }}
              >
                {labels.endHour}
              </text>
            )}
            {hasExtraColumn && (
              <text
                key="extra-col-label"
                x={
                  labelWidth +
                  24 * (blockWidth + blockMargin) +
                  blockSize / 2 +
                  (blockWidth + blockMargin) +
                  blockWidth / 2
                }
                y={0}
                textAnchor="middle"
                dominantBaseline="hanging"
                style={{ fontSize: `${fontSize * 0.75}px` }}
              >
                {extraColumn?.label}
              </text>
            )}
          </g>
        )}

        {!hideWeekdayLabels && (
          <g className={cn("fill-current font-mono", labelClassName)}>
            {orderedWeekdayIndices.map((weekday, displayIndex) => {
              const label = labels.weekdays?.[weekday] ?? "";
              const yPosition =
                labelHeight +
                (blockSize + blockMargin) * displayIndex +
                blockSize / 2;
              return (
                <text
                  key={`weekday-${weekday}`}
                  x={labelWidth - 8}
                  y={yPosition}
                  dominantBaseline="middle"
                  textAnchor="end"
                  style={{ fontSize: `${fontSize * 0.75}px` }}
                >
                  {label}
                </text>
              );
            })}
            {hasExtraRow && (
              <text
                key="weekday-extra"
                x={labelWidth - 8}
                y={
                  labelHeight +
                  (blockSize + blockMargin) * 7 +
                  (blockSize + blockMargin) +
                  blockSize / 2
                }
                dominantBaseline="middle"
                textAnchor="end"
                style={{ fontSize: `${fontSize * 0.75}px` }}
              >
                {extraRow?.label}
              </text>
            )}
          </g>
        )}

        {regularActivities.map(({ activity, weekdayIndex }) => (
          <Fragment key={`${activity.weekday}-${activity.hour}`}>
            {children({ activity, weekdayIndex })}
          </Fragment>
        ))}
        {extraColumnActivities.map(({ activity, weekdayIndex }) => (
          <Fragment key={`extra-column-${activity.weekday}`}>
            {(renderExtraColumn ?? children)({ activity, weekdayIndex })}
          </Fragment>
        ))}
        {extraRowActivities.map(({ activity, weekdayIndex }) => (
          <Fragment key={`extra-row-${activity.hour}`}>
            {(renderExtraRow ?? children)({ activity, weekdayIndex })}
          </Fragment>
        ))}
      </svg>
    </div>
  );
};

export type WeekdayHeatmapFooterProps = HTMLAttributes<HTMLDivElement>;

export const WeekdayHeatmapFooter = ({
  className,
  ...props
}: WeekdayHeatmapFooterProps) => (
  <div
    data-slot="weekday-heatmap-footer"
    className={cn(
      "flex flex-wrap gap-1 whitespace-nowrap sm:gap-x-4",
      className,
    )}
    {...props}
  />
);

export type WeekdayHeatmapStatProps = Omit<
  HTMLAttributes<HTMLDivElement>,
  "children"
> & {
  compute?: (data: WeekdayHourlyActivityWithLevel[]) => number | string;
  label?: string; // Template overriding labels.stat. Placeholder: {{value}}
  children?: (result: {
    value: number | string;
    data: WeekdayHourlyActivityWithLevel[];
  }) => ReactNode;
};

export const WeekdayHeatmapStat = ({
  compute,
  label,
  className,
  children,
  ...props
}: WeekdayHeatmapStatProps) => {
  const { data, totalCount, labels } = useWeekdayHeatmap();

  const value = compute ? compute(data) : totalCount;

  if (typeof value !== "string" && !Number.isFinite(value)) {
    throw new RangeError(
      "Stat compute must return a finite number or a string.",
    );
  }

  if (children) {
    return <>{children({ value, data })}</>;
  }

  const template = label ?? labels.stat ?? "Contributions: {{value}}";

  return (
    <div
      data-slot="weekday-heatmap-stat"
      className={cn("text-muted-foreground tabular-nums", className)}
      {...props}
    >
      {template.replaceAll("{{value}}", () => String(value))}
    </div>
  );
};

export type WeekdayHeatmapLegendProps = Omit<
  HTMLAttributes<HTMLFieldSetElement>,
  "children"
> & {
  labels?: { less?: string; more?: string };
  children?: (props: { level: number }) => ReactNode;
};

export const WeekdayHeatmapLegend = ({
  labels: labelsProp,
  className,
  children,
  ...props
}: WeekdayHeatmapLegendProps) => {
  const {
    levels,
    isNormalized,
    blockSize,
    blockWidth,
    blockRadius,
    colors,
    labels,
  } = useWeekdayHeatmap();

  const lessLabel = labelsProp?.less ?? "Less";
  const moreLabel = labelsProp?.more ?? "More";

  const legendLevels = Array.from({ length: levels }, (_, i) =>
    isNormalized ? i + 1 : i,
  );

  return (
    <fieldset
      data-slot="weekday-heatmap-legend"
      aria-label={labels.legendLabel ?? "Activity intensity legend"}
      className={cn(
        "text-muted-foreground ml-auto flex items-center gap-1",
        className,
      )}
      {...props}
    >
      <span className="mr-1 text-xs font-medium">{lessLabel}</span>
      {legendLevels.map((level) =>
        children ? (
          <Fragment key={`legend-level-${level}`}>
            {children({ level })}
          </Fragment>
        ) : (
          <svg
            role="img"
            aria-label={(
              labels.legendLevelLabel ?? "Level {{level}}"
            ).replaceAll("{{level}}", String(level))}
            height={blockSize}
            key={`legend-level-${level}`}
            width={blockWidth}
          >
            <rect
              data-level={level}
              height={blockSize}
              rx={blockRadius}
              ry={blockRadius}
              width={blockWidth}
              style={{
                fill: getLevelFill(level, levels, isNormalized, false, colors),
              }}
            />
          </svg>
        ),
      )}
      <span className="ml-1 text-xs font-medium">{moreLabel}</span>
    </fieldset>
  );
};
