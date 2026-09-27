import React, { useState } from "react";
import { Button, Modal } from "./ui";

/**
 * The invitation link, on its own.
 *
 * It used to be shown inside the "Nouveau membre" card, so asking for a fresh
 * link had to open the creation form to have somewhere to put it — which looked
 * like the wrong button had been pressed.
 */
export const InvitationLink: React.FC<{ url: string | null; onClose: () => void }> = ({
  url,
  onClose,
}) => {
  const [copied, setCopied] = useState(false);

  React.useEffect(() => {
    setCopied(false);
  }, [url]);

  if (!url) return null;

  return (
    <Modal open={!!url} onClose={onClose} title="Lien d'invitation">
      <p className="text-sm text-ink-soft">
        À usage unique, valable trente jours. En générer un nouveau annule le précédent.
      </p>
      <code className="mt-3 block break-all rounded-control border border-line bg-raised px-3 py-2 text-sm">
        {url}
      </code>
      <div className="mt-4 flex justify-end gap-2">
        <Button onClick={onClose}>Fermer</Button>
        <Button
          variant="primary"
          icon={copied ? "check" : "copy"}
          onClick={() => {
            // Clipboard access can be refused; the link stays selectable either way.
            void navigator.clipboard
              .writeText(url)
              .then(() => setCopied(true))
              .catch(() => setCopied(false));
          }}
        >
          {copied ? "copié" : "copier"}
        </Button>
      </div>
    </Modal>
  );
};
