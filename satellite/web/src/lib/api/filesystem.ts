// Filesystem browsing for the panel-local path browser component.

import { appApi } from "./client";

export type DirectoryEntry = {
  name: string;
  path: string;
};

export type DirectoryListing = {
  path: string;
  parent: string | null;
  directories: DirectoryEntry[];
};

export function listDirectories(path?: string | null) {
  const query = path ? `?path=${encodeURIComponent(path)}` : "";
  return appApi<DirectoryListing>(`/filesystem/directories${query}`);
}
