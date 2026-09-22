/** Mise en forme francaise : l'interface est en francais (§14). */

const JOURS = ["dimanche", "lundi", "mardi", "mercredi", "jeudi", "vendredi", "samedi"];
const MOIS = [
  "janvier", "fevrier", "mars", "avril", "mai", "juin",
  "juillet", "aout", "septembre", "octobre", "novembre", "decembre",
];

export function dateLongue(iso: string): string {
  const d = new Date(iso);
  return `${JOURS[d.getDay()]} ${d.getDate()} ${MOIS[d.getMonth()]} ${d.getFullYear()}`;
}

export function dateCourte(iso: string): string {
  const d = new Date(iso);
  return `${String(d.getDate()).padStart(2, "0")}/${String(d.getMonth() + 1).padStart(2, "0")}/${d.getFullYear()}`;
}

export function heure(iso: string): string {
  const d = new Date(iso);
  return `${String(d.getHours()).padStart(2, "0")}h${String(d.getMinutes()).padStart(2, "0")}`;
}

export function dateEtHeure(iso: string): string {
  return `${dateCourte(iso)} a ${heure(iso)}`;
}

/** « dans 12 jours », « il y a 3 h » — utile pour dire ce qui presse. */
export function relatif(iso: string): string {
  const diff = new Date(iso).getTime() - Date.now();
  const abs = Math.abs(diff);
  const jours = Math.round(abs / 86_400_000);
  const heures = Math.round(abs / 3_600_000);
  const minutes = Math.round(abs / 60_000);

  const valeur =
    jours >= 1 ? `${jours} jour${jours > 1 ? "s" : ""}`
    : heures >= 1 ? `${heures} h`
    : `${minutes} min`;

  return diff >= 0 ? `dans ${valeur}` : `il y a ${valeur}`;
}

export function joursAvant(iso: string): number {
  return Math.ceil((new Date(iso).getTime() - Date.now()) / 86_400_000);
}

export function poids(octets: number): string {
  if (octets < 1024) return `${octets} o`;
  if (octets < 1024 * 1024) return `${Math.round(octets / 1024)} ko`;
  return `${(octets / (1024 * 1024)).toFixed(1)} Mo`;
}

export const STATUT_EVENEMENT: Record<string, string> = {
  draft: "brouillon",
  confirmed: "confirme",
  past: "passe",
  cancelled: "annule",
};

export const STATUT_OPPORTUNITE: Record<string, string> = {
  discussing: "en discussion",
  poll_open: "sondage ouvert",
  date_chosen: "date retenue",
  confirmed: "confirmee",
  abandoned: "abandonnee",
};

export const STATUT_TACHE: Record<string, string> = {
  draft: "brouillon",
  ready: "pret",
  assigned: "assigne",
  published: "publie",
  missed: "rate",
};

export const REPONSE_DISPO: Record<string, string> = {
  yes: "dispo",
  maybe: "peut-etre",
  no: "non",
};

export const PRESENCE_RESIDENCE: Record<string, string> = {
  coming: "je viens",
  not_coming: "je ne viens pas",
  unsure: "je ne sais pas encore",
};
