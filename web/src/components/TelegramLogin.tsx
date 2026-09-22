import React, { useEffect, useRef } from "react";

/**
 * Telegram Login Widget. Le script depose un bouton officiel ; Telegram signe
 * les donnees, et **la signature est verifiee cote Rust** (§15) — le widget
 * n'est qu'une porte, pas une preuve.
 */
export const TelegramLogin: React.FC<{
  botUsername: string;
  onAuth: (data: Record<string, unknown>) => void;
}> = ({ botUsername, onAuth }) => {
  const host = useRef<HTMLDivElement>(null);
  const handler = useRef(onAuth);
  handler.current = onAuth;

  useEffect(() => {
    const node = host.current;
    if (!node) return;

    // Le widget appelle une fonction globale : on en pose une, propre a ce
    // montage, et on la retire en partant.
    const callbackName = `onTelegramAuth_${Math.random().toString(36).slice(2)}`;
    (window as unknown as Record<string, unknown>)[callbackName] = (
      user: Record<string, unknown>,
    ) => handler.current(user);

    const script = document.createElement("script");
    script.src = "https://telegram.org/js/telegram-widget.js?22";
    script.async = true;
    script.setAttribute("data-telegram-login", botUsername);
    script.setAttribute("data-size", "large");
    script.setAttribute("data-radius", "8");
    script.setAttribute("data-request-access", "write");
    script.setAttribute("data-onauth", `${callbackName}(user)`);
    node.appendChild(script);

    return () => {
      node.innerHTML = "";
      delete (window as unknown as Record<string, unknown>)[callbackName];
    };
  }, [botUsername]);

  return <div ref={host} />;
};
