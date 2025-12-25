import { SectionHeader } from "./shared";

export function AboutSection() {
  return (
    <section>
      <SectionHeader>About</SectionHeader>
      <div className="flex items-start gap-4">
        <div
          className="w-14 h-14 rounded-2xl flex items-center justify-center flex-shrink-0"
          style={{ background: "linear-gradient(135deg, var(--color-accent), #5856d6)" }}
        >
          <span className="text-2xl font-bold text-white">G</span>
        </div>
        <div>
          <h3 className="font-semibold" style={{ color: "var(--color-text-primary)" }}>
            gibb.eri.sh
          </h3>
          <p className="text-sm" style={{ color: "var(--color-text-tertiary)" }}>
            Version 0.1.0
          </p>
          <p className="text-sm mt-2" style={{ color: "var(--color-text-tertiary)" }}>
            Local, private speech-to-text. All processing happens on your device.
          </p>
          <a
            href="https://github.com/mpuig/gibb.eri.sh"
            target="_blank"
            rel="noopener noreferrer"
            className="text-sm mt-1 inline-block"
            style={{ color: "var(--color-accent)" }}
          >
            github.com/mpuig/gibb.eri.sh
          </a>
        </div>
      </div>
    </section>
  );
}
