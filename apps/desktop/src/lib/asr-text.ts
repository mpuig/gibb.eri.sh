export function normalizeSherpaDisplayText(text: string): string {
  const trimmed = text.trim();
  if (!trimmed) return "";

  const normalizedSpaces = trimmed.replace(/\s+/g, " ");

  const hasLetters = /[A-Za-z]/.test(normalizedSpaces);
  const hasLowercase = /[a-z]/.test(normalizedSpaces);
  const hasUppercase = /[A-Z]/.test(normalizedSpaces);

  if (!hasLetters || hasLowercase || !hasUppercase) {
    return normalizedSpaces;
  }

  const tokens = normalizedSpaces.split(" ").map((token) => {
    const lettersOnly = token.replace(/[^A-Za-z]/g, "");
    const isAcronym =
      lettersOnly.length >= 2 &&
      lettersOnly.length <= 4 &&
      lettersOnly === lettersOnly.toUpperCase();

    if (isAcronym) return token;

    const lower = token.toLowerCase();
    if (lower === "i") return "I";
    return lower;
  });

  const joined = tokens.join(" ");
  return joined.charAt(0).toUpperCase() + joined.slice(1);
}

