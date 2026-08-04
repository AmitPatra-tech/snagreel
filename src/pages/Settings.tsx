import { useForm, Controller } from "react-hook-form";
import { zodResolver } from "@hookform/resolvers/zod";
import { z } from "zod";
import { open } from "@tauri-apps/plugin-dialog";
import { FolderOpen, Loader2 } from "lucide-react";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Switch } from "@/components/ui/switch";
import { Sparkles } from "lucide-react";
import { useSettings, useUpdateSettings } from "@/hooks/useSettings";
import { ProActivation } from "@/components/ProActivation";

const schema = z.object({
  download_path: z.string().min(1, "Choose a download folder"),
  theme: z.enum(["dark", "light", "system"]),
  language: z.string(),
  max_concurrent_downloads: z.coerce.number().int().min(1).max(10),
  auto_update: z.boolean(),
  notifications: z.boolean(),
  filename_template: z.string().min(1, "Template cannot be empty"),
  organize_by_platform: z.boolean(),
  cookies_browser: z.string(),
});

type FormValues = z.infer<typeof schema>;

export function SettingsPage() {
  const { data: settings } = useSettings();
  const updateSettings = useUpdateSettings();

  const form = useForm<FormValues>({
    resolver: zodResolver(schema),
    values: settings,
  });

  const { register, handleSubmit, control, setValue, watch, formState } = form;
  const downloadPath = watch("download_path");

  const browseFolder = async () => {
    const dir = await open({
      directory: true,
      defaultPath: downloadPath || undefined,
      title: "Choose download folder",
    });
    if (typeof dir === "string") {
      setValue("download_path", dir, { shouldDirty: true });
    }
  };

  const onSubmit = (values: FormValues) => {
    updateSettings.mutate({ ...values, theme: values.theme });
  };

  if (!settings) {
    return (
      <div className="flex h-full items-center justify-center text-muted-foreground">
        <Loader2 className="h-6 w-6 animate-spin" />
      </div>
    );
  }

  return (
    <div className="mx-auto h-full max-w-2xl overflow-y-auto p-6">
    <form onSubmit={handleSubmit(onSubmit)} className="space-y-6">
      <div className="flex items-center justify-between">
        <h1 className="text-xl font-semibold">Settings</h1>
        <Button type="submit" disabled={updateSettings.isPending || !formState.isDirty}>
          {updateSettings.isPending ? (
            <>
              <Loader2 className="animate-spin" /> Saving…
            </>
          ) : updateSettings.isSuccess && !formState.isDirty ? (
            "Saved"
          ) : (
            "Save changes"
          )}
        </Button>
      </div>

      <Card>
        <CardHeader>
          <CardTitle>Downloads</CardTitle>
          <CardDescription>Where and how files are downloaded.</CardDescription>
        </CardHeader>
        <CardContent className="space-y-5">
          <div className="space-y-2">
            <Label>Download folder</Label>
            <div className="flex gap-2">
              <Input {...register("download_path")} readOnly className="flex-1" />
              <Button type="button" variant="outline" onClick={browseFolder}>
                <FolderOpen /> Browse
              </Button>
            </div>
            {formState.errors.download_path && (
              <p className="text-xs text-destructive">
                {formState.errors.download_path.message}
              </p>
            )}
          </div>

          <div className="flex items-center justify-between">
            <div>
              <Label>Organize by platform</Label>
              <p className="text-xs text-muted-foreground">
                Save into per-site subfolders like YouTube/, TikTok/, Twitter/.
              </p>
            </div>
            <Controller
              control={control}
              name="organize_by_platform"
              render={({ field }) => (
                <Switch checked={field.value} onCheckedChange={field.onChange} />
              )}
            />
          </div>

          <div className="space-y-2">
            <Label>Concurrent downloads</Label>
            <Controller
              control={control}
              name="max_concurrent_downloads"
              render={({ field }) => (
                <Select
                  value={String(field.value)}
                  onValueChange={(v) => field.onChange(Number(v))}
                >
                  <SelectTrigger className="w-32">
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    {[1, 2, 3, 4, 5, 6, 8, 10].map((n) => (
                      <SelectItem key={n} value={String(n)}>
                        {n}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              )}
            />
            <p className="text-xs text-muted-foreground">
              How many downloads run at the same time.
            </p>
          </div>

          <div className="space-y-2">
            <Label>Filename template</Label>
            <Input {...register("filename_template")} spellCheck={false} />
            <p className="text-xs text-muted-foreground">
              yt-dlp output template, e.g.{" "}
              <code className="rounded bg-secondary px-1">
                %(title)s.%(ext)s
              </code>{" "}
              or{" "}
              <code className="rounded bg-secondary px-1">
                %(uploader)s - %(title)s.%(ext)s
              </code>
            </p>
          </div>

          <div className="space-y-2">
            <Label>Sign-in cookies from browser</Label>
            <Controller
              control={control}
              name="cookies_browser"
              render={({ field }) => (
                <Select value={field.value || "none"} onValueChange={field.onChange}>
                  <SelectTrigger className="w-48">
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="none">Don't use cookies</SelectItem>
                    <SelectItem value="chrome">Chrome</SelectItem>
                    <SelectItem value="edge">Edge</SelectItem>
                    <SelectItem value="firefox">Firefox</SelectItem>
                    <SelectItem value="brave">Brave</SelectItem>
                    <SelectItem value="opera">Opera</SelectItem>
                    <SelectItem value="vivaldi">Vivaldi</SelectItem>
                    <SelectItem value="chromium">Chromium</SelectItem>
                  </SelectContent>
                </Select>
              )}
            />
            <p className="text-xs text-muted-foreground">
              Lets Snagreel download videos from sites that require you to be logged in, by
              reusing your browser's session. If a download fails to read cookies, close that
              browser and try again. (DRM-protected services still can't be downloaded.)
            </p>
          </div>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>Appearance</CardTitle>
        </CardHeader>
        <CardContent className="space-y-5">
          <div className="space-y-2">
            <Label>Theme</Label>
            <Controller
              control={control}
              name="theme"
              render={({ field }) => (
                <Select value={field.value} onValueChange={field.onChange}>
                  <SelectTrigger className="w-40">
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="dark">Dark</SelectItem>
                    <SelectItem value="light">Light</SelectItem>
                    <SelectItem value="system">System</SelectItem>
                  </SelectContent>
                </Select>
              )}
            />
          </div>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>General</CardTitle>
        </CardHeader>
        <CardContent className="space-y-5">
          <div className="flex items-center justify-between">
            <div>
              <Label>Notifications</Label>
              <p className="text-xs text-muted-foreground">
                Notify when downloads finish or fail.
              </p>
            </div>
            <Controller
              control={control}
              name="notifications"
              render={({ field }) => (
                <Switch checked={field.value} onCheckedChange={field.onChange} />
              )}
            />
          </div>

          <div className="flex items-center justify-between">
            <div>
              <Label>Automatic updates</Label>
              <p className="text-xs text-muted-foreground">
                Keep the app up to date automatically.
              </p>
            </div>
            <Controller
              control={control}
              name="auto_update"
              render={({ field }) => (
                <Switch checked={field.value} onCheckedChange={field.onChange} />
              )}
            />
          </div>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>About</CardTitle>
        </CardHeader>
        <CardContent className="text-sm text-muted-foreground">
          <p>Snagreel v1.0.0 — by HutZon</p>
          <p className="mt-1">
            The fastest, cleanest way to download and manage your media.
          </p>
        </CardContent>
      </Card>
    </form>

      <Card className="mt-6">
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <Sparkles className="h-5 w-5 text-primary" /> Pro
          </CardTitle>
          <CardDescription>
            Unlock the video &amp; audio editor, clip downloads and audio extraction.
          </CardDescription>
        </CardHeader>
        <CardContent>
          <ProActivation />
        </CardContent>
      </Card>
    </div>
  );
}
