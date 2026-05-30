import type { Metadata } from "next";

export const metadata: Metadata = {
  title: "Terms",
  description: "Terms of service for the rbxup website and CLI.",
};

const sections = [
  {
    title: "Relationship to Roblox",
    points: [
      "rbxup is not made by Roblox Corporation.",
      "rbxup is an independent CLI that uses Roblox Open Cloud APIs where available.",
      "Roblox APIs, upload behavior, and OAuth support may change or stop working over time.",
    ],
  },
  {
    title: "User responsibilities",
    points: [
      "Users must only upload or manage assets they own or are authorized to manage.",
      "Users must follow Roblox Terms, Creator Terms, and Community Standards.",
      "Users are responsible for their files, names, descriptions, credentials, API keys, and OAuth sessions.",
    ],
  },
  {
    title: "Warranty and liability",
    points: [
      "rbxup is provided “as is” without warranties of any kind.",
      "The project maintainers are not responsible for asset moderation outcomes, API outages, data loss, or business interruption.",
      "To the fullest extent allowed by law, liability is limited for any indirect, incidental, special, consequential, or punitive damages related to use of rbxup.",
    ],
  },
];

export default function TermsPage() {
  return (
    <div className="space-y-8">
      <section className="panel space-y-4">
        <span className="eyebrow">Terms of Service</span>
        <h1 className="text-3xl font-semibold tracking-tight text-white sm:text-4xl">
          Terms for using rbxup
        </h1>
        <p className="max-w-3xl text-sm leading-7 text-slate-300 sm:text-base">
          These starter terms are intended to cover the public website and the
          CLI release of rbxup.
        </p>
      </section>

      <div className="grid gap-6">
        {sections.map((section) => (
          <section key={section.title} className="panel space-y-4">
            <h2 className="text-xl font-semibold text-white">{section.title}</h2>
            <ul className="space-y-3 text-sm leading-7 text-slate-300 sm:text-base">
              {section.points.map((point) => (
                <li key={point} className="flex gap-3">
                  <span className="mt-2 h-1.5 w-1.5 shrink-0 rounded-full bg-fuchsia-400" />
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
          Questions about these terms can be sent to{" "}
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
