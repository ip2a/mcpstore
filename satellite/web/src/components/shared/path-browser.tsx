import { Fragment, type ReactNode, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { keepPreviousData, useQuery } from "@tanstack/react-query";
import { ArrowUpIcon, CheckIcon, ChevronRightIcon, FolderIcon, PencilIcon, RefreshCwIcon, XIcon } from "lucide-react";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import {
  Breadcrumb,
  BreadcrumbEllipsis,
  BreadcrumbItem,
  BreadcrumbLink,
  BreadcrumbList,
  BreadcrumbPage,
  BreadcrumbSeparator,
} from "@/components/ui/breadcrumb";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { Empty, EmptyDescription, EmptyHeader, EmptyTitle } from "@/components/ui/empty";
import { Input } from "@/components/ui/input";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Separator } from "@/components/ui/separator";
import { Spinner } from "@/components/ui/spinner";
import { useI18n } from "@/lib/i18n-context";
import { cn } from "@/lib/utils";
import { listDirectories } from "@/lib/api/filesystem";

type PathCrumb = {
  label: string;
  path: string;
};

type CollapsedPathCrumbs = {
  leading: PathCrumb[];
  hidden: PathCrumb[];
  trailing: PathCrumb[];
};

const BREADCRUMB_GAP_PX = 6;

function pathCrumbs(path: string): PathCrumb[] {
  const windowsPath = /^[A-Za-z]:[\\/]/.test(path);
  const separator = windowsPath ? "\\" : "/";
  const segments = path.split(/[\\/]+/).filter(Boolean);

  if (windowsPath) {
    return segments.map((segment, index) => ({
      label: segment,
      path: `${segments.slice(0, index + 1).join(separator)}${index === 0 ? separator : ""}`,
    }));
  }

  return [
    { label: "/", path: "/" },
    ...segments.map((segment, index) => ({
      label: segment,
      path: `/${segments.slice(0, index + 1).join("/")}`,
    })),
  ];
}

function toCollapsedPathCrumbs(crumbs: PathCrumb[], leadingCount: number, trailingCount: number): CollapsedPathCrumbs {
  if (trailingCount === 0) {
    return { leading: crumbs.slice(0, leadingCount), hidden: [], trailing: [] };
  }

  return {
    leading: crumbs.slice(0, leadingCount),
    hidden: crumbs.slice(leadingCount, crumbs.length - trailingCount),
    trailing: crumbs.slice(crumbs.length - trailingCount),
  };
}

function initialCollapsedPathCrumbs(crumbs: PathCrumb[]): CollapsedPathCrumbs {
  if (crumbs.length <= 2) return { leading: crumbs, hidden: [], trailing: [] };
  return {
    leading: [crumbs[0]],
    hidden: crumbs.slice(1, -1),
    trailing: [crumbs[crumbs.length - 1]],
  };
}

function collapsedPathWidth(
  itemWidths: number[],
  separatorWidth: number,
  ellipsisWidth: number,
  leadingCount: number,
  trailingCount: number,
) {
  const totalItems = itemWidths.length;
  const hiddenCount = totalItems - leadingCount - trailingCount;
  const leadingWidth = itemWidths.slice(0, leadingCount).reduce((sum, width) => sum + width, 0);
  const trailingWidth = itemWidths.slice(totalItems - trailingCount).reduce((sum, width) => sum + width, 0);
  const visibleNodes = leadingCount + trailingCount + (hiddenCount > 0 ? 1 : 0);
  const separatorNodes = Math.max(0, visibleNodes - 1);
  const flexChildren = visibleNodes + separatorNodes;

  return (
    leadingWidth +
    trailingWidth +
    separatorNodes * separatorWidth +
    (hiddenCount > 0 ? ellipsisWidth : 0) +
    Math.max(0, flexChildren - 1) * BREADCRUMB_GAP_PX
  );
}

function computeCollapsedPathCrumbs(
  crumbs: PathCrumb[],
  itemWidths: number[],
  separatorWidth: number,
  ellipsisWidth: number,
  availableWidth: number,
): CollapsedPathCrumbs {
  const totalItems = crumbs.length;
  if (totalItems === 0) return { leading: [], hidden: [], trailing: [] };
  if (totalItems === 1) return { leading: crumbs, hidden: [], trailing: [] };
  if (availableWidth <= 0) return initialCollapsedPathCrumbs(crumbs);

  const safeWidth = Math.max(0, availableWidth - 4);
  const minimum = initialCollapsedPathCrumbs(crumbs);

  for (let trailingCount = 0; trailingCount <= Math.min(3, totalItems - 1); trailingCount += 1) {
    for (let leadingCount = totalItems - trailingCount; leadingCount >= 1; leadingCount -= 1) {
      const hiddenCount = totalItems - leadingCount - trailingCount;
      if (hiddenCount < 0) continue;

      if (
        collapsedPathWidth(itemWidths, separatorWidth, ellipsisWidth, leadingCount, trailingCount) >
        safeWidth
      ) {
        continue;
      }

      return toCollapsedPathCrumbs(crumbs, leadingCount, trailingCount);
    }
  }

  return minimum;
}

function collapsedStatesEqual(left: CollapsedPathCrumbs, right: CollapsedPathCrumbs) {
  return (
    left.leading.length === right.leading.length &&
    left.hidden.length === right.hidden.length &&
    left.trailing.length === right.trailing.length &&
    left.leading.every((crumb, index) => crumb.path === right.leading[index]?.path) &&
    left.hidden.every((crumb, index) => crumb.path === right.hidden[index]?.path) &&
    left.trailing.every((crumb, index) => crumb.path === right.trailing[index]?.path)
  );
}

function PathBreadcrumbNav({
  slotRef,
  rowRef,
  crumbs,
  layoutSeed,
  onNavigate,
}: {
  slotRef: React.RefObject<HTMLDivElement | null>;
  rowRef: React.RefObject<HTMLDivElement | null>;
  crumbs: PathCrumb[];
  layoutSeed: string;
  onNavigate: (path: string) => void;
}) {
  const { t } = useI18n();
  const listRef = useRef<HTMLOListElement>(null);
  const measureRef = useRef<HTMLDivElement>(null);
  const [collapsed, setCollapsed] = useState<CollapsedPathCrumbs>(() => initialCollapsedPathCrumbs(crumbs));
  const crumbKey = crumbs.map((crumb) => crumb.path).join("|");

  useLayoutEffect(() => {
    setCollapsed(initialCollapsedPathCrumbs(crumbs));
  }, [crumbKey, crumbs, layoutSeed]);

  useLayoutEffect(() => {
    const slot = slotRef.current;
    const row = rowRef.current;
    const measure = measureRef.current;
    if (!slot || !row || !measure || crumbs.length === 0) return;

    let frame = 0;
    let cancelled = false;

    const measureAndUpdate = () => {
      if (cancelled) return;

      const availableWidth = slot.clientWidth;
      if (availableWidth <= 0) {
        frame += 1;
        if (frame <= 8) requestAnimationFrame(measureAndUpdate);
        return;
      }

      const separator = measure.querySelector("[data-path-crumb-separator]");
      const ellipsis = measure.querySelector("[data-path-crumb-ellipsis]");
      if (!separator || !ellipsis) return;

      const separatorWidth = separator.getBoundingClientRect().width;
      const ellipsisWidth = ellipsis.getBoundingClientRect().width;
      const itemWidths = Array.from(measure.querySelectorAll("[data-path-crumb-item]")).map(
        (element) => element.getBoundingClientRect().width,
      );

      if (itemWidths.length !== crumbs.length) return;

      const next = computeCollapsedPathCrumbs(
        crumbs,
        itemWidths,
        separatorWidth,
        ellipsisWidth,
        availableWidth,
      );

      setCollapsed((current) => (collapsedStatesEqual(current, next) ? current : next));

      requestAnimationFrame(() => {
        if (cancelled) return;
        const list = listRef.current;
        if (!list || crumbs.length <= 2) return;

        const minimum = initialCollapsedPathCrumbs(crumbs);
        if (list.scrollWidth <= slot.clientWidth + 1) return;

        setCollapsed((current) => (collapsedStatesEqual(current, minimum) ? current : minimum));
      });
    };

    measureAndUpdate();

    const observer = new ResizeObserver(measureAndUpdate);
    observer.observe(row);
    observer.observe(slot);

    return () => {
      cancelled = true;
      observer.disconnect();
    };
  }, [crumbKey, crumbs, layoutSeed, rowRef, slotRef]);

  function renderCrumb(crumb: PathCrumb, isCurrent: boolean) {
    return (
      <BreadcrumbItem className="max-w-full min-w-0 shrink">
        {isCurrent ? (
          <BreadcrumbPage className="block truncate">{crumb.label}</BreadcrumbPage>
        ) : (
          <BreadcrumbLink asChild>
            <button type="button" className="block max-w-full truncate" onClick={() => onNavigate(crumb.path)}>
              {crumb.label}
            </button>
          </BreadcrumbLink>
        )}
      </BreadcrumbItem>
    );
  }

  const { leading, hidden, trailing } = collapsed;

  return (
    <>
      <div
        ref={measureRef}
        aria-hidden
        className="pointer-events-none invisible absolute top-0 left-0 flex items-center gap-1.5 text-sm text-muted-foreground"
      >
        {crumbs.map((crumb) => (
          <span key={crumb.path} data-path-crumb-item className="inline-flex whitespace-nowrap">
            {crumb.label}
          </span>
        ))}
        <span data-path-crumb-separator className="inline-flex items-center [&>svg]:size-3.5">
          <ChevronRightIcon />
        </span>
        <span data-path-crumb-ellipsis className="inline-flex size-5 items-center justify-center">
          <BreadcrumbEllipsis />
        </span>
      </div>

      <Breadcrumb className="min-w-0 w-full max-w-full">
        <BreadcrumbList ref={listRef} className="max-w-full flex-nowrap overflow-hidden">
          {leading.map((crumb, index) => (
            <Fragment key={crumb.path}>
              {index > 0 ? <BreadcrumbSeparator className="shrink-0" /> : null}
              {renderCrumb(crumb, trailing.length === 0 && hidden.length === 0 && index === leading.length - 1)}
            </Fragment>
          ))}
          {hidden.length ? (
            <>
              <BreadcrumbSeparator className="shrink-0" />
              <BreadcrumbItem className="shrink-0">
                <DropdownMenu>
                  <DropdownMenuTrigger asChild>
                    <button
                      type="button"
                      className="flex size-5 items-center justify-center rounded-md transition-colors hover:text-foreground"
                      aria-label={t("pathBrowserShowCollapsedSegments")}
                    >
                      <BreadcrumbEllipsis />
                    </button>
                  </DropdownMenuTrigger>
                  <DropdownMenuContent align="start" className="max-w-sm">
                    {hidden.map((crumb) => (
                      <DropdownMenuItem key={crumb.path} onClick={() => onNavigate(crumb.path)}>
                        <span className="truncate">{crumb.label}</span>
                      </DropdownMenuItem>
                    ))}
                  </DropdownMenuContent>
                </DropdownMenu>
              </BreadcrumbItem>
            </>
          ) : null}
          {trailing.map((crumb, index) => (
            <Fragment key={crumb.path}>
              <BreadcrumbSeparator className="shrink-0" />
              {renderCrumb(crumb, index === trailing.length - 1)}
            </Fragment>
          ))}
        </BreadcrumbList>
      </Breadcrumb>
    </>
  );
}

function PathNavButton({
  className,
  ...props
}: React.ComponentProps<typeof Button>) {
  return (
    <Button
      type="button"
      variant="outline"
      size="icon"
      className={cn("size-8 shrink-0 rounded-none shadow-xs", className)}
      {...props}
    />
  );
}

function directoryLabel(path: string) {
  const normalized = path.replace(/[\\/]+$/, "");
  const segments = normalized.split(/[\\/]+/).filter(Boolean);
  return segments.at(-1) ?? path;
}

function normalizeDirectoryPath(path: string) {
  return path.replace(/[\\/]+$/, "");
}

function DirectoryList({
  directories,
  currentPath,
  emptyDescription,
  isLoading,
  onNavigate,
}: {
  directories: { name: string; path: string }[];
  currentPath?: string | null;
  emptyDescription: string;
  isLoading: boolean;
  onNavigate: (path: string) => void;
}) {
  const { t } = useI18n();

  if (isLoading) {
    return (
      <div className="flex min-h-24 items-center justify-center gap-2 text-sm text-muted-foreground">
        <Spinner />
        {t("pathBrowserLoadingDirectories")}
      </div>
    );
  }

  if (!directories.length) {
    return (
      <Empty className="min-h-24 border-0">
        <EmptyHeader>
          <EmptyTitle>{t("pathBrowserNoDirectories")}</EmptyTitle>
          <EmptyDescription>{emptyDescription}</EmptyDescription>
        </EmptyHeader>
      </Empty>
    );
  }

  return (
    <div className="flex flex-col gap-0.5 p-1.5">
      {directories.map((directory) => {
        const isCurrent =
          currentPath != null &&
          normalizeDirectoryPath(directory.path) === normalizeDirectoryPath(currentPath);
        return (
          <Button
            key={directory.path}
            type="button"
            variant={isCurrent ? "secondary" : "ghost"}
            aria-current={isCurrent ? "true" : undefined}
            className="h-auto w-full justify-start px-2 py-2 font-normal"
            onClick={() => onNavigate(directory.path)}
          >
            <FolderIcon data-icon="inline-start" />
            <span className="truncate">{directory.name}</span>
          </Button>
        );
      })}
    </div>
  );
}

export function PathBrowser({
  initialPath,
  onPathChange,
  pathActions,
  filter = "",
  onFilterChange,
  active = true,
}: {
  initialPath?: string | null;
  onPathChange: (path: string) => void;
  pathActions?: ReactNode;
  filter?: string;
  onFilterChange?: (filter: string) => void;
  active?: boolean;
}) {
  const { t } = useI18n();
  const activePath = initialPath?.trim() || null;
  const [pathInput, setPathInput] = useState(activePath || "");
  const [isEditingPath, setIsEditingPath] = useState(false);
  const pathInputRef = useRef<HTMLInputElement>(null);
  const rowRef = useRef<HTMLDivElement>(null);
  const breadcrumbSlotRef = useRef<HTMLDivElement>(null);

  const listing = useQuery({
    queryKey: ["filesystem", "directories", activePath],
    queryFn: () => listDirectories(activePath),
    placeholderData: keepPreviousData,
  });

  const parentPath = listing.data?.parent ?? null;

  const parentListing = useQuery({
    queryKey: ["filesystem", "directories", parentPath],
    queryFn: () => listDirectories(parentPath),
    enabled: Boolean(parentPath),
    placeholderData: keepPreviousData,
  });

  const layoutSeed = `${active ? "active" : "inactive"}:${listing.fetchStatus}:${listing.data?.path ?? pathInput}`;

  useEffect(() => {
    if (!listing.data || listing.isPlaceholderData || listing.isFetching) return;
    if (listing.data.path !== activePath) onPathChange(listing.data.path);
  }, [activePath, listing.data, listing.isFetching, listing.isPlaceholderData, onPathChange]);

  useEffect(() => {
    if (!isEditingPath) return;
    pathInputRef.current?.focus();
    pathInputRef.current?.select();
  }, [isEditingPath]);

  const filterDirectories = useMemo(() => {
    const needle = filter.trim().toLocaleLowerCase();
    return (entries: { name: string; path: string }[]) => {
      if (!needle) return entries;
      return entries.filter((directory) => directory.name.toLocaleLowerCase().includes(needle));
    };
  }, [filter]);

  const directories = useMemo(
    () => filterDirectories(listing.data?.directories ?? []),
    [filterDirectories, listing.data?.directories],
  );

  const parentDirectories = useMemo(
    () => filterDirectories(parentListing.data?.directories ?? []),
    [filterDirectories, parentListing.data?.directories],
  );

  const displayedPath = listing.data?.path || pathInput;
  const currentDirectoryName = directoryLabel(displayedPath);

  function navigate(path: string) {
    onFilterChange?.("");
    setPathInput(path);
    setIsEditingPath(false);
    onPathChange(path);
  }

  function startEditingPath() {
    setPathInput(displayedPath);
    setIsEditingPath(true);
  }

  function cancelEditingPath() {
    setPathInput(displayedPath);
    setIsEditingPath(false);
  }

  function submitPath() {
    const path = pathInput.trim();
    if (!path) return;
    navigate(path);
  }

  const crumbs = pathCrumbs(displayedPath);

  return (
    <section className="flex min-h-0 flex-1 flex-col gap-2" aria-label={t("pathBrowserLabel")}>
      <div ref={rowRef} className="flex min-w-0 shrink-0 items-center gap-2">
        <div
          className="flex min-w-0 flex-1 basis-0 items-stretch overflow-hidden rounded-lg border border-border shadow-xs"
          aria-label={t("pathBrowserNavigation")}
        >
          <PathNavButton
            className="rounded-l-lg border-0 border-r border-border"
            aria-label={t("pathBrowserParentDirectory")}
            disabled={!listing.data?.parent || listing.isFetching || isEditingPath}
            onClick={() => listing.data?.parent && navigate(listing.data.parent)}
          >
            <ArrowUpIcon />
          </PathNavButton>

          {isEditingPath ? (
            <Input
              ref={pathInputRef}
              aria-label={t("pathBrowserDirectoryPath")}
              value={pathInput}
              placeholder={t("pathBrowserAbsolutePath")}
              className="min-w-0 flex-1 basis-0 rounded-none border-0 bg-muted shadow-none focus-visible:z-10 focus-visible:ring-0"
              onChange={(event) => setPathInput(event.target.value)}
              onKeyDown={(event) => {
                if (event.key === "Enter") {
                  event.preventDefault();
                  submitPath();
                  return;
                }
                if (event.key === "Escape") {
                  event.preventDefault();
                  cancelEditingPath();
                }
              }}
            />
          ) : (
            <div
              ref={breadcrumbSlotRef}
              className="relative flex min-w-0 flex-1 basis-0 items-center overflow-hidden bg-muted px-3"
            >
              <PathBreadcrumbNav
                slotRef={breadcrumbSlotRef}
                rowRef={rowRef}
                crumbs={crumbs}
                layoutSeed={layoutSeed}
                onNavigate={navigate}
              />
            </div>
          )}

          <PathNavButton
            className="border-0 border-l border-border"
            aria-label={t("refresh")}
            disabled={listing.isFetching || isEditingPath}
            onClick={() => void listing.refetch()}
          >
            {listing.isFetching ? <Spinner /> : <RefreshCwIcon />}
          </PathNavButton>

          {isEditingPath ? (
            <>
              <PathNavButton
                className="border-0 border-l border-border"
                aria-label={t("apply")}
                disabled={!pathInput.trim()}
                onClick={submitPath}
              >
                <CheckIcon />
              </PathNavButton>
              <PathNavButton
                className="rounded-r-lg border-0 border-l border-border"
                aria-label={t("cancel")}
                onClick={cancelEditingPath}
              >
                <XIcon />
              </PathNavButton>
            </>
          ) : (
            <PathNavButton
              className="rounded-r-lg border-0 border-l border-border"
              aria-label={t("pathBrowserEditPath")}
              onClick={startEditingPath}
            >
              <PencilIcon />
            </PathNavButton>
          )}
        </div>
        {pathActions ? <div className="shrink-0">{pathActions}</div> : null}
      </div>

      {listing.isError ? (
        <Alert variant="destructive">
          <AlertTitle>{t("pathBrowserCannotOpenDirectory")}</AlertTitle>
          <AlertDescription>{listing.error.message}</AlertDescription>
        </Alert>
      ) : (
        <div className="flex min-h-0 flex-1 overflow-hidden rounded-md border">
          {parentPath ? (
            <>
              <section
                className="flex min-h-0 w-[min(42%,14rem)] min-w-0 shrink-0 flex-col"
                aria-label={t("pathBrowserParentDirectory")}
              >
                <div className="flex min-w-0 items-center gap-2 border-b bg-muted/40 px-2.5 py-1.5">
                  <span className="shrink-0 text-xs font-medium text-muted-foreground">
                    {t("pathBrowserParent")}
                  </span>
                  <button
                    type="button"
                    className="min-w-0 truncate text-xs hover:underline"
                    onClick={() => navigate(parentPath)}
                  >
                    {directoryLabel(parentPath)}
                  </button>
                </div>
                <ScrollArea className="min-h-0 flex-1">
                  <DirectoryList
                    directories={parentDirectories}
                    currentPath={displayedPath}
                    isLoading={parentListing.isLoading && !parentListing.data}
                    emptyDescription={
                      filter ? t("pathBrowserNoFilterMatches") : t("pathBrowserParentEmpty")
                    }
                    onNavigate={navigate}
                  />
                </ScrollArea>
              </section>
              <Separator orientation="vertical" />
            </>
          ) : null}
          <section
            className="flex min-h-0 min-w-0 flex-1 flex-col"
            aria-label={t("pathBrowserCurrentDirectory")}
          >
            <div className="flex min-w-0 items-center gap-2 border-b bg-muted/40 px-2.5 py-1.5">
              <span className="shrink-0 text-xs font-medium text-muted-foreground">
                {parentPath ? t("pathBrowserCurrent") : t("pathBrowserDirectory")}
              </span>
              <span className="min-w-0 truncate text-xs">{currentDirectoryName}</span>
            </div>
            <ScrollArea className="min-h-0 flex-1">
              <DirectoryList
                directories={directories}
                isLoading={listing.isLoading && !listing.data}
                emptyDescription={
                  filter ? t("pathBrowserNoFilterMatches") : t("pathBrowserNoSubdirectories")
                }
                onNavigate={navigate}
              />
            </ScrollArea>
          </section>
        </div>
      )}
    </section>
  );
}
