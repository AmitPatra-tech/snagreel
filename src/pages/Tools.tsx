import { useState } from "react";
import { open as openFileDialog } from "@tauri-apps/plugin-dialog";
import { Languages, Music, Wrench } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { TranscribeDialog } from "@/components/TranscribeDialog";
import { ExtractAudioDialog } from "@/components/ExtractAudioDialog";
import { ProBadge, UpgradeProDialog } from "@/components/UpgradeProDialog";
import { useIsPro } from "@/hooks/useActivation";

const MEDIA_EXTENSIONS = [
  "mp4", "mkv", "webm", "mov", "avi", "flv", "m4v", "ts", "mpg", "mpeg", "wmv",
  "mp3", "m4a", "wav", "aac", "flac", "ogg", "opus", "wma",
];

function baseName(path: string): string {
  return path.split(/[\\/]/).pop() ?? path;
}

type LocalFile = { path: string; title: string };

export function ToolsPage() {
  const isPro = useIsPro();
  const [transcribeFile, setTranscribeFile] = useState<LocalFile | null>(null);
  const [extractFile, setExtractFile] = useState<LocalFile | null>(null);
  const [showUpgrade, setShowUpgrade] = useState(false);

  const pickFile = async (set: (f: LocalFile) => void, title: string) => {
    if (!isPro) {
      setShowUpgrade(true);
      return;
    }
    const picked = await openFileDialog({
      multiple: false,
      directory: false,
      title,
      filters: [
        { name: "Media files", extensions: MEDIA_EXTENSIONS },
        { name: "All files", extensions: ["*"] },
      ],
    });
    if (typeof picked === "string") set({ path: picked, title: baseName(picked) });
  };

  return (
    <div className="mx-auto flex h-full max-w-3xl flex-col gap-6 overflow-y-auto p-8">
      <div>
        <h1 className="flex items-center gap-2 text-xl font-semibold">
          <Wrench className="h-5 w-5" /> Tools
        </h1>
        <p className="mt-1 text-sm text-muted-foreground">
          Work with a video or audio file on your computer — no download needed.
        </p>
      </div>

      <ToolCard
        icon={<Languages className="h-6 w-6" />}
        title="Transcribe to text"
        description="Turn speech in any video or audio file into text and subtitles, in 90+ languages. Runs on your device."
        action="Choose a file"
        onClick={() => pickFile(setTranscribeFile, "Choose a file to transcribe")}
      />

      <ToolCard
        icon={<Music className="h-6 w-6" />}
        title="Extract audio"
        description="Pull the audio out of any video — into MP3, M4A, WAV, AAC, FLAC or OGG. Pick several formats at once."
        action="Choose a video"
        onClick={() => pickFile(setExtractFile, "Choose a video to extract audio from")}
      />

      {transcribeFile && (
        <TranscribeDialog
          title={transcribeFile.title}
          inputPath={transcribeFile.path}
          onClose={() => setTranscribeFile(null)}
        />
      )}
      {extractFile && (
        <ExtractAudioDialog
          title={extractFile.title}
          inputPath={extractFile.path}
          onClose={() => setExtractFile(null)}
        />
      )}
      <UpgradeProDialog
        open={showUpgrade}
        onClose={() => setShowUpgrade(false)}
        feature="Tools"
      />
    </div>
  );
}

function ToolCard({
  icon,
  title,
  description,
  action,
  onClick,
}: {
  icon: React.ReactNode;
  title: string;
  description: string;
  action: string;
  onClick: () => void;
}) {
  return (
    <Card>
      <CardContent className="flex flex-col gap-4 p-5 sm:flex-row sm:items-center">
        <div className="flex h-12 w-12 shrink-0 items-center justify-center rounded-xl bg-primary/10 text-primary">
          {icon}
        </div>
        <div className="min-w-0 flex-1">
          <p className="flex items-center gap-2 font-medium">
            {title} <ProBadge />
          </p>
          <p className="mt-0.5 text-sm text-muted-foreground">{description}</p>
        </div>
        <Button className="shrink-0" onClick={onClick}>
          {action}
        </Button>
      </CardContent>
    </Card>
  );
}
