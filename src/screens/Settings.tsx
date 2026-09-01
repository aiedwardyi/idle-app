type Props = {
  alwaysOnTop: boolean;
  onToggleAlwaysOnTop: () => void;
};

export function Settings({ alwaysOnTop, onToggleAlwaysOnTop }: Props) {
  return (
    <div className="stack">
      <div className="setrow">
        <span className="k">
          Always on top
          {/* The choice cannot survive a restart yet: CONTRACT.md has no
              settings table. Session-only until the schema gains one. */}
          <span className="hint">session only for now</span>
        </span>
        <button
          type="button"
          className="toggle"
          aria-pressed={alwaysOnTop}
          aria-label="Always on top"
          onClick={onToggleAlwaysOnTop}
        />
      </div>
    </div>
  );
}
