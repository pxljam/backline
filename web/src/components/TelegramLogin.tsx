import React, { useEffect, useRef } from "react";

/**
 * Telegram Login Widget. The script drops in an official button; Telegram
 * signs the data, and **the signature is verified on the Rust side** (§15) —
 * the widget is a door, not a proof.
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

    // The widget calls a global function: install one scoped to this mount,
    // and remove it on the way out.
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
