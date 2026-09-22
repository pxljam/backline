/** French formatting: the interface is in French (§14). */

const WEEKDAYS = ["dimanche", "lundi", "mardi", "mercredi", "jeudi", "vendredi", "samedi"];
const MONTHS = [
  "janvier", "fevrier", "mars", "avril", "mai", "juin",
  "juillet", "aout", "septembre", "octobre", "novembre", "decembre",
];

export function longDate(iso: string): string {
  const d = new Date(iso);
  return `${WEEKDAYS[d.getDay()]} ${d.getDate()} ${MONTHS[d.getMonth()]} ${d.getFullYear()}`;
}

export function shortDate(iso: string): string {
  const d = new Date(iso);
  return `${String(d.getDate()).padStart(2, "0")}/${String(d.getMonth() + 1).padStart(2, "0")}/${d.getFullYear()}`;
}

export function time(iso: string): string {
  const d = new Date(iso);
  return `${String(d.getHours()).padStart(2, "0")}h${String(d.getMinutes()).padStart(2, "0")}`;
}

export function dateAndTime(iso: string): string {
  return `${shortDate(iso)} a ${time(iso)}`;
}

/** "dans 12 jours", "il y a 3 h" — useful for saying what is pressing. */
export function relativeTime(iso: string): string {
  const diff = new Date(iso).getTime() - Date.now();
  const abs = Math.abs(diff);
  const days = Math.round(abs / 86_400_000);
  const hours = Math.round(abs / 3_600_000);
  const minutes = Math.round(abs / 60_000);

  const amount =
    days >= 1 ? `${days} jour${days > 1 ? "s" : ""}`
    : hours >= 1 ? `${hours} h`
    : `${minutes} min`;

  return diff >= 0 ? `dans ${amount}` : `il y a ${amount}`;
}

export function daysUntil(iso: string): number {
  return Math.ceil((new Date(iso).getTime() - Date.now()) / 86_400_000);
}

export function fileSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} o`;
  if (bytes < 1024 * 1024) return `${Math.round(bytes / 1024)} ko`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} Mo`;
}

export const EVENT_STATUS_LABELS: Record<string, string> = {
  draft: "brouillon",
  confirmed: "confirme",
  past: "passe",
  cancelled: "annule",
};

export const OPPORTUNITY_STATUS_LABELS: Record<string, string> = {
  discussing: "en discussion",
  poll_open: "sondage ouvert",
  date_chosen: "date retenue",
  confirmed: "confirmee",
  abandoned: "abandonnee",
};

export const TASK_STATUS_LABELS: Record<string, string> = {
  draft: "brouillon",
  ready: "pret",
  assigned: "assigne",
  published: "publie",
  missed: "rate",
};

export const AVAILABILITY_LABELS: Record<string, string> = {
  yes: "dispo",
  maybe: "peut-etre",
  no: "non",
};

export const RESIDENCY_PRESENCE_LABELS: Record<string, string> = {
  coming: "je viens",
  not_coming: "je ne viens pas",
  unsure: "je ne sais pas encore",
};
