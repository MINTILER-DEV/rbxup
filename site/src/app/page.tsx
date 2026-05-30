import Link from "next/link";

const commands = [
  "rbxup upload ./icon.png --type image",
  "rbxup upload ./icon.png --type image --yield",
  "rbxup upload ./assets --type image --recursive --yield --output jsonl",
];

const features = [
  {
    title: "Open Cloud uploads",
    body: "Use Roblox Open Cloud to submit asset uploads from a terminal-first workflow instead of a browser dashboard.",
  },
  {
    title: "Operation status checks",
    body: "Track upload operations, wait for completion with --yield, and keep automation aware of moderation or processing state.",
  },
  {
    title: "Script-friendly output",
    body: "Keep stdout predictable with job IDs, asset IDs, JSON, JSONL, and map output that works in scripts and CI.",
  },
];

export default function Home() {
  return (
    <div className="space-y-8">
      <section className="hero-grid">
        <div className="panel">
          <div className="space-y-6">
            <div className="space-y-3">
              <span className="eyebrow">CLI for Roblox Open Cloud</span>
              <h1 className="text-4xl font-semibold tracking-[-0.04em] text-white sm:text-5xl lg:text-6xl">
                rbxup
              </h1>
              <p className="max-w-2xl text-lg text-slate-200 sm:text-xl">
                Upload Roblox assets from your terminal.
              </p>
            </div>

            <p className="max-w-2xl text-sm leading-7 text-slate-300 sm:text-base">
              rbxup uses Roblox Open Cloud to upload assets, check upload
              operation status, and produce script-friendly output for humans,
              tools, and CI workflows.
            </p>

            <div className="flex flex-wrap gap-3">
              <a
                className="button-primary"
                href="https://github.com/MINTILER-DEV/rbxup"
                target="_blank"
                rel="noreferrer"
              >
                View on GitHub
              </a>
              <Link className="button-secondary" href="/privacy">
                Privacy Policy
              </Link>
              <Link className="button-secondary" href="/terms">
                Terms of Service
              </Link>
            </div>
          </div>
        </div>

        <aside className="panel terminal-panel">
          <div className="terminal-bar">
            <span />
            <span />
            <span />
          </div>
          <div className="space-y-3">
            <p className="font-mono text-xs uppercase tracking-[0.3em] text-cyan-300/80">
              Example commands
            </p>
            <div className="space-y-3">
              {commands.map((command) => (
                <div key={command} className="terminal-line">
                  <span className="text-cyan-300">$</span>
                  <code>{command}</code>
                </div>
              ))}
            </div>
          </div>
        </aside>
      </section>

      <section className="grid gap-5 lg:grid-cols-[1.3fr_0.7fr]">
        <div className="panel space-y-5">
          <div className="space-y-2">
            <span className="eyebrow">Why rbxup</span>
            <h2 className="text-2xl font-semibold tracking-tight text-white">
              Built for terminal-native asset pipelines
            </h2>
          </div>
          <p className="max-w-3xl text-sm leading-7 text-slate-300 sm:text-base">
            Whether you are pushing one image, polling a single operation, or
            uploading a folder with structured output, rbxup is designed to keep
            terminal workflows readable and automation safe.
          </p>
          <div className="grid gap-4 sm:grid-cols-3">
            {features.map((feature) => (
              <article key={feature.title} className="feature-card">
                <h3 className="text-base font-semibold text-white">
                  {feature.title}
                </h3>
                <p className="text-sm leading-6 text-slate-300">
                  {feature.body}
                </p>
              </article>
            ))}
          </div>
        </div>

        <div className="panel space-y-4">
          <span className="eyebrow">Developer notes</span>
          <ul className="space-y-3 text-sm leading-7 text-slate-300">
            <li className="flex gap-3">
              <span className="mt-2 h-1.5 w-1.5 shrink-0 rounded-full bg-cyan-400" />
              <span>Uses Roblox OAuth or Open Cloud API keys.</span>
            </li>
            <li className="flex gap-3">
              <span className="mt-2 h-1.5 w-1.5 shrink-0 rounded-full bg-cyan-400" />
              <span>Does not ask for Roblox passwords or .ROBLOSECURITY cookies.</span>
            </li>
            <li className="flex gap-3">
              <span className="mt-2 h-1.5 w-1.5 shrink-0 rounded-full bg-cyan-400" />
              <span>Designed so stdout stays machine-friendly and logs stay out of the way.</span>
            </li>
          </ul>
        </div>
      </section>
    </div>
  );
}
