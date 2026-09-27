import React from "react";
import type { Asset } from "../lib/types";
import { Button, Field, Select, cx } from "../components/ui";
import { Icon } from "../components/icons";

/**
 * Choose a media — or add one without leaving the editor.
 *
 * Until now the library could only be filled from Studio → Médias, so putting
 * an image in a template meant leaving the editor, uploading, coming back and
 * finding the file in a list. Worse, on an empty library the picker offered
 * "— à choisir —" and nothing else, with no hint that anything could be added.
 *
 * §9.1: no technical skill required, and nothing that sends you somewhere else
 * to finish what you started.
 */
export const MediaPicker: React.FC<{
  label: string;
  kind: "image" | "video";
  value: string | null | undefined;
  assets: Asset[];
  onChange: (assetId: string) => void;
  /** Uploads and returns the new asset's id, or null when it failed. */
  onUpload?: (file: File) => Promise<string | null>;
}> = ({ label, kind, value, assets, onChange, onUpload }) => {
  const input = React.useRef<HTMLInputElement>(null);
  const [busy, setBusy] = React.useState(false);
  const [error, setError] = React.useState<string | null>(null);

  const available = assets.filter((a) => a.kind === kind);

  return (
    <Field
      label={label}
      hint={
        available.length === 0
          ? "La bibliothèque est vide — ajoute un fichier ci-dessous."
          : "Bibliothèque du collectif."
      }
    >
      <div className="flex gap-2">
        <Select value={value ?? ""} onChange={(e) => onChange(e.target.value)}>
          <option value="">— à choisir —</option>
          {available.map((a) => (
            <option key={a.id} value={a.id}>
              {a.filename}
            </option>
          ))}
        </Select>
        {onUpload && (
          <Button
            type="button"
            variant={available.length === 0 ? "primary" : "ghost"}
            disabled={busy}
            onClick={() => input.current?.click()}
            title="Ajouter un fichier à la bibliothèque"
          >
            <Icon name={busy ? "spark" : "plus"} className={cx(busy && "animate-pulse")} />
          </Button>
        )}
      </div>

      {onUpload && (
        <input
          ref={input}
          type="file"
          accept={kind === "image" ? "image/*" : "video/*"}
          className="hidden"
          onChange={async (e) => {
            const file = e.target.files?.[0];
            if (!file) return;
            setBusy(true);
            setError(null);
            const id = await onUpload(file).catch(() => null);
            setBusy(false);
            // The picker is cleared either way, so the same file can be retried.
            e.target.value = "";
            if (id) onChange(id);
            else setError("le fichier n'a pas pu être ajouté");
          }}
        />
      )}
      {error && <span className="mt-1 block text-xs text-danger">{error}</span>}
    </Field>
  );
};
