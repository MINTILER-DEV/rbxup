import type { Metadata } from "next";

export const metadata: Metadata = {
  title: "Privacy",
  description: "Privacy policy for the rbxup website and CLI.",
};

const sections = [
  {
    title: "What rbxup does not collect",
    points: [
      "rbxup does not collect Roblox passwords.",
      "rbxup does not ask for .ROBLOSECURITY cookies.",
      "rbxup does not sell user data or share it with advertisers.",
    ],
  },
  {
    title: "Authentication and credentials",
    points: [
      "rbxup may use Roblox OAuth or Roblox Open Cloud API keys to authenticate requests.",
      "rbxup may request asset:read and asset:write scopes when the OAuth flow is used.",
      "Tokens and local configuration may be stored on the user’s device so the CLI can keep working between sessions.",
    ],
  },
  {
    title: "Files and uploads",
    points: [
      "Files are uploaded only when the user explicitly runs an upload command.",
      "rbxup uses Roblox Open Cloud to submit upload requests and to check upload operation status.",
      "The website itself does not include login, token storage, analytics, or a backend service.",
    ],
  },
];

export default function PrivacyPage() {
  return (
    <div className="space-y-8">
      <section className="panel space-y-4">
        <span className="eyebrow">Privacy Policy</span>
        <h1 className="text-3xl font-semibold tracking-tight text-white sm:text-4xl">
          Privacy policy for rbxup
        </h1>
        <p className="max-w-3xl text-sm leading-7 text-slate-300 sm:text-base">
          This starter privacy policy describes how rbxup handles credentials,
          uploads, and local storage for the CLI and this informational website.
        </p>
      </section>

      <div className="grid gap-6">
        {sections.map((section) => (
          <section key={section.title} className="panel space-y-4">
            <h2 className="text-xl font-semibold text-white">{section.title}</h2>
            <ul className="space-y-3 text-sm leading-7 text-slate-300 sm:text-base">
              {section.points.map((point) => (
                <li key={point} className="flex gap-3">
                  <span className="mt-2 h-1.5 w-1.5 shrink-0 rounded-full bg-cyan-400" />
                  <span>{point}</span>
                </li>
              ))}
            </ul>
          </section>
        ))}
      </div>

      <section className="panel space-y-4">
        <h2 className="text-xl font-semibold text-white">Contact</h2>
        <p className="text-sm leading-7 text-slate-300 sm:text-base">
          Questions about privacy can be sent to{" "}
          <a
            className="text-cyan-300 transition hover:text-cyan-200"
            href="mailto:mintilerant@gmail.com"
          >
            mintilerant@gmail.com
          </a>
          .
        </p>
      </section>
    </div>
  );
}
