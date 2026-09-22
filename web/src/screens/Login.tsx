import React, { useState } from "react";
import { Navigate, useNavigate } from "react-router-dom";
import { api } from "../lib/api";
import { useAction, useResource } from "../lib/hooks";
import { useSession } from "../lib/session";
import { Button, ErrorNote, Field, Input } from "../components/ui";
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
    <div className="mx-auto flex min-h-full max-w-sm flex-col justify-center px-6 py-16">
      <div className="mb-8">
        <p className="text-2xl font-semibold tracking-[0.3em]">BCKLN</p>
        <h1 className="mt-2 text-lg">Backline</h1>
        <p className="mt-1 text-sm text-ink-soft">
          Pas d'inscription : un administrateur cree les comptes et envoie un lien
          d'invitation.
        </p>
      </div>

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
        <Button type="submit" variant="primary" disabled={busy} className="w-full">
          {busy ? "…" : "Se connecter"}
        </Button>
      </form>
    </div>
  );
};
