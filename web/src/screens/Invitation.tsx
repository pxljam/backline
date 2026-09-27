import React, { useState } from "react";
import { useNavigate, useParams } from "react-router-dom";
import { api } from "../lib/api";
import { useAction, useResource } from "../lib/hooks";
import { useSession } from "../lib/session";
import { Button, ErrorNote, Field, Input, Loading } from "../components/ui";
import { TelegramLogin } from "../components/TelegramLogin";

interface Peek {
  display_name: string;
  collectives: string[];
  bot_username: string | null;
  has_email: boolean;
}

/** Single-use invitation link (§3). It is consumed once. */
export const Invitation: React.FC = () => {
  const { code = "" } = useParams();
  const { data, loading, error } = useResource<Peek>(`/api/auth/invitation/${code}`);
  const { run, busy, error: actionError } = useAction();
  const { reload } = useSession();
  const [password, setPassword] = useState("");
  const [email, setEmail] = useState("");
  const navigate = useNavigate();

  const accept = async (body: unknown) => {
    const ok = await run(() => api.post(`/api/auth/invitation/${code}`, body));
    if (ok !== null) {
      await reload();
      // A greeting rather than a wall of empty cards (see Dashboard).
      navigate("/?bienvenue=1");
    }
  };

  if (loading) return <Loading />;

  if (error || !data) {
    return (
      <div className="mx-auto max-w-sm px-6 py-20 text-center">
        <h1 className="text-lg font-semibold">Invitation introuvable</h1>
        <p className="mt-2 text-sm text-ink-soft">
          {error ?? "Ce lien a déjà servi, ou il a expiré."} Demande un nouveau lien a un
          administrateur.
        </p>
      </div>
    );
  }

  return (
    <div className="mx-auto flex min-h-full max-w-sm flex-col justify-center px-6 py-16">
      <p className="text-2xl font-semibold tracking-[0.3em]">BCKLN</p>
      <h1 className="mt-3 text-lg">Bienvenue, {data.display_name}</h1>
      <p className="mt-1 text-sm text-ink-soft">
        {data.collectives.length > 0
          ? `Tu rejoins ${data.collectives.join(", ")}.`
          : "Ton compte est prêt."}{" "}
        Lié ton compte pour entrer — ce lien ne sert qu'une fois.
      </p>

      {data.bot_username && (
        <div className="my-6">
          <TelegramLogin
            botUsername={data.bot_username}
            onAuth={(user) => void accept({ telegram: user })}
          />
        </div>
      )}

      <form
        className="mt-4 space-y-3"
        onSubmit={(e) => {
          e.preventDefault();
          void accept({ password, email: email || undefined });
        }}
      >
        <p className="text-xs uppercase tracking-wide text-ink-soft">
          {data.bot_username ? "Ou, sans Telegram" : "Choisis un mot de passe"}
        </p>
        <Field
          label={data.has_email ? "E-mail (optionnel)" : "E-mail"}
          hint={
            data.has_email
              ? "Laisse vide pour garder celui que l'administrateur a renseigné."
              : "C'est avec cette adresse que tu te connecteras."
          }
        >
          <Input
            type="email"
            autoComplete="username"
            value={email}
            onChange={(e) => setEmail(e.target.value)}
            required={!data.has_email}
          />
        </Field>
        <Field label="Mot de passe" hint="10 caractères minimum">
          <Input
            type="password"
            autoComplete="new-password"
            minLength={10}
            value={password}
            onChange={(e) => setPassword(e.target.value)}
            required
          />
        </Field>
        <ErrorNote>{actionError}</ErrorNote>
        <Button type="submit" variant="primary" disabled={busy} className="w-full">
          {busy ? "…" : "Activer mon compte"}
        </Button>
      </form>
    </div>
  );
};
