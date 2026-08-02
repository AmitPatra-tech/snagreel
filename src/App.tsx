import { useEffect } from "react";
import { HashRouter, Route, Routes } from "react-router-dom";
import { QueryClient, QueryClientProvider, useQueryClient } from "@tanstack/react-query";
import { Sidebar } from "@/components/Sidebar";
import { HomePage } from "@/pages/Home";
import { DownloadsPage } from "@/pages/Downloads";
import { LibraryPage } from "@/pages/Library";
import { ToolsPage } from "@/pages/Tools";
import { SettingsPage } from "@/pages/Settings";
import { Toaster } from "@/components/Toaster";
import { UpdatePrompt } from "@/components/UpdatePrompt";
import { attachDownloadListeners } from "@/services/events";
import { useSettings } from "@/hooks/useSettings";

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      retry: 1,
      refetchOnWindowFocus: false,
    },
  },
});

function ThemeWatcher() {
  const { data: settings } = useSettings();

  useEffect(() => {
    const theme = settings?.theme ?? "dark";
    const root = document.documentElement;
    const apply = (dark: boolean) => root.classList.toggle("dark", dark);

    if (theme === "system") {
      const media = window.matchMedia("(prefers-color-scheme: dark)");
      apply(media.matches);
      const onChange = (e: MediaQueryListEvent) => apply(e.matches);
      media.addEventListener("change", onChange);
      return () => media.removeEventListener("change", onChange);
    }
    apply(theme === "dark");
  }, [settings?.theme]);

  return null;
}

function EventBridge() {
  const client = useQueryClient();
  useEffect(() => attachDownloadListeners(client), [client]);
  return null;
}

export default function App() {
  return (
    <QueryClientProvider client={queryClient}>
      <ThemeWatcher />
      <EventBridge />
      <Toaster />
      <UpdatePrompt />
      <HashRouter>
        <div className="flex h-full">
          <Sidebar />
          <main className="min-w-0 flex-1">
            <Routes>
              <Route path="/" element={<HomePage />} />
              <Route path="/downloads" element={<DownloadsPage />} />
              <Route path="/library" element={<LibraryPage />} />
              <Route path="/tools" element={<ToolsPage />} />
              <Route path="/settings" element={<SettingsPage />} />
            </Routes>
          </main>
        </div>
      </HashRouter>
    </QueryClientProvider>
  );
}
