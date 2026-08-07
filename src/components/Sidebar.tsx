import { NavLink } from "react-router-dom";
import { Download, Home, Library, Settings, Wrench } from "lucide-react";
import { cn } from "@/lib/utils";
import { useDownloads } from "@/hooks/useDownloads";
import { useAppVersion } from "@/hooks/useAppVersion";
import logo from "@/assets/logo.png";

const links = [
  { to: "/", label: "Home", icon: Home },
  { to: "/downloads", label: "Downloads", icon: Download },
  { to: "/library", label: "Library", icon: Library },
  { to: "/tools", label: "Tools", icon: Wrench },
  { to: "/settings", label: "Settings", icon: Settings },
];

export function Sidebar() {
  const { data: downloads } = useDownloads();
  const version = useAppVersion();
  const activeCount =
    downloads?.filter((d) => d.status === "downloading" || d.status === "queued")
      .length ?? 0;

  return (
    <aside className="flex h-full w-56 shrink-0 flex-col border-r bg-card/50">
      <div className="flex items-center gap-2.5 px-4 py-5">
        <img src={logo} alt="Snagreel by HutZon" className="h-8 w-8 rounded-md" />
        <div className="leading-tight">
          <div className="text-sm font-semibold">Snagreel</div>
          <div className="text-xs text-muted-foreground">by HutZon</div>
        </div>
      </div>
      <nav className="flex flex-1 flex-col gap-1 px-3">
        {links.map(({ to, label, icon: Icon }) => (
          <NavLink
            key={to}
            to={to}
            end={to === "/"}
            className={({ isActive }) =>
              cn(
                "flex items-center gap-3 rounded-md px-3 py-2 text-sm font-medium transition-colors",
                isActive
                  ? "bg-primary/15 text-primary"
                  : "text-muted-foreground hover:bg-accent hover:text-foreground",
              )
            }
          >
            <Icon className="h-4 w-4" />
            <span className="flex-1">{label}</span>
            {label === "Downloads" && activeCount > 0 && (
              <span className="rounded-full bg-primary px-1.5 py-0.5 text-[10px] font-semibold text-primary-foreground">
                {activeCount}
              </span>
            )}
          </NavLink>
        ))}
      </nav>
      <div className="px-4 py-3 text-[11px] text-muted-foreground">
        {version && `v${version}`}
      </div>
    </aside>
  );
}
