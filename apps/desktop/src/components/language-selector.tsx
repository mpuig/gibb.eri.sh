import { useState, useRef, useEffect, useMemo } from "react";
import { useStt, type Language } from "../hooks/use-stt";

const ALL_LANGUAGES: { code: Language; label: string }[] = [
  { code: "auto", label: "Auto" },
  { code: "en", label: "EN" },
  { code: "es", label: "ES" },
  { code: "ca", label: "CA" },
];

export function LanguageSelector() {
  const { language, setLanguage, loadingModel, isCurrentModelMultilingual, currentModelSupportedLanguages } = useStt();
  const [isOpen, setIsOpen] = useState(false);
  const menuRef = useRef<HTMLDivElement>(null);

  // Determine available languages based on model capabilities
  const availableLanguages = useMemo(() => {
    if (isCurrentModelMultilingual) {
      // Multilingual model: show all languages including auto-detect
      return ALL_LANGUAGES;
    }
    // Language-specific model: show only supported languages
    return ALL_LANGUAGES.filter((lang) =>
      currentModelSupportedLanguages.includes(lang.code)
    );
  }, [isCurrentModelMultilingual, currentModelSupportedLanguages]);

  const currentLang = availableLanguages.find((l) => l.code === language) || availableLanguages[0];
  const isLoading = loadingModel !== null;

  // Auto-switch language when model doesn't support current selection
  useEffect(() => {
    if (availableLanguages.length > 0 && !availableLanguages.find((l) => l.code === language)) {
      const defaultLang = availableLanguages[0].code;
      setLanguage(defaultLang).catch((err) => {
        console.error(`Failed to auto-switch to ${defaultLang}:`, err);
      });
    }
  }, [availableLanguages, language, setLanguage]);

  useEffect(() => {
    const handleClickOutside = (event: MouseEvent) => {
      if (menuRef.current && !menuRef.current.contains(event.target as Node)) {
        setIsOpen(false);
      }
    };

    if (isOpen) {
      document.addEventListener("mousedown", handleClickOutside);
    }

    return () => {
      document.removeEventListener("mousedown", handleClickOutside);
    };
  }, [isOpen]);

  const handleSelect = async (lang: Language) => {
    setIsOpen(false);
    if (lang !== language) {
      try {
        await setLanguage(lang);
      } catch (err) {
        console.error("Failed to set language:", err);
      }
    }
  };

  return (
    <div className="relative" ref={menuRef}>
      <button
        onClick={() => setIsOpen(!isOpen)}
        disabled={isLoading}
        className="h-8 px-2 rounded-md text-xs font-medium transition-all duration-150 flex items-center gap-1"
        style={{
          background: "var(--color-bg-tertiary)",
          color: isLoading ? "var(--color-text-quaternary)" : "var(--color-text-secondary)",
          cursor: isLoading ? "not-allowed" : "pointer",
          opacity: isLoading ? 0.5 : 1,
        }}
        title="Select transcription language"
      >
        <span>{isLoading ? "..." : currentLang.label}</span>
        <svg
          className="w-3 h-3"
          viewBox="0 0 20 20"
          fill="currentColor"
          style={{
            transform: isOpen ? "rotate(180deg)" : "rotate(0deg)",
            transition: "transform 150ms",
          }}
        >
          <path
            fillRule="evenodd"
            d="M5.293 7.293a1 1 0 011.414 0L10 10.586l3.293-3.293a1 1 0 111.414 1.414l-4 4a1 1 0 01-1.414 0l-4-4a1 1 0 010-1.414z"
            clipRule="evenodd"
          />
        </svg>
      </button>

      {isOpen && (
        <div
          className="absolute top-full mt-1 left-0 rounded-md shadow-lg py-1 min-w-[60px] z-50"
          style={{
            background: "var(--color-bg-secondary)",
            border: "1px solid var(--color-border)",
          }}
        >
          {availableLanguages.map((lang) => (
            <button
              key={lang.code}
              onClick={() => handleSelect(lang.code)}
              className="w-full px-3 py-1.5 text-left text-xs transition-colors"
              style={{
                color:
                  lang.code === language
                    ? "var(--color-accent)"
                    : "var(--color-text-secondary)",
                background:
                  lang.code === language
                    ? "rgba(10, 132, 255, 0.1)"
                    : "transparent",
              }}
            >
              {lang.label}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
