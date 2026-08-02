import { useState } from "react";
import {
  ArrowDownWideNarrow,
  ArrowUpNarrowWide,
  FolderOpen,
  LayoutGrid,
  Library as LibraryIcon,
  Music,
  Pencil,
  Play,
  Search,
  Table as TableIcon,
  Trash2,
} from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { Input } from "@/components/ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { useLibrary, usePlatforms } from "@/hooks/useDownloads";
import { useIsPro } from "@/hooks/useActivation";
import { EditorDialog } from "@/components/EditorDialog";
import { ProBadge, UpgradeProDialog } from "@/components/UpgradeProDialog";
import { formatBytes, formatDate, formatDuration } from "@/lib/format";
import { api } from "@/services/api";
import { openItem } from "@/lib/toast";
import { useQueryClient } from "@tanstack/react-query";
import type { Download, LibrarySort } from "@/types";

export function LibraryPage() {
  const queryClient = useQueryClient();
  const [query, setQuery] = useState("");
  const [platform, setPlatform] = useState<string>("all");
  const [kind, setKind] = useState<"all" | "video" | "audio">("all");
  const [sort, setSort] = useState<LibrarySort>("date");
  const [descending, setDescending] = useState(true);
  const [view, setView] = useState<"cards" | "table">("cards");
  const [toDelete, setToDelete] = useState<Download | null>(null);
  const [deleteFile, setDeleteFile] = useState(false);
  const [toEdit, setToEdit] = useState<Download | null>(null);
  const [showUpgrade, setShowUpgrade] = useState(false);
  const isPro = useIsPro();

  const requestEdit = (item: Download) => {
    if (isPro) setToEdit(item);
    else setShowUpgrade(true);
  };

  const { data: items, isLoading } = useLibrary({
    query,
    platform: platform === "all" ? null : platform,
    kind,
    sort,
    descending,
    limit: null,
  });
  const { data: platforms } = usePlatforms();

  const confirmDelete = async () => {
    if (!toDelete) return;
    await api.removeDownload(toDelete.id, deleteFile);
    setToDelete(null);
    setDeleteFile(false);
    queryClient.invalidateQueries({ queryKey: ["library"] });
    queryClient.invalidateQueries({ queryKey: ["downloads"] });
  };

  return (
    <div className="flex h-full flex-col gap-4 p-6">
      <div className="flex items-center justify-between">
        <h1 className="text-xl font-semibold">Library</h1>
        <div className="flex rounded-md border">
          <Button
            variant={view === "cards" ? "secondary" : "ghost"}
            size="icon-sm"
            title="Card view"
            onClick={() => setView("cards")}
          >
            <LayoutGrid />
          </Button>
          <Button
            variant={view === "table" ? "secondary" : "ghost"}
            size="icon-sm"
            title="Table view"
            onClick={() => setView("table")}
          >
            <TableIcon />
          </Button>
        </div>
      </div>

      <div className="flex flex-wrap items-center gap-2">
        <div className="relative min-w-64 flex-1">
          <Search className="absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
          <Input
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder="Search title, filename, website or extension…"
            className="pl-9"
          />
        </div>
        <Select value={platform} onValueChange={setPlatform}>
          <SelectTrigger className="w-36">
            <SelectValue placeholder="Platform" />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="all">All platforms</SelectItem>
            {(platforms ?? []).map((p) => (
              <SelectItem key={p} value={p}>
                {p}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
        <Select value={kind} onValueChange={(v) => setKind(v as typeof kind)}>
          <SelectTrigger className="w-28">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="all">All types</SelectItem>
            <SelectItem value="video">Video</SelectItem>
            <SelectItem value="audio">Audio</SelectItem>
          </SelectContent>
        </Select>
        <Select value={sort} onValueChange={(v) => setSort(v as LibrarySort)}>
          <SelectTrigger className="w-32">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="date">Date</SelectItem>
            <SelectItem value="title">Title</SelectItem>
            <SelectItem value="size">Size</SelectItem>
            <SelectItem value="duration">Duration</SelectItem>
          </SelectContent>
        </Select>
        <Button
          variant="outline"
          size="icon"
          title={descending ? "Descending" : "Ascending"}
          onClick={() => setDescending((d) => !d)}
        >
          {descending ? <ArrowDownWideNarrow /> : <ArrowUpNarrowWide />}
        </Button>
      </div>

      <div className="flex-1 overflow-y-auto">
        {!isLoading && (items ?? []).length === 0 ? (
          <div className="flex h-full flex-col items-center justify-center gap-3 text-muted-foreground">
            <LibraryIcon className="h-10 w-10" />
            <p className="text-sm">
              {query || platform !== "all" || kind !== "all"
                ? "Nothing matches your filters."
                : "Completed downloads will appear here."}
            </p>
          </div>
        ) : view === "cards" ? (
          <div className="grid grid-cols-[repeat(auto-fill,minmax(220px,1fr))] gap-4 pb-4">
            {(items ?? []).map((item) => (
              <LibraryCard
                key={item.id}
                item={item}
                onDelete={() => setToDelete(item)}
                onEdit={() => requestEdit(item)}
              />
            ))}
          </div>
        ) : (
          <table className="w-full text-sm">
            <thead className="sticky top-0 bg-background text-left text-xs text-muted-foreground">
              <tr className="border-b">
                <th className="py-2 pr-3 font-medium">Title</th>
                <th className="py-2 pr-3 font-medium">Platform</th>
                <th className="py-2 pr-3 font-medium">Format</th>
                <th className="py-2 pr-3 font-medium">Size</th>
                <th className="py-2 pr-3 font-medium">Date</th>
                <th className="py-2 font-medium"></th>
              </tr>
            </thead>
            <tbody>
              {(items ?? []).map((item) => (
                <tr key={item.id} className="border-b hover:bg-accent/40">
                  <td
                    className="max-w-md cursor-pointer truncate py-2 pr-3 font-medium"
                    onClick={() => openItem(api.openFile, item.id)}
                    title={item.title}
                  >
                    {item.title}
                  </td>
                  <td className="py-2 pr-3">
                    <Badge variant="secondary">{item.platform}</Badge>
                  </td>
                  <td className="py-2 pr-3 text-muted-foreground">
                    {item.format.toUpperCase()}
                    {item.resolution ? ` · ${item.resolution}` : ""}
                  </td>
                  <td className="py-2 pr-3 text-muted-foreground">
                    {formatBytes(item.file_size)}
                  </td>
                  <td className="py-2 pr-3 text-muted-foreground">
                    {formatDate(item.completed_at)}
                  </td>
                  <td className="py-2 text-right">
                    <div className="flex justify-end gap-1">
                      <Button size="icon-sm" variant="ghost" title="Open" onClick={() => openItem(api.openFile, item.id)}>
                        <Play />
                      </Button>
                      <Button size="icon-sm" variant="ghost" title="Edit (Pro)" onClick={() => requestEdit(item)}>
                        <Pencil />
                      </Button>
                      <Button size="icon-sm" variant="ghost" title="Show in folder" onClick={() => api.showInFolder(item.id)}>
                        <FolderOpen />
                      </Button>
                      <Button
                        size="icon-sm"
                        variant="ghost"
                        title="Delete"
                        className="hover:text-destructive"
                        onClick={() => setToDelete(item)}
                      >
                        <Trash2 />
                      </Button>
                    </div>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </div>

      <Dialog
        open={toDelete !== null}
        onOpenChange={(open) => {
          if (!open) {
            setToDelete(null);
            setDeleteFile(false);
          }
        }}
      >
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Delete from library?</DialogTitle>
            <DialogDescription>“{toDelete?.title}” will be removed from your library.</DialogDescription>
          </DialogHeader>
          <label className="flex cursor-pointer items-center gap-2 text-sm">
            <input
              type="checkbox"
              checked={deleteFile}
              onChange={(e) => setDeleteFile(e.target.checked)}
            />
            Also delete the file from disk
          </label>
          <DialogFooter>
            <Button variant="ghost" onClick={() => setToDelete(null)}>
              Cancel
            </Button>
            <Button variant="destructive" onClick={confirmDelete}>
              Delete
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {toEdit && <EditorDialog item={toEdit} onClose={() => setToEdit(null)} />}
      <UpgradeProDialog
        open={showUpgrade}
        onClose={() => setShowUpgrade(false)}
        feature="Editing"
      />
    </div>
  );
}

function LibraryCard({
  item,
  onDelete,
  onEdit,
}: {
  item: Download;
  onDelete: () => void;
  onEdit: () => void;
}) {
  return (
    <div className="group overflow-hidden rounded-xl border bg-card transition-colors hover:border-primary/40">
      <div
        className="relative aspect-video cursor-pointer bg-secondary"
        onClick={() => openItem(api.openFile, item.id)}
      >
        {item.thumbnail ? (
          <img
            src={item.thumbnail}
            alt=""
            className="h-full w-full object-cover"
            loading="lazy"
          />
        ) : (
          <div className="flex h-full items-center justify-center text-muted-foreground">
            <Music className="h-8 w-8" />
          </div>
        )}
        {item.duration != null && (
          <span className="absolute bottom-1.5 right-1.5 rounded bg-black/75 px-1.5 py-0.5 text-[11px] font-medium text-white">
            {formatDuration(item.duration)}
          </span>
        )}
        {item.resolution && (
          <span className="absolute left-1.5 top-1.5 rounded bg-black/75 px-1.5 py-0.5 text-[11px] font-medium text-white">
            {item.resolution}
          </span>
        )}
        <div className="absolute inset-0 flex items-center justify-center bg-black/40 opacity-0 transition-opacity group-hover:opacity-100">
          <Play className="h-8 w-8 text-white" />
        </div>
      </div>
      <div className="flex items-start justify-between gap-1 p-3">
        <div className="min-w-0">
          <p className="truncate text-sm font-medium" title={item.title}>
            {item.title}
          </p>
          <p className="mt-0.5 text-xs text-muted-foreground">
            {item.platform} · {formatBytes(item.file_size)}
          </p>
        </div>
        <DropdownMenu>
          <DropdownMenuTrigger asChild>
            <Button size="icon-sm" variant="ghost" className="shrink-0">
              <span className="text-lg leading-none">⋯</span>
            </Button>
          </DropdownMenuTrigger>
          <DropdownMenuContent align="end">
            <DropdownMenuItem onClick={() => openItem(api.openFile, item.id)}>
              <Play /> Open
            </DropdownMenuItem>
            <DropdownMenuItem onClick={() => api.showInFolder(item.id)}>
              <FolderOpen /> Show in folder
            </DropdownMenuItem>
            <DropdownMenuItem onClick={onEdit}>
              <Pencil /> Edit <ProBadge className="ml-auto" />
            </DropdownMenuItem>
            <DropdownMenuSeparator />
            <DropdownMenuItem className="text-destructive" onClick={onDelete}>
              <Trash2 /> Delete
            </DropdownMenuItem>
          </DropdownMenuContent>
        </DropdownMenu>
      </div>
    </div>
  );
}
