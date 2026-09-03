import type { ReactNode } from "react";

/**
 * Native <details>, so collapse works without JavaScript and comes with
 * keyboard handling and the right ARIA semantics for free. Collapsed by
 * default: omitting `open` is the default state.
 */
type Props = {
  title: string;
  hint?: string;
  children: ReactNode;
};

export function Section({ title, hint, children }: Props) {
  return (
    <details className="section">
      <summary>
        <svg
          className="chev"
          viewBox="0 0 24 24"
          width={12}
          height={12}
          aria-hidden="true"
          fill="none"
          stroke="currentColor"
          strokeWidth={2.6}
          strokeLinecap="round"
          strokeLinejoin="round"
        >
          <path d="M9 6l6 6-6 6" />
        </svg>
        <span className="section-title">{title}</span>
        {hint !== undefined && <span className="section-hint">{hint}</span>}
      </summary>
      <div className="section-body">{children}</div>
    </details>
  );
}
