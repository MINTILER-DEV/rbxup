import type { Metadata } from "next";
import Link from "next/link";
import { IBM_Plex_Mono, Space_Grotesk } from "next/font/google";
import "./globals.css";

const spaceGrotesk = Space_Grotesk({
  variable: "--font-space-grotesk",
  subsets: ["latin"],
});

const plexMono = IBM_Plex_Mono({
  variable: "--font-plex-mono",
  subsets: ["latin"],
  weight: ["400", "500"],
});

export const metadata: Metadata = {
  title: {
    default: "rbxup",
    template: "%s | rbxup",
  },
  description: "Upload Roblox assets from your terminal.",
};

export default function RootLayout({
  children,
}: Readonly<{
  children: React.ReactNode;
}>) {
  return (
    <html
      lang="en"
      className={`${spaceGrotesk.variable} ${plexMono.variable} h-full`}
    >
      <body className="min-h-full bg-[var(--background)] text-[var(--foreground)] antialiased">
        <div className="site-shell">
          <div className="site-backdrop" />
          <header className="site-header">
            <div className="mx-auto flex w-full max-w-6xl items-center justify-between gap-4 px-5 py-5 sm:px-8">
              <Link className="brand-mark" href="/">
                <span className="brand-dot" />
                <span>rbxup</span>
              </Link>
              <nav className="flex flex-wrap items-center gap-2 text-sm text-slate-300">
                <Link className="nav-link" href="/">
                  Home
                </Link>
                <Link className="nav-link" href="/privacy">
                  Privacy
                </Link>
                <Link className="nav-link" href="/terms">
                  Terms
                </Link>
                <a
                  className="nav-link"
                  href="https://github.com/MINTILER-DEV/rbxup"
                  target="_blank"
                  rel="noreferrer"
                >
                  GitHub
                </a>
              </nav>
            </div>
          </header>

          <main className="mx-auto flex w-full max-w-6xl flex-1 px-5 pb-12 pt-6 sm:px-8 sm:pb-16 sm:pt-10">
            <div className="w-full">{children}</div>
          </main>

          <footer className="mx-auto w-full max-w-6xl px-5 pb-10 pt-2 text-sm text-slate-400 sm:px-8">
            rbxup is not affiliated with Roblox Corporation.
          </footer>
        </div>
      </body>
    </html>
  );
}
