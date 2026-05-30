import Link from "next/link";

const commands = [
  "rbxup upload ./icon.png --type image",
  "rbxup upload ./icon.png --type image --yield",
  "rbxup upload ./assets --type image --recursive --yield --output jsonl",
];

const features = [
  {
    title: "Upload one file",
    body: "Point rbxup at a file, choose the asset type, and upload it through Roblox Open Cloud without opening the browser.",
  },
  {
    title: "Upload a whole folder",
    body: "Send a folder in one run, filter what gets picked up, and keep the results easy to work with afterward.",
  },
  {
    title: "Wait for the final asset ID",
    body: "Use --yield when you want rbxup to wait for Roblox to finish and print the asset ID instead of just the job.",
  },
];

export default function Home() {
  return (
    <div className="space-y-8">
      <section className="hero-grid">
        <div className="panel">
          <div className="space-y-6">
            <div className="space-y-3">
              <span className="eyebrow">Roblox asset uploads from the terminal</span>
              <h1 className="text-4xl font-semibold tracking-[-0.04em] text-white sm:text-5xl lg:text-6xl">
                rbxup
              </h1>
              <p className="max-w-2xl text-lg text-slate-200 sm:text-xl">
                Upload Roblox assets without babysitting the browser.
              </p>
            </div>

            <p className="max-w-2xl text-sm leading-7 text-slate-300 sm:text-base">
              Uploading Roblox assets through the browser gets annoying fast.
              rbxup lets you upload from your terminal with Roblox Open Cloud,
              check operation status, upload one file or a whole folder, and
              wait for the final asset ID with <code>--yield</code>.
            </p>

            <p className="max-w-2xl text-sm leading-7 text-slate-300 sm:text-base">
              Stdout stays clean so scripts can read job IDs, asset IDs, JSON,
              JSONL, or file-to-ID maps without scraping a bunch of extra text.
            </p>

            <div className="flex flex-wrap gap-3">
              <a
                className="button-primary"
                href="https://github.com/MINTILER-DEV/rbxup"
                target="_blank"
                rel="noreferrer"
              >
                View the code
              </a>
              <Link className="button-secondary" href="/privacy">
                Privacy
              </Link>
              <Link className="button-secondary" href="/terms">
                Terms
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
              Uploading Roblox assets from the browser gets annoying fast.
            </h2>
          </div>
          <p className="max-w-3xl text-sm leading-7 text-slate-300 sm:text-base">
            rbxup lets you upload files from your terminal, check the upload
            status, and use the result in scripts without fighting messy
            output.
          </p>
          <p className="max-w-3xl text-sm leading-7 text-slate-300 sm:text-base">
            It can upload one file, upload a whole folder, wait for Roblox to
            finish with <code>--yield</code>, and print clean output like asset
            IDs, JSON, JSONL, or file-to-ID maps.
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
          <span className="eyebrow">A few notes</span>
          <ul className="space-y-3 text-sm leading-7 text-slate-300">
            <li className="flex gap-3">
              <span className="mt-2 h-1.5 w-1.5 shrink-0 rounded-full bg-cyan-400" />
              <span>Works with Roblox OAuth or Open Cloud API keys.</span>
            </li>
            <li className="flex gap-3">
              <span className="mt-2 h-1.5 w-1.5 shrink-0 rounded-full bg-cyan-400" />
              <span>It does not ask for Roblox passwords or .ROBLOSECURITY cookies.</span>
            </li>
            <li className="flex gap-3">
              <span className="mt-2 h-1.5 w-1.5 shrink-0 rounded-full bg-cyan-400" />
              <span>Stdout stays machine-friendly so scripts do not have to fight log noise.</span>
            </li>
          </ul>
        </div>
      </section>
    </div>
  );
}
