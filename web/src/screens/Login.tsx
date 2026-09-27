import React, { useState } from "react";
import { Navigate, useNavigate } from "react-router-dom";
import { api } from "../lib/api";
import { useAction, useResource } from "../lib/hooks";
import { useSession } from "../lib/session";
import { Button, Card, ErrorNote, Field, Input } from "../components/ui";
import { TelegramLogin } from "../components/TelegramLogin";

interface PublicConfig {
  telegram_bot_username: string | null;
  telegram_enabled: boolean;
}

export const Login: React.FC = () => {
  const { me, loading, reload } = useSession();
  const { data: config } = useResource<PublicConfig>("/api/config");
  const { run, busy, error } = useAction();
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const navigate = useNavigate();

  if (!loading && me) return <Navigate to="/" replace />;

  const submit = async (fn: () => Promise<unknown>) => {
    const ok = await run(fn);
    if (ok !== null) {
      await reload();
      navigate("/");
    }
  };

  return (
    // The only screen a stranger ever sees. A single lit panel on a dark stage.
    <div className="relative flex min-h-full items-center justify-center px-6 py-16">
      <div
        aria-hidden
        className="pointer-events-none absolute inset-x-0 top-0 h-64 bg-[radial-gradient(60%_100%_at_50%_0%,var(--color-accent-quiet),transparent)]"
      />
      <div className="relative w-full max-w-sm">
      <div className="mb-6 text-center">
        <p className="font-display text-2xl font-semibold tracking-[0.3em]">
          BCKLN<span className="text-accent">_</span>
        </p>
        <h1 className="mt-2 text-lg">Backline</h1>
        <p className="mt-1 text-sm text-ink-soft">
          Pas d'inscription : un administrateur crée les comptes et envoie un lien
          d'invitation.
        </p>
      </div>

      <Card>

      {config?.telegram_enabled && config.telegram_bot_username && (
        <div className="mb-6">
          <p className="mb-2 text-xs uppercase tracking-wide text-ink-soft">Connexion Telegram</p>
          <TelegramLogin
            botUsername={config.telegram_bot_username}
            onAuth={(user) => void submit(() => api.post("/api/auth/login/telegram", user))}
          />
        </div>
      )}

      <form
        className="space-y-3"
        onSubmit={(e) => {
          e.preventDefault();
          void submit(() => api.post("/api/auth/login/password", { email, password }));
        }}
      >
        <p className="text-xs uppercase tracking-wide text-ink-soft">
          {config?.telegram_enabled ? "Ou, secours : e-mail" : "E-mail et mot de passe"}
        </p>
        <Field label="E-mail">
          <Input
            type="email"
            autoComplete="username"
            value={email}
            onChange={(e) => setEmail(e.target.value)}
            required
          />
        </Field>
        <Field label="Mot de passe">
          <Input
            type="password"
            autoComplete="current-password"
            value={password}
            onChange={(e) => setPassword(e.target.value)}
            required
          />
        </Field>
        <ErrorNote>{error}</ErrorNote>
        <Button type="submit" variant="primary" size="lg" disabled={busy}>
          {busy ? "…" : "Se connecter"}
        </Button>
      </form>
      </Card>
      </div>
    </div>
  );
};
