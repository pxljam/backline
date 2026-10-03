import React from "react";
import { Link, useLocation } from "react-router-dom";
import { Button } from "../components/ui";

/**
 * A wrong address says so.
 *
 * It used to redirect to the dashboard without a word, which is how a batch of
 * links pointing at accented paths went unnoticed: every click "worked", it
 * just always landed on the same page.
 */
export const NotFound: React.FC = () => {
  const { pathname } = useLocation();
  return (
    <section className="max-w-lg py-16 lg:py-24">
      <p className="font-mono text-xs text-ink-faint">404</p>
      <h1 className="mt-2 font-display text-3xl font-semibold tracking-tight">
        Cette page n'existe pas.
      </h1>
      <p className="mt-3 text-sm text-ink-soft">
        Rien ne répond à{" "}
        <code className="break-all rounded bg-raised px-1 py-0.5 font-mono text-xs text-ink">
          {pathname}
        </code>
        . Le lien est peut-être ancien, ou l'élément a été supprimé.
      </p>
      <Link to="/" className="mt-6 inline-block">
        <Button variant="primary" icon="arrow" tabIndex={-1}>
          Retour au tableau de bord
        </Button>
      </Link>
    </section>
  );
};
